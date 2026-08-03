use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path};

const JAVASCRIPT_FAMILY_EXTENSIONS: &[&str] =
    &["js", "jsx", "mjs", "cjs", "ts", "mts", "cts", "tsx"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JavaScriptNamedImport {
    pub(crate) imported_name: String,
    pub(crate) module_paths: BTreeSet<PathBuf>,
}

pub(crate) fn javascript_local_module_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    javascript_static_module_specifiers(root, source).map(|specifiers| {
        specifiers
            .into_iter()
            .filter_map(|specifier| resolve_local_javascript_module_path(path, &specifier))
            .collect()
    })
}

pub(crate) fn javascript_named_import_module_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, JavaScriptNamedImport>> {
    let mut bindings = BTreeMap::new();
    collect_javascript_named_import_module_paths(path, root, source, &mut bindings)?;
    Ok(bindings)
}

pub(crate) fn resolve_local_javascript_module_path(
    current_path: &Path,
    specifier: &str,
) -> Option<PathBuf> {
    if !is_relative_module_specifier(specifier) {
        return None;
    }

    let parent = current_path.parent()?;
    let base = normalize_absolute_path(&parent.join(specifier)).ok()?;
    local_module_candidates(&base)
        .into_iter()
        .find(|candidate| is_javascript_family_source_file(candidate))
}

fn collect_javascript_named_import_module_paths(
    path: &Path,
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeMap<String, JavaScriptNamedImport>,
) -> Result<()> {
    if node.kind() == "import_statement" {
        let module_path = node
            .child_by_field_name("source")
            .or_else(|| first_string_child(node))
            .and_then(|source_node| javascript_string_literal(source_node, source).transpose())
            .transpose()?
            .and_then(|specifier| resolve_local_javascript_module_path(path, &specifier));
        for (imported_name, local_name) in named_import_bindings(node, source)? {
            let binding = bindings
                .entry(local_name)
                .or_insert_with(|| JavaScriptNamedImport {
                    imported_name: imported_name.clone(),
                    module_paths: BTreeSet::new(),
                });
            if binding.imported_name == imported_name
                && let Some(module_path) = &module_path
            {
                binding.module_paths.insert(module_path.clone());
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_javascript_named_import_module_paths(path, child, source, bindings)?;
    }
    Ok(())
}

fn named_import_bindings(node: Node<'_>, source: &str) -> Result<Vec<(String, String)>> {
    let mut bindings = Vec::new();
    collect_named_import_bindings(node, source, &mut bindings)?;
    Ok(bindings)
}

fn collect_named_import_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut Vec<(String, String)>,
) -> Result<()> {
    if node.kind() == "import_specifier" {
        let names = identifier_names(node, source)?;
        if let Some(imported_name) = names.first() {
            let local_name = names.last().unwrap_or(imported_name);
            bindings.push((imported_name.clone(), local_name.clone()));
        }
        return Ok(());
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_named_import_bindings(child, source, bindings)?;
    }
    Ok(())
}

fn identifier_names(node: Node<'_>, source: &str) -> Result<Vec<String>> {
    let mut names = Vec::new();
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if matches!(current.kind(), "identifier" | "type_identifier") {
            let name = node_text(current, source)?.trim().to_string();
            if !name.is_empty() {
                names.push(name);
            }
            continue;
        }
        let mut cursor = current.walk();
        let children = current.named_children(&mut cursor).collect::<Vec<_>>();
        pending.extend(children.into_iter().rev());
    }
    Ok(names)
}

fn javascript_static_module_specifiers(root: Node<'_>, source: &str) -> Result<BTreeSet<String>> {
    let mut specifiers = BTreeSet::new();
    collect_javascript_static_module_specifiers(root, source, &mut specifiers)?;
    Ok(specifiers)
}

