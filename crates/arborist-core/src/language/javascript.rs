use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path};

const JAVASCRIPT_FAMILY_EXTENSIONS: &[&str] =
    &["js", "jsx", "mjs", "cjs", "ts", "mts", "cts", "tsx"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JavaScriptNamedModuleBinding {
    pub(crate) imported_name: String,
    pub(crate) module_paths: BTreeSet<PathBuf>,
    pub(crate) unresolved: bool,
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

pub(crate) fn javascript_named_import_module_paths_with_overrides_and_check(
    path: &Path,
    root: Node<'_>,
    source: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    check: Option<&dyn Fn() -> Result<()>>,
) -> Result<BTreeMap<String, JavaScriptNamedModuleBinding>> {
    let mut bindings = BTreeMap::new();
    collect_javascript_named_import_module_paths(
        path,
        root,
        source,
        file_overrides,
        check,
        &mut bindings,
    )?;
    Ok(bindings)
}

pub(crate) fn javascript_named_reexport_module_paths_with_overrides_and_check(
    path: &Path,
    root: Node<'_>,
    source: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    check: Option<&dyn Fn() -> Result<()>>,
) -> Result<BTreeMap<String, JavaScriptNamedModuleBinding>> {
    let mut bindings = BTreeMap::new();
    collect_javascript_named_reexport_module_paths(
        path,
        root,
        source,
        file_overrides,
        check,
        &mut bindings,
    )?;
    Ok(bindings)
}

pub(crate) fn javascript_star_reexport_module_paths_with_overrides_and_check(
    path: &Path,
    root: Node<'_>,
    source: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    check: Option<&dyn Fn() -> Result<()>>,
) -> Result<BTreeSet<PathBuf>> {
    let mut module_paths = BTreeSet::new();
    collect_javascript_star_reexport_module_paths(
        path,
        root,
        source,
        file_overrides,
        check,
        &mut module_paths,
    )?;
    Ok(module_paths)
}

pub(crate) fn resolve_local_javascript_module_path(
    current_path: &Path,
    specifier: &str,
) -> Option<PathBuf> {
    resolve_local_javascript_module_path_with_overrides(current_path, specifier, None)
}

pub(crate) fn resolve_local_javascript_module_path_with_overrides(
    current_path: &Path,
    specifier: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Option<PathBuf> {
    if !is_relative_module_specifier(specifier) {
        return None;
    }

    let parent = current_path.parent()?;
    let base = normalize_absolute_path(&parent.join(specifier)).ok()?;
    local_module_candidates(&base)
        .into_iter()
        .find(|candidate| {
            is_javascript_family_source_file(candidate)
                || file_overrides.is_some_and(|overrides| {
                    overrides.contains_key(&super::normalize_path(candidate))
                })
        })
}

fn collect_javascript_named_import_module_paths(
    path: &Path,
    node: Node<'_>,
    source: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    check: Option<&dyn Fn() -> Result<()>>,
    bindings: &mut BTreeMap<String, JavaScriptNamedModuleBinding>,
) -> Result<()> {
    if let Some(check) = check {
        check()?;
    }
    if node.kind() == "import_statement" {
        let module_path = node
            .child_by_field_name("source")
            .or_else(|| first_string_child(node))
            .and_then(|source_node| javascript_string_literal(source_node, source).transpose())
            .transpose()?
            .and_then(|specifier| {
                resolve_local_javascript_module_path_with_overrides(
                    path,
                    &specifier,
                    file_overrides,
                )
            });
        collect_default_and_namespace_import_bindings(node, source, module_path.clone(), bindings)?;
        for (imported_name, local_name) in named_import_bindings(node, source)? {
            insert_javascript_module_binding(
                bindings,
                local_name,
                imported_name.clone(),
                module_path.clone(),
                imported_name == "<unsupported>",
            );
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_javascript_named_import_module_paths(
            path,
            child,
            source,
            file_overrides,
            check,
            bindings,
        )?;
    }
    Ok(())
}

fn collect_javascript_named_reexport_module_paths(
    path: &Path,
    node: Node<'_>,
    source: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    check: Option<&dyn Fn() -> Result<()>>,
    bindings: &mut BTreeMap<String, JavaScriptNamedModuleBinding>,
) -> Result<()> {
    if let Some(check) = check {
        check()?;
    }
    if node.kind() == "export_statement"
        && let Some(source_node) = node.child_by_field_name("source")
    {
        let module_path = javascript_string_literal(source_node, source)?.and_then(|specifier| {
            resolve_local_javascript_module_path_with_overrides(path, &specifier, file_overrides)
        });
        for (imported_name, exported_name) in named_reexport_bindings(node, source)? {
            insert_javascript_module_binding(
                bindings,
                exported_name,
                imported_name.clone(),
                module_path.clone(),
                imported_name == "<unsupported>",
            );
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_javascript_named_reexport_module_paths(
            path,
            child,
            source,
            file_overrides,
            check,
            bindings,
        )?;
    }
    Ok(())
}

fn collect_javascript_star_reexport_module_paths(
    path: &Path,
    node: Node<'_>,
    source: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    check: Option<&dyn Fn() -> Result<()>>,
    module_paths: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    if let Some(check) = check {
        check()?;
    }
    if is_javascript_star_reexport_statement(node)
        && let Some(source_node) = node.child_by_field_name("source")
        && let Some(specifier) = javascript_string_literal(source_node, source)?
        && let Some(module_path) =
            resolve_local_javascript_module_path_with_overrides(path, &specifier, file_overrides)
    {
        module_paths.insert(module_path);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_javascript_star_reexport_module_paths(
            path,
            child,
            source,
            file_overrides,
            check,
            module_paths,
        )?;
    }
    Ok(())
}

/// Collects the names a module source declares as direct named exports:
/// declaration exports (`export function foo`, `export const foo`,
/// `export class Foo`, TypeScript `export interface Foo`/`export type Foo`),
/// and `export { foo }` / `export { foo as bar }` clauses without a source.
/// Re-export forms (`export ... from "./module"`) and default exports are not
/// direct named exports and are excluded.
pub(crate) fn javascript_named_export_names(
    root: Node<'_>,
    source: &str,
    check: Option<&dyn Fn() -> Result<()>>,
) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if let Some(check) = check {
            check()?;
        }
        if statement.kind() != "export_statement" {
            continue;
        }
        if statement.child_by_field_name("source").is_some() {
            // `export { name as alias } from "./module"` and `export * from
            // "./module"` are re-exports, not direct declarations.
            continue;
        }
        if let Some(export_clause) = statement.named_child(0)
            && export_clause.kind() == "export_clause"
        {
            let mut clause_cursor = export_clause.walk();
            for specifier in export_clause.named_children(&mut clause_cursor) {
                if specifier.kind() != "export_specifier" {
                    continue;
                }
                let exported_node = specifier
                    .child_by_field_name("alias")
                    .or_else(|| specifier.child_by_field_name("name"));
                if let Some(exported_node) = exported_node
                    && let Ok(exported_name) = node_text(exported_node, source)
                    && !exported_name.trim().is_empty()
                {
                    names.insert(exported_name.trim().to_owned());
                }
            }
            continue;
        }
        // `export default ...` exports the name `default`, not the underlying
        // declaration's name; the dedicated default-export helper owns it.
        let text = node_text(statement, source)?.trim_start();
        let is_default_export = text
            .strip_prefix("export")
            .is_some_and(|rest| rest.trim_start().starts_with("default"));
        if is_default_export {
            continue;
        }
        if let Some(declaration) = statement.child_by_field_name("declaration") {
            // Function, class, interface, type-alias, and enum declarations
            // carry their name directly; const/let/var declarations carry it
            // on each variable declarator.
            let mut declared_names = BTreeSet::new();
            if let Some(declared_name) = declaration.child_by_field_name("name")
                && matches!(declared_name.kind(), "identifier" | "type_identifier")
                && let Ok(name) = node_text(declared_name, source)
                && !name.trim().is_empty()
            {
                declared_names.insert(name.trim().to_owned());
            }
            let mut cursor = declaration.walk();
            for child in declaration.named_children(&mut cursor) {
                if child.kind() != "variable_declarator" {
                    continue;
                }
                if let Some(declared_name) = child.child_by_field_name("name")
                    && matches!(declared_name.kind(), "identifier" | "type_identifier")
                    && let Ok(name) = node_text(declared_name, source)
                    && !name.trim().is_empty()
                {
                    declared_names.insert(name.trim().to_owned());
                }
            }
            names.extend(declared_names);
        }
    }
    Ok(names)
}

fn collect_default_and_namespace_import_bindings(
    node: Node<'_>,
    source: &str,
    module_path: Option<PathBuf>,
    bindings: &mut BTreeMap<String, JavaScriptNamedModuleBinding>,
) -> Result<()> {
    let Some(import_clause) = node
        .named_children(&mut node.walk())
        .find(|child| child.kind() == "import_clause")
    else {
        return Ok(());
    };

    let mut cursor = import_clause.walk();
    for child in import_clause.named_children(&mut cursor) {
        let (local_name, imported_name) = match child.kind() {
            "identifier" => (node_text(child, source)?.trim().to_owned(), "default"),
            "namespace_import" => {
                let Some(local_node) = child.named_child(0) else {
                    continue;
                };
                (
                    node_text(local_node, source)?.trim().to_owned(),
                    "<namespace>",
                )
            }
            _ => continue,
        };
        if local_name.is_empty() {
            continue;
        }
        // Default imports resolve to the target module's default export and
        // namespace imports bind the whole target module so member calls such
        // as `ns.helper(...)` can resolve within it; non-local module paths
        // still fail closed inside insert_javascript_module_binding.
        insert_javascript_module_binding(
            bindings,
            local_name,
            imported_name.to_owned(),
            module_path.clone(),
            false,
        );
    }
    Ok(())
}

/// Returns the local declaration name of a module's default export when it can
/// be resolved conservatively: a named `export default function`/`class`
/// declaration, `export default <identifier>` naming a declared module-level
/// symbol, or `export { localName as default }`. Anonymous default exports,
/// expression defaults that do not name a declaration, and modules with
/// conflicting or absent default exports fail closed (`None`).
pub(crate) fn javascript_module_default_export_local_name(
    root: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let mut names = BTreeSet::new();
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if statement.kind() != "export_statement" {
            continue;
        }
        if let Some(name) = javascript_default_export_name(statement, source)? {
            names.insert(name);
        }
    }
    // A module may declare at most one default export; anything else fails
    // closed instead of guessing.
    Ok((names.len() == 1)
        .then(|| names.iter().next().cloned())
        .flatten())
}

fn javascript_default_export_name(statement: Node<'_>, source: &str) -> Result<Option<String>> {
    // `export { localName as default };` names a declared module-level symbol
    // even though the statement text does not start with "export default".
    // Re-export forms with a source clause are handled by the named
    // re-export machinery and must not count as a local default export.
    if let Some(export_clause) = statement.named_child(0)
        && export_clause.kind() == "export_clause"
        && statement.child_by_field_name("source").is_none()
    {
        let mut names = BTreeSet::new();
        let mut clause_cursor = export_clause.walk();
        for specifier in export_clause.named_children(&mut clause_cursor) {
            if specifier.kind() != "export_specifier" {
                continue;
            }
            let Some(alias) = specifier.child_by_field_name("alias") else {
                continue;
            };
            if node_text(alias, source)?.trim() != "default" {
                continue;
            }
            let Some(name) = specifier.child_by_field_name("name") else {
                continue;
            };
            if let Ok(name) = node_text(name, source) {
                let name = name.trim();
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
        }
        return Ok((names.len() == 1)
            .then(|| names.iter().next().cloned())
            .flatten());
    }

    let text = node_text(statement, source)?.trim_start();
    let is_default_export = text
        .strip_prefix("export")
        .is_some_and(|rest| rest.trim_start().starts_with("default"));
    if !is_default_export {
        return Ok(None);
    }

    if let Some(declaration) = statement.child_by_field_name("declaration") {
        // Named `export default function foo() {}` / `export default class Foo {}`
        // declarations carry a stable name; anonymous declarations do not.
        if let Some(name) = declaration.child_by_field_name("name")
            && !name.is_missing()
            && let Ok(name) = node_text(name, source)
        {
            let name = name.trim();
            return Ok((!name.is_empty()).then(|| name.to_string()));
        }
        return Ok(None);
    }

    // `export default <identifier>;` names a declared module-level symbol.
    if let Some(value) = statement.child_by_field_name("value")
        && value.kind() == "identifier"
        && let Ok(name) = node_text(value, source)
    {
        let name = name.trim();
        return Ok((!name.is_empty()).then(|| name.to_string()));
    }

    Ok(None)
}

fn insert_javascript_module_binding(
    bindings: &mut BTreeMap<String, JavaScriptNamedModuleBinding>,
    local_name: String,
    imported_name: String,
    module_path: Option<PathBuf>,
    unresolved: bool,
) {
    let Some(module_path) = module_path else {
        bindings.insert(
            local_name,
            JavaScriptNamedModuleBinding {
                imported_name,
                module_paths: BTreeSet::new(),
                unresolved: true,
            },
        );
        return;
    };
    if unresolved {
        bindings.insert(
            local_name,
            JavaScriptNamedModuleBinding {
                imported_name,
                module_paths: BTreeSet::new(),
                unresolved: true,
            },
        );
        return;
    }

    match bindings.entry(local_name) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(JavaScriptNamedModuleBinding {
                imported_name,
                module_paths: BTreeSet::from([module_path]),
                unresolved: false,
            });
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let binding = entry.get_mut();
            binding.unresolved = true;
            binding.module_paths.clear();
        }
    }
}

/// Returns whether the statement is a star re-export (`export * from "./x"`).
/// `export * as ns from "./x"` is a namespace re-export and is not included.
fn is_javascript_star_reexport_statement(node: Node<'_>) -> bool {
    if node.kind() != "export_statement" || node.child_by_field_name("source").is_none() {
        return false;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .all(|child| child.kind() != "export_clause" && child.kind() != "namespace_export")
}

fn named_import_bindings(node: Node<'_>, source: &str) -> Result<Vec<(String, String)>> {
    let mut bindings = Vec::new();
    collect_named_import_bindings(node, source, &mut bindings)?;
    Ok(bindings)
}

fn named_reexport_bindings(node: Node<'_>, source: &str) -> Result<Vec<(String, String)>> {
    let mut bindings = Vec::new();
    collect_named_reexport_bindings(node, source, &mut bindings)?;
    Ok(bindings)
}

fn collect_named_import_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut Vec<(String, String)>,
) -> Result<()> {
    if node.kind() == "import_specifier" {
        if let Some(binding) = named_identifier_binding(node, source)? {
            bindings.push(binding);
        }
        return Ok(());
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_named_import_bindings(child, source, bindings)?;
    }
    Ok(())
}

fn collect_named_reexport_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut Vec<(String, String)>,
) -> Result<()> {
    if node.kind() == "export_specifier" {
        if let Some(binding) = named_identifier_binding(node, source)? {
            bindings.push(binding);
        }
        return Ok(());
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_named_reexport_bindings(child, source, bindings)?;
    }
    Ok(())
}

