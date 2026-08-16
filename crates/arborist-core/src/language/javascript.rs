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
        collect_import_equals_namespace_bindings(node, path, source, file_overrides, bindings)?;
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
    } else if matches!(node.kind(), "lexical_declaration" | "variable_declaration") {
        for (local_name, imported_name, module_path) in
            javascript_require_declaration_bindings(node, path, source, file_overrides)?
        {
            insert_javascript_module_binding(
                bindings,
                local_name,
                imported_name,
                module_path,
                false,
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
        // `export * as ns from "./module"` re-exports the target module's
        // namespace under the exported name; member calls on it resolve
        // within the target module like a namespace import.
        if let Some(namespace_export) = node
            .named_children(&mut node.walk())
            .find(|child| child.kind() == "namespace_export")
            && let Some(namespace_name) = namespace_export.named_child(0)
            && let Ok(namespace_name) = node_text(namespace_name, source)
            && !namespace_name.trim().is_empty()
        {
            insert_javascript_module_binding(
                bindings,
                namespace_name.trim().to_owned(),
                "<namespace>".to_owned(),
                module_path,
                false,
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
/// `export { foo }` / `export { foo as bar }` clauses without a source,
/// CommonJS `module.exports = { ... }` object-literal properties that name a
/// local identifier, and CommonJS `exports.name = ...` /
/// `module.exports.name = ...` member assignments that name a local symbol.
/// Re-export forms (`export ... from "./module"`), default exports, computed
/// or string property access, and anonymous assigned values are not direct
/// named exports and are excluded.
pub(crate) fn javascript_named_export_names(
    root: Node<'_>,
    source: &str,
    check: Option<&dyn Fn() -> Result<()>>,
) -> Result<BTreeSet<String>> {
    Ok(javascript_direct_export_facts(root, source, check)?.0)
}

/// Collects the exported-name to local-name mappings for aliased direct
/// exports: `export { local as exported }` clauses, CommonJS
/// `module.exports = { exported: local }` pairs whose value is a local
/// identifier, and CommonJS `exports.exported = local` /
/// `module.exports.exported = local` assignments whose value is a differently
/// named local symbol. Namespace member resolution uses these so
/// `ns.exported(...)` resolves to the declaring local symbol.
pub(crate) fn javascript_export_local_names(
    root: Node<'_>,
    source: &str,
    check: Option<&dyn Fn() -> Result<()>>,
) -> Result<BTreeMap<String, String>> {
    Ok(javascript_direct_export_facts(root, source, check)?.1)
}

/// Returns the byte offset of the last top-level `module.exports = <value>`
/// replacement assignment, or `None` when the module never reassigns
/// `module.exports`. Once any replacement runs, the `exports` alias keeps
/// pointing at the original object, so member assignments on it no longer
/// reach the exported object, and the final replacement also shadows any
/// export object it replaced.
fn last_javascript_module_exports_replacement(
    root: Node<'_>,
    source: &str,
    check: Option<&dyn Fn() -> Result<()>>,
) -> Result<Option<usize>> {
    let mut last = None;
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if let Some(check) = check {
            check()?;
        }
        if statement.kind() != "expression_statement" {
            continue;
        }
        let Some(expression) = statement.named_child(0) else {
            continue;
        };
        if expression.kind() != "assignment_expression"
            || !is_javascript_module_exports_assignment(expression, source)?
        {
            continue;
        }
        last = Some(statement.start_byte());
    }
    Ok(last)
}

/// Returns whether `statement` is a top-level `module.exports = <value>`
/// replacement assignment.
fn is_javascript_module_exports_replacement_statement(
    statement: Node<'_>,
    source: &str,
) -> Result<bool> {
    if statement.kind() != "expression_statement" {
        return Ok(false);
    }
    let Some(expression) = statement.named_child(0) else {
        return Ok(false);
    };
    Ok(expression.kind() == "assignment_expression"
        && is_javascript_module_exports_assignment(expression, source)?)
}

/// Walks top-level statements once, collecting the direct named export names
/// and their exported-name to local-name alias mappings in a single pass.
fn javascript_direct_export_facts(
    root: Node<'_>,
    source: &str,
    check: Option<&dyn Fn() -> Result<()>>,
) -> Result<(BTreeSet<String>, BTreeMap<String, String>)> {
    let mut names = BTreeSet::new();
    let mut local_names = BTreeMap::new();
    // A top-level `module.exports = <value>` replacement abandons the
    // `exports` alias and any export object it replaced. `exports.*` member
    // assignments therefore never reach the exported object once a
    // replacement exists, and object-literal or `module.exports.*` member
    // exports that precede the final replacement are shadowed; only
    // `module.exports.*` member assignments after the final replacement
    // attach to the exported object.
    let last_module_exports_replacement =
        last_javascript_module_exports_replacement(root, source, check)?;
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if let Some(check) = check {
            check()?;
        }
        if statement.kind() == "expression_statement" {
            // CommonJS `module.exports = { ... }` object literals export their
            // identifier-valued property names, and `exports.name = ...` /
            // `module.exports.name = ...` member assignments export the
            // assigned local symbol; other assignment shapes fail closed in
            // the helpers below. A `module.exports = <value>` replacement
            // shadows the `exports` alias and earlier export objects, so only
            // the final replacement's object exports and the member
            // assignments that attach to it survive.
            if is_javascript_module_exports_replacement_statement(statement, source)? {
                if Some(statement.start_byte()) == last_module_exports_replacement {
                    commonjs_object_export_facts(statement, source, &mut names, &mut local_names)?;
                }
            } else {
                commonjs_exports_member_export_facts(
                    statement,
                    source,
                    last_module_exports_replacement,
                    &mut names,
                    &mut local_names,
                )?;
            }
            continue;
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
                let local_node = specifier.child_by_field_name("name");
                let exported_node = specifier.child_by_field_name("alias").or(local_node);
                if let Some(exported_node) = exported_node
                    && let Ok(exported_name) = node_text(exported_node, source)
                    && !exported_name.trim().is_empty()
                {
                    let exported_name = exported_name.trim().to_owned();
                    names.insert(exported_name.clone());
                    if let Some(local_node) = local_node
                        && let Ok(local_name) = node_text(local_node, source)
                        && !local_name.trim().is_empty()
                        && local_name.trim() != exported_name
                    {
                        local_names.insert(exported_name, local_name.trim().to_owned());
                    }
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
    Ok((names, local_names))
}

/// Records the names a CommonJS module exports through a direct
/// `module.exports = { ... }` object literal, plus exported-name to local-name
/// aliases for pairs whose value is a differently-named identifier. Shorthand
/// properties (`module.exports = { helper }`), same-named pairs
/// (`module.exports = { helper: helper }`), and aliased pairs
/// (`module.exports = { helper: localHelper }`) name an exported local symbol;
/// method definitions, computed and string keys, non-identifier values, and
/// non-object exports fail closed rather than guessing which local symbol a
/// differently-shaped property exports.
fn commonjs_object_export_facts(
    statement: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
    local_names: &mut BTreeMap<String, String>,
) -> Result<()> {
    let Some(assignment) = statement.named_child(0) else {
        return Ok(());
    };
    if assignment.kind() != "assignment_expression" {
        return Ok(());
    }
    let Some(left) = assignment.child_by_field_name("left") else {
        return Ok(());
    };
    if !is_module_exports_member(left, source)? {
        return Ok(());
    }
    let Some(right) = assignment.child_by_field_name("right") else {
        return Ok(());
    };
    if right.kind() != "object" {
        return Ok(());
    }
    let mut cursor = right.walk();
    for property in right.named_children(&mut cursor) {
        match property.kind() {
            "shorthand_property_identifier" => {
                let name = node_text(property, source)?.trim().to_owned();
                if !name.is_empty() {
                    names.insert(name);
                }
            }
            "pair" => {
                let Some(key) = property.child_by_field_name("key") else {
                    continue;
                };
                // Only static unquoted keys name a local symbol export;
                // computed (`[name]`) and string (`"name"`) keys fail closed.
                if key.kind() != "property_identifier" {
                    continue;
                }
                let Some(value) = property.child_by_field_name("value") else {
                    continue;
                };
                if value.kind() != "identifier" {
                    continue;
                }
                let key_name = node_text(key, source)?.trim().to_owned();
                let local_name = node_text(value, source)?.trim().to_owned();
                if key_name.is_empty() || local_name.is_empty() {
                    continue;
                }
                names.insert(key_name.clone());
                if key_name != local_name {
                    local_names.insert(key_name, local_name);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Records the names a CommonJS module exports through top-level member
/// assignments on the `exports` object (`exports.helper = ...`) or the
/// `module.exports` object (`module.exports.helper = ...`). The exported name
/// maps to the assigned value's local symbol when the value is an identifier
/// (`exports.helper = helper`), a named function/generator expression
/// (`exports.helper = function helper() {}`), or a named class expression;
/// anonymous functions/classes, arrow functions, calls, and other non-symbol
/// values fail closed because they name no module-level symbol. Computed and
/// string property access (`exports[name]`, `exports["name"]`) also fails
/// closed.
fn commonjs_exports_member_export_facts(
    statement: Node<'_>,
    source: &str,
    last_module_exports_replacement: Option<usize>,
    names: &mut BTreeSet<String>,
    local_names: &mut BTreeMap<String, String>,
) -> Result<()> {
    let Some(assignment) = statement.named_child(0) else {
        return Ok(());
    };
    if assignment.kind() != "assignment_expression" {
        return Ok(());
    }
    let Some(left) = assignment.child_by_field_name("left") else {
        return Ok(());
    };
    if left.kind() != "member_expression" {
        return Ok(());
    }
    let Some(object) = left.child_by_field_name("object") else {
        return Ok(());
    };
    // Only `exports.name` and `module.exports.name` mutate the exported
    // namespace; other member objects (and `exports[name]` / `exports["name"]`
    // which parse as subscript expressions) fail closed.
    // A top-level `module.exports = <value>` replacement abandons the
    // `exports` alias, so `exports.name = ...` assignments never reach the
    // exported object and must not count as exports; they fail closed instead
    // of guessing. `module.exports.name = ...` assignments attach to the
    // export object only when they run after the final replacement; earlier
    // ones mutate an object that gets replaced.
    let is_exports_alias =
        object.kind() == "identifier" && node_text(object, source)?.trim() == "exports";
    let is_module_exports_object = is_module_exports_member(object, source)?;
    if !is_exports_alias && !is_module_exports_object {
        return Ok(());
    }
    if is_exports_alias && last_module_exports_replacement.is_some() {
        return Ok(());
    }
    if is_module_exports_object
        && last_module_exports_replacement.is_some_and(|offset| statement.start_byte() < offset)
    {
        return Ok(());
    }
    let Some(property) = left.child_by_field_name("property") else {
        return Ok(());
    };
    if property.kind() != "property_identifier" {
        return Ok(());
    }
    let exported_name = node_text(property, source)?.trim().to_owned();
    if exported_name.is_empty() {
        return Ok(());
    }
    let Some(value) = assignment.child_by_field_name("right") else {
        return Ok(());
    };
    let Some(local_name) = javascript_assigned_export_local_name(value, source)? else {
        return Ok(());
    };
    names.insert(exported_name.clone());
    if exported_name != local_name {
        local_names.insert(exported_name, local_name);
    }
    Ok(())
}

/// Returns the local symbol name an assignment value names, or `None` when the
/// value is anonymous or not a module-level symbol. Identifiers and named
/// function/generator/class expressions name a symbol; anonymous functions,
/// arrow functions, calls, and other expressions fail closed.
fn javascript_assigned_export_local_name(value: Node<'_>, source: &str) -> Result<Option<String>> {
    match value.kind() {
        "identifier" => {
            let name = node_text(value, source)?.trim().to_owned();
            Ok((!name.is_empty()).then_some(name))
        }
        "function_expression" | "generator_function" | "class" => {
            let Some(name) = value.child_by_field_name("name") else {
                return Ok(None);
            };
            if name.is_missing() {
                return Ok(None);
            }
            let name = node_text(name, source)?.trim().to_owned();
            Ok((!name.is_empty()).then_some(name))
        }
        _ => Ok(None),
    }
}

/// Returns whether `node` is a `module.exports` member expression.
fn is_module_exports_member(node: Node<'_>, source: &str) -> Result<bool> {
    if node.kind() != "member_expression" {
        return Ok(false);
    }
    let Some(object) = node.child_by_field_name("object") else {
        return Ok(false);
    };
    let Some(property) = node.child_by_field_name("property") else {
        return Ok(false);
    };
    Ok(node_text(object, source)?.trim() == "module"
        && node_text(property, source)?.trim() == "exports")
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

/// Records TypeScript `import name = require("./module")` bindings as module
/// namespaces so member calls (`name.helper(...)`) and namespace-object calls
/// (`name(...)`) resolve through the existing machinery. The module specifier
/// lives on the `import_require_clause`; dynamic specifiers and unresolvable
/// local modules fail closed through the shared binding insert.
fn collect_import_equals_namespace_bindings(
    node: Node<'_>,
    path: &Path,
    source: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    bindings: &mut BTreeMap<String, JavaScriptNamedModuleBinding>,
) -> Result<()> {
    let mut cursor = node.walk();
    let Some(clause) = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "import_require_clause")
    else {
        return Ok(());
    };
    let mut clause_cursor = clause.walk();
    let mut local_name = None;
    let mut specifier = None;
    for child in clause.named_children(&mut clause_cursor) {
        match child.kind() {
            "identifier" => {
                let name = node_text(child, source)?.trim().to_owned();
                if !name.is_empty() {
                    local_name = Some(name);
                }
            }
            "string" => {
                specifier = javascript_string_literal(child, source)?;
            }
            _ => {}
        }
    }
    let Some(local_name) = local_name else {
        return Ok(());
    };
    let module_path = specifier.and_then(|specifier| {
        resolve_local_javascript_module_path_with_overrides(path, &specifier, file_overrides)
    });
    insert_javascript_module_binding(
        bindings,
        local_name,
        "<namespace>".to_owned(),
        module_path,
        false,
    );
    Ok(())
}

/// Returns the local declaration name of a module's default export when it can
/// be resolved conservatively: a named `export default function`/`class`
/// declaration, `export default <identifier>` naming a declared module-level
/// symbol, `export { localName as default }`, or a CommonJS `exports.default =
/// ...` / `module.exports.default = ...` member assignment naming a module-level
/// symbol. Anonymous default exports, expression defaults that do not name a
/// declaration, re-export forms with a source clause, and modules with
/// conflicting or absent default exports fail closed (`None`).
pub(crate) fn javascript_module_default_export_local_name(
    root: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let mut esm_names = BTreeSet::new();
    let mut cjs_default_names = BTreeSet::new();
    let last_module_exports_replacement =
        last_javascript_module_exports_replacement(root, source, None)?;
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if statement.kind() == "expression_statement" {
            // A top-level `module.exports = <value>` assignment replaces the
            // exported object; the `exports` alias keeps pointing at the old
            // object, so `exports.default` member assignments no longer reach
            // the exported object and `module.exports.default` assignments
            // only count when they attach to the final replacement.
            if is_javascript_module_exports_replacement_statement(statement, source)? {
                continue;
            }
            // CommonJS `exports.default = helper` / `module.exports.default =
            // helper` member assignments expose a `default` member equivalent
            // to the module's default export for interop consumers.
            if let Some(name) = javascript_cjs_default_member_assigned_name(
                statement,
                source,
                last_module_exports_replacement,
            )? {
                cjs_default_names.insert(name);
            }
            continue;
        }
        if statement.kind() != "export_statement" {
            continue;
        }
        if let Some(name) = javascript_default_export_name(statement, source)? {
            esm_names.insert(name);
        }
    }
    let mut names = esm_names;
    names.extend(cjs_default_names);
    // A module may declare at most one default export; anything else fails
    // closed instead of guessing.
    Ok((names.len() == 1)
        .then(|| names.iter().next().cloned())
        .flatten())
}

/// Returns the local symbol name a top-level CommonJS `exports.default = ...`
/// or `module.exports.default = ...` member assignment names, or `None` for
/// other statement shapes. The assigned value must name a module-level symbol;
/// anonymous functions/classes, arrow functions, calls, and other non-symbol
/// values fail closed.
fn javascript_cjs_default_member_assigned_name(
    statement: Node<'_>,
    source: &str,
    last_module_exports_replacement: Option<usize>,
) -> Result<Option<String>> {
    let expression = if statement.kind() == "expression_statement" {
        statement.named_child(0)
    } else {
        None
    };
    let Some(assignment) = expression else {
        return Ok(None);
    };
    if assignment.kind() != "assignment_expression" {
        return Ok(None);
    }
    let Some(left) = assignment.child_by_field_name("left") else {
        return Ok(None);
    };
    if left.kind() != "member_expression" {
        return Ok(None);
    }
    let Some(object) = left.child_by_field_name("object") else {
        return Ok(None);
    };
    // Only `exports.default` and `module.exports.default` mutate the exported
    // namespace's default member; other member objects (and `exports["default"]`
    // which parses as a subscript expression) fail closed. A `module.exports =
    // <value>` replacement abandons the `exports` alias, and `module.exports`
    // member assignments only attach to the final export object when they run
    // after the replacement, so shadowed defaults name no symbol.
    let is_exports_alias =
        object.kind() == "identifier" && node_text(object, source)?.trim() == "exports";
    let is_module_exports_object = is_module_exports_member(object, source)?;
    if !is_exports_alias && !is_module_exports_object {
        return Ok(None);
    }
    if is_exports_alias && last_module_exports_replacement.is_some() {
        return Ok(None);
    }
    if is_module_exports_object
        && last_module_exports_replacement.is_some_and(|offset| statement.start_byte() < offset)
    {
        return Ok(None);
    }
    let Some(property) = left.child_by_field_name("property") else {
        return Ok(None);
    };
    if property.kind() != "property_identifier" || node_text(property, source)?.trim() != "default"
    {
        return Ok(None);
    }
    let Some(value) = assignment.child_by_field_name("right") else {
        return Ok(None);
    };
    javascript_assigned_export_local_name(value, source)
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

/// Returns the local name of the callable a CommonJS module exports through a
/// top-level `module.exports = ...` assignment when it can be resolved
/// conservatively: a named function expression (`module.exports = function
/// helper() {}`) or an identifier naming a module-level callable declaration
/// (`function helper() {}` or `const helper = () => {}`). Anonymous function
/// expressions, non-function exports, and modules with conflicting or absent
/// `module.exports` assignments fail closed (`None`). ESM-only modules have no
/// such assignment and also return `None`, so namespace-object calls stay
/// fail-closed for them.
pub(crate) fn javascript_module_callable_export_local_name(
    root: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let mut names = BTreeSet::new();
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if let Some(name) = javascript_module_exports_callable_name(statement, source, root)? {
            names.insert(name);
        }
    }
    // A module may export at most one callable value; anything else fails
    // closed instead of guessing.
    Ok((names.len() == 1)
        .then(|| names.iter().next().cloned())
        .flatten())
}

fn javascript_module_exports_callable_name(
    statement: Node<'_>,
    source: &str,
    root: Node<'_>,
) -> Result<Option<String>> {
    let expression = if statement.kind() == "expression_statement" {
        statement.named_child(0)
    } else {
        None
    };
    let Some(assignment) = expression else {
        return Ok(None);
    };
    if assignment.kind() != "assignment_expression"
        || !is_javascript_module_exports_assignment(assignment, source)?
    {
        return Ok(None);
    }
    let Some(value) = assignment.child_by_field_name("right") else {
        return Ok(None);
    };
    match value.kind() {
        // `module.exports = helper;` names a module-level symbol; only a
        // callable declaration makes the module itself callable.
        "identifier" => {
            let name = node_text(value, source)?.trim().to_owned();
            if name.is_empty() || !javascript_module_level_callable_declared(root, source, &name)? {
                return Ok(None);
            }
            Ok(Some(name))
        }
        // A named function expression carries the exported callable's local
        // name; anonymous expressions fail closed because they name no symbol.
        "function_expression" | "generator_function" => {
            let Some(name) = value.child_by_field_name("name") else {
                return Ok(None);
            };
            if name.is_missing() {
                return Ok(None);
            }
            let name = node_text(name, source)?.trim().to_owned();
            Ok((!name.is_empty()).then_some(name))
        }
        _ => Ok(None),
    }
}

fn is_javascript_module_exports_assignment(assignment: Node<'_>, source: &str) -> Result<bool> {
    let Some(left) = assignment.child_by_field_name("left") else {
        return Ok(false);
    };
    if left.kind() != "member_expression" {
        return Ok(false);
    }
    let Some(object) = left.child_by_field_name("object") else {
        return Ok(false);
    };
    if object.kind() != "identifier" || node_text(object, source)?.trim() != "module" {
        return Ok(false);
    }
    let Some(property) = left.child_by_field_name("property") else {
        return Ok(false);
    };
    Ok(
        property.kind() == "property_identifier"
            && node_text(property, source)?.trim() == "exports",
    )
}

fn javascript_module_level_callable_declared(
    root: Node<'_>,
    source: &str,
    name: &str,
) -> Result<bool> {
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        let declared_name = match statement.kind() {
            "function_declaration" | "generator_function_declaration" => {
                statement.child_by_field_name("name").and_then(|name| {
                    node_text(name, source)
                        .ok()
                        .map(|text| text.trim().to_owned())
                })
            }
            "lexical_declaration" | "variable_declaration" => {
                let mut declarator_cursor = statement.walk();
                let mut declared_name = None;
                for declarator in statement.named_children(&mut declarator_cursor) {
                    if declarator.kind() != "variable_declarator" {
                        continue;
                    }
                    if !declarator
                        .child_by_field_name("value")
                        .is_some_and(|value| {
                            matches!(value.kind(), "arrow_function" | "function_expression")
                        })
                    {
                        continue;
                    }
                    if let Some(name_node) = declarator.child_by_field_name("name")
                        && let Ok(text) = node_text(name_node, source)
                        && !text.trim().is_empty()
                    {
                        declared_name = Some(text.trim().to_owned());
                        break;
                    }
                }
                declared_name
            }
            _ => None,
        };
        if declared_name.as_deref() == Some(name) {
            return Ok(true);
        }
    }
    Ok(false)
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

/// Returns the local module paths a CommonJS module re-exports wholesale
/// through top-level `module.exports = require("./module")` assignments. The
/// module's namespace is the target module's export object, so named members,
/// default members, and callable-object calls resolve within the target.
/// Dynamic require arguments, non-require values, and unresolvable local
/// specifiers fail closed and contribute no path.
pub(crate) fn javascript_module_reexport_module_paths_with_overrides_and_check(
    path: &Path,
    root: Node<'_>,
    source: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    check: Option<&dyn Fn() -> Result<()>>,
) -> Result<BTreeSet<PathBuf>> {
    let mut module_paths = BTreeSet::new();
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if let Some(check) = check {
            check()?;
        }
        if let Some(module_path) =
            javascript_module_reexport_target(statement, path, source, file_overrides)?
        {
            module_paths.insert(module_path);
        }
    }
    Ok(module_paths)
}

/// Returns the local module path a top-level statement re-exports wholesale
/// through `module.exports = require("./module")`, or `None` for other shapes.
fn javascript_module_reexport_target(
    statement: Node<'_>,
    path: &Path,
    source: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<Option<PathBuf>> {
    let expression = if statement.kind() == "expression_statement" {
        statement.named_child(0)
    } else {
        None
    };
    let Some(assignment) = expression else {
        return Ok(None);
    };
    if assignment.kind() != "assignment_expression" {
        return Ok(None);
    }
    let Some(left) = assignment.child_by_field_name("left") else {
        return Ok(None);
    };
    if !is_module_exports_member(left, source)? {
        return Ok(None);
    }
    let Some(right) = assignment.child_by_field_name("right") else {
        return Ok(None);
    };
    let Some(specifier) = direct_require_specifier(right, source)? else {
        return Ok(None);
    };
    Ok(resolve_local_javascript_module_path_with_overrides(
        path,
        &specifier,
        file_overrides,
    ))
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
        // TypeScript `import name = require("./module")` carries its specifier
        // as a string child of the clause rather than a field on the import
        // statement.
        "import_require_clause" => {
            if let Some(source_node) = first_string_child(node)
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

/// Returns the `(local_name, imported_name, module_path)` bindings introduced
/// by `const`/`let`/`var` declarations whose initializer is a direct
/// `require("./module")` call. Identifier patterns bind the whole module
/// namespace (`const ns = require(...)`); object patterns bind named members
/// (`const { helper } = require(...)` and `const { helper: alias } = ...`).
/// Dynamic require arguments, unsupported patterns, and unresolvable module
/// specifiers fail closed: the caller records an unresolved binding.
fn javascript_require_declaration_bindings(
    node: Node<'_>,
    path: &Path,
    source: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<Vec<(String, String, Option<PathBuf>)>> {
    let mut bindings = Vec::new();
    let mut cursor = node.walk();
    for declarator in node
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "variable_declarator")
    {
        let Some(value) = declarator.child_by_field_name("value") else {
            continue;
        };
        if value.kind() != "call_expression" {
            continue;
        }
        let Some(specifier) = direct_require_specifier(value, source)? else {
            continue;
        };
        let module_path =
            resolve_local_javascript_module_path_with_overrides(path, &specifier, file_overrides);
        let Some(pattern) = declarator.child_by_field_name("name") else {
            continue;
        };
        let mut pattern_bindings = Vec::new();
        collect_require_pattern_bindings(pattern, source, &mut pattern_bindings)?;
        for (local_name, imported_name) in pattern_bindings {
            bindings.push((local_name, imported_name, module_path.clone()));
        }
    }
    Ok(bindings)
}

/// Collects `(local_name, imported_name)` pairs from a variable declarator
/// pattern bound to a `require` call. An identifier binds the module namespace
/// (`"<namespace>"`); an object pattern binds each simple named member,
/// keeping the imported spelling so aliases resolve to the right symbol.
/// Defaults, rest elements, nested patterns, and array patterns are left
/// unbound and fail closed.
fn collect_require_pattern_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut Vec<(String, String)>,
) -> Result<()> {
    match node.kind() {
        "identifier" => {
            let local_name = node_text(node, source)?.trim().to_owned();
            if !local_name.is_empty() {
                bindings.push((local_name, "<namespace>".to_owned()));
            }
        }
        "object_pattern" => {
            let mut cursor = node.walk();
            for member in node.named_children(&mut cursor) {
                match member.kind() {
                    "shorthand_property_identifier_pattern" => {
                        let local_name = node_text(member, source)?.trim().to_owned();
                        if !local_name.is_empty() {
                            bindings.push((local_name.clone(), local_name));
                        }
                    }
                    "pair_pattern" => {
                        let Some(key) = member.child_by_field_name("key") else {
                            continue;
                        };
                        let Some(value) = member.child_by_field_name("value") else {
                            continue;
                        };
                        if value.kind() != "identifier" {
                            continue;
                        }
                        let imported_name = node_text(key, source)?.trim().to_owned();
                        let local_name = node_text(value, source)?.trim().to_owned();
                        if !imported_name.is_empty() && !local_name.is_empty() {
                            bindings.push((local_name, imported_name));
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
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
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    use anyhow::bail;

    use super::{
        javascript_export_local_names, javascript_module_callable_export_local_name,
        javascript_module_default_export_local_name,
        javascript_module_reexport_module_paths_with_overrides_and_check,
        javascript_named_export_names,
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
            // CommonJS interop default members name the assigned symbol.
            (
                "function helper() {}\nexports.default = helper;\n",
                Some("helper"),
            ),
            (
                "function helper() {}\nmodule.exports.default = helper;\n",
                Some("helper"),
            ),
            (
                "function helper() {}\nexports.default = helper;\nmodule.exports.default = helper;\n",
                Some("helper"),
            ),
            // The same symbol named by both ESM and CommonJS forms is one
            // default; competing symbols are ambiguous.
            (
                "function helper() {}\nexport default helper;\nexports.default = helper;\n",
                Some("helper"),
            ),
            (
                "function helper() {}\nfunction other() {}\nexport default helper;\nexports.default = other;\n",
                None,
            ),
            // Anonymous and non-symbol assigned values fail closed.
            ("exports.default = function () {};\n", None),
            ("exports.default = () => 1;\n", None),
            ("exports.default = 42;\n", None),
            // Only the `default` member counts as the default export.
            ("function helper() {}\nexports.helper = helper;\n", None),
            // A `module.exports = <value>` replacement shadows member
            // assignments, so the .default member stops naming a default.
            (
                "function helper() {}\nexports.default = helper;\nmodule.exports = function app() {}\n",
                None,
            ),
            (
                "function helper() {}\nexports.default = helper;\nmodule.exports = { other: 1 };\n",
                None,
            ),
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
    fn resolves_module_callable_export_local_names_conservatively() {
        for (source, expected) in [
            (
                "function helper() {}\nmodule.exports = helper;\n",
                Some("helper"),
            ),
            (
                "const helper = () => 1;\nmodule.exports = helper;\n",
                Some("helper"),
            ),
            (
                "const helper = function () {}\nmodule.exports = helper;\n",
                Some("helper"),
            ),
            (
                "function helper() {}\nmodule.exports = function helper() {}\n",
                Some("helper"),
            ),
            (
                "module.exports = function* generate() {}\n",
                Some("generate"),
            ),
            ("export default function helper() {}\n", None),
            ("export function helper() {}\n", None),
            ("module.exports = function () {}\n", None),
            ("module.exports = () => 1;\n", None),
            ("function helper() {}\nmodule.exports = { helper };\n", None),
            ("const helper = 42;\nmodule.exports = helper;\n", None),
            (
                "function first() {}\nfunction second() {}\nmodule.exports = first;\nmodule.exports = second;\n",
                None,
            ),
            ("module.exports.helper = function helper() {}\n", None),
            ("exports.helper = function helper() {}\n", None),
        ] {
            let document = parse_document(Path::new("sample.ts"), source).unwrap();
            assert_eq!(
                javascript_module_callable_export_local_name(document.tree.root_node(), source)
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

    #[test]
    fn binds_namespace_reexports_to_local_modules() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-reexports-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        let helper_path = crate::language::normalize_path(&helper);
        std::fs::write(&helper, "export function helper() {}\n").unwrap();
        let source = "export * as ns from \"./helper\";\nexport * from \"./other\";\n";
        let document = parse_document(&bridge, source).unwrap();

        let reexports = javascript_named_reexport_module_paths_with_overrides_and_check(
            &bridge,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        let ns = reexports
            .get("ns")
            .expect("namespace re-export should be recorded");
        assert_eq!(ns.imported_name, "<namespace>");
        assert!(!ns.unresolved);
        assert_eq!(
            ns.module_paths
                .iter()
                .map(|path| crate::language::normalize_path(path))
                .collect::<Vec<_>>(),
            vec![helper_path]
        );
        assert!(!reexports.contains_key("other"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn binds_require_namespace_and_destructured_members_to_local_modules() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-bindings-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let importer = root.join("caller.ts");
        let helper = root.join("helper.ts");
        let helper_path = helper.clone();
        std::fs::write(&helper, "export function helper() {}\n").unwrap();
        let source =
            "const ns = require(\"./helper\");\nconst { helper, other } = require(\"./helper\");\n";
        let document = parse_document(&importer, source).unwrap();

        let bindings = javascript_named_import_module_paths_with_overrides_and_check(
            &importer,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        let namespace = bindings.get("ns").expect("require namespace binding");
        assert_eq!(namespace.imported_name, "<namespace>");
        assert!(!namespace.unresolved);
        assert_eq!(
            namespace.module_paths,
            BTreeSet::from([helper_path.clone()])
        );
        let destructured = bindings.get("helper").expect("require member binding");
        assert_eq!(destructured.imported_name, "helper");
        assert!(!destructured.unresolved);
        assert_eq!(
            destructured.module_paths,
            BTreeSet::from([helper_path.clone()])
        );
        assert_eq!(
            bindings.get("other").map(|binding| &binding.module_paths),
            Some(&BTreeSet::from([helper_path]))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn binds_require_aliased_members_to_their_imported_spelling() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-alias-bindings-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let importer = root.join("caller.ts");
        let helper = root.join("helper.ts");
        let helper_path = helper.clone();
        std::fs::write(&helper, "export function helper() {}\n").unwrap();
        let source = "const { helper: bound } = require(\"./helper\");\n";
        let document = parse_document(&importer, source).unwrap();

        let bindings = javascript_named_import_module_paths_with_overrides_and_check(
            &importer,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        let binding = bindings.get("bound").expect("aliased require binding");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(binding.module_paths, BTreeSet::from([helper_path]));
        assert!(!bindings.contains_key("helper"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_require_bindings_fail_closed_for_dynamic_and_missing_modules() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-fail-closed-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let importer = root.join("caller.ts");
        let source =
            "const dynamic = require(moduleName);\nconst missing = require(\"./missing\");\n";
        let document = parse_document(&importer, source).unwrap();

        let bindings = javascript_named_import_module_paths_with_overrides_and_check(
            &importer,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        assert!(
            !bindings.contains_key("dynamic"),
            "dynamic require arguments must not create bindings"
        );
        let missing = bindings
            .get("missing")
            .expect("missing local module still records a binding");
        assert!(missing.unresolved);
        assert!(missing.module_paths.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_require_unsupported_patterns_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-unsupported-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let importer = root.join("caller.ts");
        let helper = root.join("helper.ts");
        std::fs::write(&helper, "export function helper() {}\n").unwrap();
        let source = "const [first] = require(\"./helper\");\nconst { helper: bound = fallback } = require(\"./helper\");\nconst { ...rest } = require(\"./helper\");\nconst { nested: { deep } } = require(\"./helper\");\n";
        let document = parse_document(&importer, source).unwrap();

        let bindings = javascript_named_import_module_paths_with_overrides_and_check(
            &importer,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        assert!(
            bindings.is_empty(),
            "unsupported require patterns must fail closed, bindings: {bindings:?}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn collects_final_commonjs_object_export_names_conservatively() {
        // The final `module.exports = { ... }` replacement shadows any export
        // object it replaced, so only the final object's properties survive;
        // aliased pairs still map through the exported-name to local-name
        // mapping.
        let source =
            "module.exports = { helper };\nmodule.exports = { first: localFirst, second };\n";
        let document = parse_document(Path::new("sample.cjs"), source).unwrap();

        let names = javascript_named_export_names(document.tree.root_node(), source, None).unwrap();
        assert_eq!(
            names,
            BTreeSet::from(["first".to_string(), "second".to_string()])
        );
        let local_names =
            javascript_export_local_names(document.tree.root_node(), source, None).unwrap();
        assert_eq!(
            local_names,
            BTreeMap::from([("first".to_string(), "localFirst".to_string())])
        );
    }

    #[test]
    fn keeps_expression_and_non_object_exports_fail_closed() {
        for source in [
            // Method shorthand and computed/string keys do not name a local
            // identifier export.
            "module.exports = { helper() {} };\n",
            "module.exports = { [name]: helper };\n",
            "module.exports = { \"helper\": helper };\n",
            // Non-identifier values and non-object module.exports assignments
            // export nothing through this conservative path.
            "module.exports = { helper: function () {} };\n",
            "module.exports = helper;\n",
            "const value = 1;\n",
        ] {
            let document = parse_document(Path::new("sample.cjs"), source).unwrap();
            let names =
                javascript_named_export_names(document.tree.root_node(), source, None).unwrap();
            assert!(
                names.is_empty(),
                "source {source:?} must fail closed, names: {names:?}"
            );
        }
    }

    #[test]
    fn collects_commonjs_object_export_local_aliases() {
        let source = "module.exports = { helper: localHelper, plain };\n";
        let document = parse_document(Path::new("sample.cjs"), source).unwrap();
        let root = document.tree.root_node();

        let names = javascript_named_export_names(root, source, None).unwrap();
        assert_eq!(
            names,
            BTreeSet::from(["helper".to_string(), "plain".to_string()])
        );
        let local_names = javascript_export_local_names(root, source, None).unwrap();
        assert_eq!(
            local_names,
            BTreeMap::from([("helper".to_string(), "localHelper".to_string())])
        );
    }

    #[test]
    fn collects_es_export_local_aliases() {
        let source = "function localHelper() {}\nexport { localHelper as helper };\n";
        let document = parse_document(Path::new("sample.ts"), source).unwrap();
        let root = document.tree.root_node();

        let names = javascript_named_export_names(root, source, None).unwrap();
        assert_eq!(names, BTreeSet::from(["helper".to_string()]));
        let local_names = javascript_export_local_names(root, source, None).unwrap();
        assert_eq!(
            local_names,
            BTreeMap::from([("helper".to_string(), "localHelper".to_string())])
        );
    }

    #[test]
    fn collects_commonjs_exports_member_export_names() {
        let source = r#"
function helper(value) { return value; }
exports.helper = helper;
module.exports.direct = function direct(value) { return value; };
exports.alias = localHelper;
exports.generator = function* generator() { yield 1; };
exports.Klass = class Klass {};
"#;
        let document = parse_document(Path::new("sample.cjs"), source).unwrap();
        let root = document.tree.root_node();

        let names = javascript_named_export_names(root, source, None).unwrap();
        assert_eq!(
            names,
            BTreeSet::from([
                "helper".to_string(),
                "direct".to_string(),
                "alias".to_string(),
                "generator".to_string(),
                "Klass".to_string(),
            ])
        );
        let local_names = javascript_export_local_names(root, source, None).unwrap();
        assert_eq!(
            local_names,
            BTreeMap::from([("alias".to_string(), "localHelper".to_string())])
        );
    }

    #[test]
    fn keeps_anonymous_computed_and_non_symbol_exports_member_fail_closed() {
        for source in [
            // Anonymous values name no module-level symbol.
            "exports.helper = () => {};\n",
            "exports.helper = function () {};\n",
            // Computed and string property access do not expose a static name.
            "exports[helper] = helper;\n",
            "exports[\"helper\"] = helper;\n",
            // Non-symbol values and non-exports assignments export nothing.
            "exports.helper = other.helper;\n",
            "exports.helper = buildHelper();\n",
            "const value = 1;\n",
        ] {
            let document = parse_document(Path::new("sample.cjs"), source).unwrap();
            let root = document.tree.root_node();
            let names = javascript_named_export_names(root, source, None).unwrap();
            assert!(
                names.is_empty(),
                "source {source:?} must fail closed, names: {names:?}"
            );
            let local_names = javascript_export_local_names(root, source, None).unwrap();
            assert!(
                local_names.is_empty(),
                "source {source:?} must fail closed, local_names: {local_names:?}"
            );
        }
    }

    #[test]
    fn shadows_exports_alias_members_after_module_exports_replacement() {
        // A `module.exports = <value>` replacement abandons the `exports`
        // alias, so `exports.*` member assignments (before or after the
        // replacement) never reach the exported object and fail closed.
        for source in [
            "exports.helper = helper;\nmodule.exports = app;\n",
            "module.exports = app;\nexports.helper = helper;\n",
        ] {
            let document = parse_document(Path::new("sample.cjs"), source).unwrap();
            let root = document.tree.root_node();
            let names = javascript_named_export_names(root, source, None).unwrap();
            assert!(
                names.is_empty(),
                "source {source:?} must fail closed, names: {names:?}"
            );
            let local_names = javascript_export_local_names(root, source, None).unwrap();
            assert!(
                local_names.is_empty(),
                "source {source:?} must fail closed, local_names: {local_names:?}"
            );
        }
        // The replacement's own object exports survive; only the abandoned
        // `exports` alias members are shadowed.
        let source = "exports.helper = helper;\nmodule.exports = { app };\n";
        let document = parse_document(Path::new("sample.cjs"), source).unwrap();
        let root = document.tree.root_node();
        let names = javascript_named_export_names(root, source, None).unwrap();
        assert_eq!(names, BTreeSet::from(["app".to_string()]));
        let local_names = javascript_export_local_names(root, source, None).unwrap();
        assert!(local_names.is_empty());
    }

    #[test]
    fn keeps_module_exports_member_assignments_attached_after_final_replacement() {
        // The express-style pattern assigns members onto the final
        // `module.exports` object after the callable replacement, so those
        // members are real exports; assignments before the final replacement
        // mutate an object that gets replaced and fail closed.
        let express = "function app() {}\nfunction extraFn() {}\nmodule.exports = app;\nmodule.exports.extra = extraFn;\n";
        let document = parse_document(Path::new("sample.cjs"), express).unwrap();
        let root = document.tree.root_node();
        let names = javascript_named_export_names(root, express, None).unwrap();
        assert_eq!(names, BTreeSet::from(["extra".to_string()]));
        let local_names = javascript_export_local_names(root, express, None).unwrap();
        assert_eq!(
            local_names,
            BTreeMap::from([("extra".to_string(), "extraFn".to_string())])
        );

        let shadowed = "function app() {}\nfunction extraFn() {}\nmodule.exports.extra = extraFn;\nmodule.exports = app;\n";
        let document = parse_document(Path::new("sample.cjs"), shadowed).unwrap();
        let root = document.tree.root_node();
        let names = javascript_named_export_names(root, shadowed, None).unwrap();
        assert!(
            names.is_empty(),
            "pre-replacement member assignments must fail closed, names: {names:?}"
        );
        let local_names = javascript_export_local_names(root, shadowed, None).unwrap();
        assert!(local_names.is_empty());
    }

    #[test]
    fn shadows_default_member_before_and_keeps_default_member_after_replacement() {
        // `exports.default` is abandoned with the `exports` alias once a
        // replacement exists, while `module.exports.default` attaches to the
        // final export object when assigned after the replacement.
        let shadowed = "function helper() {}\nfunction app() {}\nexports.default = helper;\nmodule.exports = app;\n";
        let document = parse_document(Path::new("sample.cjs"), shadowed).unwrap();
        let root = document.tree.root_node();
        assert_eq!(
            javascript_module_default_export_local_name(root, shadowed).unwrap(),
            None,
            "shadowed exports.default must not name the default export"
        );

        let attached =
            "function helper() {}\nmodule.exports = app;\nmodule.exports.default = helper;\n";
        let document = parse_document(Path::new("sample.cjs"), attached).unwrap();
        let root = document.tree.root_node();
        assert_eq!(
            javascript_module_default_export_local_name(root, attached).unwrap(),
            Some("helper".to_owned()),
            "module.exports.default after the final replacement names the default"
        );
    }

    #[test]
    fn collects_typescript_import_equals_specifiers_and_namespace_bindings() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-import-equals-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let helper = root.join("helper.ts");
        let helper_path = crate::language::normalize_path(&helper);
        std::fs::write(&helper, "export function helper() {}\n").unwrap();
        let source = "import ns = require(\"./helper\");\n";
        let document = parse_document(&module, source).unwrap();

        assert_eq!(
            javascript_static_module_specifiers(document.tree.root_node(), source).unwrap(),
            BTreeSet::from(["./helper".to_string()])
        );
        let imports = javascript_named_import_module_paths_with_overrides_and_check(
            &module,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        let binding = imports.get("ns").unwrap();
        assert_eq!(binding.imported_name, "<namespace>");
        assert!(!binding.unresolved);
        assert_eq!(
            binding
                .module_paths
                .iter()
                .map(|path| crate::language::normalize_path(path))
                .collect::<Vec<_>>(),
            vec![helper_path]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_typescript_import_equals_non_local_specifiers_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-import-equals-package-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let source = "import pkg = require(\"package-name\");\n";
        let document = parse_document(&module, source).unwrap();

        let imports = javascript_named_import_module_paths_with_overrides_and_check(
            &module,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        let binding = imports.get("pkg").unwrap();
        assert_eq!(binding.imported_name, "<namespace>");
        assert!(binding.unresolved);
        assert!(binding.module_paths.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn collects_module_reexport_require_targets() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-module-reexport-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let module = root.join("bridge.cjs");
        let impl_path = root.join("impl.cjs");
        std::fs::write(&impl_path, "exports.helper = function helper() {}\n").unwrap();
        let source = "module.exports = require(\"./impl.cjs\");\n";
        let document = parse_document(&module, source).unwrap();

        let paths = javascript_module_reexport_module_paths_with_overrides_and_check(
            &module,
            document.tree.root_node(),
            source,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            paths
                .iter()
                .map(|path| crate::language::normalize_path(path))
                .collect::<Vec<_>>(),
            vec![crate::language::normalize_path(&impl_path)]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_non_require_module_exports_shapes_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-module-reexport-shapes-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let module = root.join("bridge.cjs");
        for source in [
            // Non-require values and member assignments do not re-export the
            // whole module.
            "module.exports = { helper };\n",
            "module.exports = function helper() {}\n",
            "module.exports = helper;\n",
            "exports.helper = require(\"./impl.cjs\");\n",
            // Dynamic require arguments fail closed.
            "module.exports = require(specifier);\n",
        ] {
            let document = parse_document(&module, source).unwrap();
            let paths = javascript_module_reexport_module_paths_with_overrides_and_check(
                &module,
                document.tree.root_node(),
                source,
                None,
                None,
            )
            .unwrap();
            assert!(
                paths.is_empty(),
                "source {source:?} must fail closed, paths: {paths:?}"
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