fn collect_javascript_static_module_specifiers(
    node: Node<'_>,
    source: &str,
    specifiers: &mut BTreeSet<String>,
) -> Result<()> {
    match node.kind() {
        "import_statement" | "export_statement" => {
            if let Some(source_node) = node
                .child_by_field_name("source")
                .or_else(|| first_string_child(node))
                && let Some(specifier) = javascript_string_literal(source_node, source)?
            {
                specifiers.insert(specifier);
            }
        }
        "call_expression" => {
            if let Some(specifier) = direct_require_specifier(node, source)? {
                specifiers.insert(specifier);
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_javascript_static_module_specifiers(child, source, specifiers)?;
    }
    Ok(())
}

fn direct_require_specifier(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(function) = node.child_by_field_name("function") else {
        return Ok(None);
    };
    if function.kind() != "identifier" || node_text(function, source)?.trim() != "require" {
        return Ok(None);
    }

    let Some(arguments) = node.child_by_field_name("arguments") else {
        return Ok(None);
    };
    let mut cursor = arguments.walk();
    let arguments = arguments.named_children(&mut cursor).collect::<Vec<_>>();
    let [argument] = arguments.as_slice() else {
        return Ok(None);
    };
    javascript_string_literal(*argument, source)
}

fn first_string_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "string")
}

fn javascript_string_literal(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if node.kind() != "string" {
        return Ok(None);
    }
    let literal = node_text(node, source)?.trim();
    let value = literal
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            literal
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        });
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() || value.contains('\\') {
        return Ok(None);
    }
    Ok(Some(value.to_string()))
}

fn is_relative_module_specifier(specifier: &str) -> bool {
    specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../")
}

fn local_module_candidates(base: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if base.extension().is_some() {
        candidates.push(base.to_path_buf());
        return candidates;
    }

    for extension in JAVASCRIPT_FAMILY_EXTENSIONS {
        candidates.push(base.with_extension(extension));
    }
    for extension in JAVASCRIPT_FAMILY_EXTENSIONS {
        candidates.push(base.join("index").with_extension(extension));
    }
    candidates
}

fn is_javascript_family_source_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                JAVASCRIPT_FAMILY_EXTENSIONS
                    .iter()
                    .any(|candidate| extension.eq_ignore_ascii_case(candidate))
            })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::{javascript_named_import_module_paths, javascript_static_module_specifiers};
    use crate::language::parse_document;

    #[test]
    fn collects_static_import_reexport_and_direct_require_specifiers() {
        let source = r#"
import { helper } from "./helper";
export { helper as forwarded } from './bridge';
const legacy = require("../legacy");
const dynamic = import(moduleName);
const packageValue = require("package-name");
const escaped = require("./escaped\\name");
"#;
        let document = parse_document(Path::new("sample.ts"), source).unwrap();

        assert_eq!(
            javascript_static_module_specifiers(document.tree.root_node(), source).unwrap(),
            BTreeSet::from([
                "../legacy".to_string(),
                "./bridge".to_string(),
                "./helper".to_string(),
                "package-name".to_string(),
            ])
        );
    }

    #[test]
    fn resolves_named_import_bindings_to_local_modules() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-import-bindings-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let importer = root.join("caller.ts");
        let helper = root.join("helper.ts");
        std::fs::write(&helper, "export function helper() {}\n").unwrap();
        let source = "import { helper as localHelper, other } from \"./helper\";\n";
        let document = parse_document(&importer, source).unwrap();

        let bindings =
            javascript_named_import_module_paths(&importer, document.tree.root_node(), source)
                .unwrap();
        assert_eq!(
            bindings
                .get("localHelper")
                .map(|binding| &binding.imported_name),
            Some(&"helper".to_string())
        );
        assert_eq!(
            bindings
                .get("localHelper")
                .map(|binding| &binding.module_paths),
            Some(&BTreeSet::from([helper.clone()]))
        );
        assert_eq!(
            bindings.get("other").map(|binding| &binding.module_paths),
            Some(&BTreeSet::from([helper]))
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