fn named_identifier_binding(node: Node<'_>, source: &str) -> Result<Option<(String, String)>> {
    let Some(imported_node) = node.child_by_field_name("name") else {
        return Ok(None);
    };
    let imported_name = if imported_node.kind() == "identifier" {
        node_text(imported_node, source)?.trim().to_owned()
    } else {
        "<unsupported>".to_owned()
    };
    if imported_name.is_empty() {
        return Ok(None);
    }

    let Some(alias_node) = node.child_by_field_name("alias") else {
        return if imported_name == "<unsupported>" {
            Ok(None)
        } else {
            Ok(Some((imported_name.clone(), imported_name)))
        };
    };
    if alias_node.kind() != "identifier" {
        return Ok(None);
    }
    let local_name = node_text(alias_node, source)?.trim().to_owned();
    if local_name.is_empty() {
        return Ok(None);
    }
    Ok(Some((imported_name, local_name)))
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
    use std::cell::Cell;
    use std::collections::BTreeSet;
    use std::path::Path;

    use anyhow::bail;

    use super::{
        javascript_module_default_export_local_name, javascript_named_export_names,
        javascript_named_import_module_paths_with_overrides_and_check,
        javascript_named_reexport_module_paths_with_overrides_and_check,
        javascript_star_reexport_module_paths_with_overrides_and_check,
        javascript_static_module_specifiers,
    };
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
    fn checks_the_traversal_budget_while_collecting_named_imports() {
        let source = "import { helper } from \"./helper\";\n";
        let document = parse_document(Path::new("sample.ts"), source).unwrap();
        let checks = Cell::new(0);
        let check = || {
            let count = checks.get() + 1;
            checks.set(count);
            if count == 2 {
                bail!("binding traversal budget exhausted");
            }
            Ok(())
        };

        let error = javascript_named_import_module_paths_with_overrides_and_check(
            Path::new("sample.ts"),
            document.tree.root_node(),
            source,
            None,
            Some(&check),
        )
        .expect_err("binding traversal must stop when the supplied budget expires");
        assert!(
            error
                .to_string()
                .contains("binding traversal budget exhausted")
        );
        assert_eq!(checks.get(), 2);
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

        let bindings = javascript_named_import_module_paths_with_overrides_and_check(
            &importer,
            document.tree.root_node(),
            source,
            None,
            None,
        )
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

    #[test]
    fn binds_default_and_namespace_imports_to_local_modules() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-default-bindings-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let helper = root.join("helper.ts");
        let helper_path = crate::language::normalize_path(&helper);
        std::fs::write(&helper, "export default function helper() {}\n").unwrap();
        let source = "import selected from \"./helper\";\nimport * as namespace from \"./helper\";\nimport { default as selectedAlias } from \"./helper\";\nexport { default as forwarded } from \"./helper\";\n";
        let document = parse_document(&module, source).unwrap();

        let imports = javascript_named_import_module_paths_with_overrides_and_check(
            &module,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        let reexports = javascript_named_reexport_module_paths_with_overrides_and_check(
            &module,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        for local_name in ["selected", "selectedAlias"] {
            let binding = imports.get(local_name).unwrap();
            assert_eq!(binding.imported_name, "default");
            assert!(!binding.unresolved);
            assert_eq!(
                binding
                    .module_paths
                    .iter()
                    .map(|path| crate::language::normalize_path(path))
                    .collect::<Vec<_>>(),
                vec![helper_path.clone()]
            );
        }
        let namespace = imports.get("namespace").unwrap();
        assert_eq!(namespace.imported_name, "<namespace>");
        assert!(!namespace.unresolved);
        assert_eq!(
            namespace
                .module_paths
                .iter()
                .map(|path| crate::language::normalize_path(path))
                .collect::<Vec<_>>(),
            vec![helper_path.clone()]
        );

        let forwarded = reexports.get("forwarded").unwrap();
        assert_eq!(forwarded.imported_name, "default");
        assert!(!forwarded.unresolved);
        assert_eq!(
            forwarded
                .module_paths
                .iter()
                .map(|path| crate::language::normalize_path(path))
                .collect::<Vec<_>>(),
            vec![helper_path]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_module_default_export_local_names_conservatively() {
        for (source, expected) in [
            ("export default function helper() {}\n", Some("helper")),
            ("export default class Counter {}\n", Some("Counter")),
            (
                "function helper() {}\nexport default helper;\n",
                Some("helper"),
            ),
            (
                "function helper() {}\nexport { helper as default };\n",
                Some("helper"),
            ),
            (
                "function helper() {}\nexport { helper as default } from \"./other\";\n",
                None,
            ),
            ("export default function () {}\n", None),
            ("export default class {}\n", None),
            ("export default 42;\n", None),
            ("export const helper = () => 1;\n", None),
        ] {
            let document = parse_document(Path::new("sample.ts"), source).unwrap();
            assert_eq!(
                javascript_module_default_export_local_name(document.tree.root_node(), source)
                    .unwrap(),
                expected.map(str::to_string),
                "source: {source:?}"
            );
        }
    }

    #[test]
    fn ignores_local_named_exports_when_collecting_reexport_bindings() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-local-exports-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let source = "function helper() {}\nexport { helper };\n";
        let document = parse_document(&module, source).unwrap();

        let bindings = javascript_named_reexport_module_paths_with_overrides_and_check(
            &module,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        assert!(bindings.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn poisons_conflicting_named_reexport_bindings() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-reexport-conflicts-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        std::fs::write(&helper, "export function helper() {}\n").unwrap();
        let source = "export { helper as forwarded } from \"./missing\";\nexport { helper as forwarded } from \"./helper\";\n";
        let document = parse_document(&bridge, source).unwrap();

        let bindings = javascript_named_reexport_module_paths_with_overrides_and_check(
            &bridge,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        assert!(
            bindings
                .get("forwarded")
                .is_some_and(|binding| binding.unresolved && binding.module_paths.is_empty())
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_named_reexport_bindings_to_local_modules() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-reexport-bindings-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        std::fs::write(&helper, "export function helper() {}\n").unwrap();
        let source = "export { helper as forwarded, other } from \"./helper\";\n";
        let document = parse_document(&bridge, source).unwrap();

        let bindings = javascript_named_reexport_module_paths_with_overrides_and_check(
            &bridge,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            bindings
                .get("forwarded")
                .map(|binding| &binding.imported_name),
            Some(&"helper".to_string())
        );
        assert_eq!(
            bindings
                .get("forwarded")
                .map(|binding| &binding.module_paths),
            Some(&BTreeSet::from([helper.clone()]))
        );
        assert_eq!(
            bindings.get("other").map(|binding| &binding.module_paths),
            Some(&BTreeSet::from([helper]))
        );
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn collects_star_reexport_module_paths_to_local_modules() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-reexports-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let bridge = root.join("bridge.ts");
        let first = root.join("first.ts");
        let second = root.join("second.ts");
        std::fs::write(&first, "export function first() {}\n").unwrap();
        std::fs::write(&second, "export function second() {}\n").unwrap();
        let source = "export * from \"./first\";\nexport * as ns from \"./second\";\nexport { first as renamed } from \"./first\";\n";
        let document = parse_document(&bridge, source).unwrap();

        let module_paths = javascript_star_reexport_module_paths_with_overrides_and_check(
            &bridge,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            module_paths
                .iter()
                .map(|path| crate::language::normalize_path(path))
                .collect::<Vec<_>>(),
            vec![crate::language::normalize_path(&first)]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn collects_direct_named_export_names_conservatively() {
        for (source, expected) in [
            (
                "export function helper() {}\n",
                BTreeSet::from(["helper".to_string()]),
            ),
            (
                "export const helper = () => 1;\nexport class Counter {}\nexport interface Shape {}\n",
                BTreeSet::from([
                    "Counter".to_string(),
                    "Shape".to_string(),
                    "helper".to_string(),
                ]),
            ),
            (
                "export { helper };\n",
                BTreeSet::from(["helper".to_string()]),
            ),
            (
                "export { helper as forwarded };\n",
                BTreeSet::from(["forwarded".to_string()]),
            ),
            (
                "export { helper as default };\n",
                BTreeSet::from(["default".to_string()]),
            ),
            ("export default function helper() {}\n", BTreeSet::new()),
            ("export default helper;\n", BTreeSet::new()),
            (
                "export { helper } from \"./other\";\nexport * from \"./other\";\n",
                BTreeSet::new(),
            ),
            ("function helper() {}\n", BTreeSet::new()),
        ] {
            let document = parse_document(Path::new("sample.ts"), source).unwrap();
            assert_eq!(
                javascript_named_export_names(document.tree.root_node(), source, None).unwrap(),
                expected,
                "source: {source:?}"
            );
        }
    }
}
