use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path};

const JAVASCRIPT_FAMILY_EXTENSIONS: &[&str] =
    &["js", "jsx", "mjs", "cjs", "ts", "mts", "cts", "tsx"];

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

    use super::javascript_static_module_specifiers;
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
}
