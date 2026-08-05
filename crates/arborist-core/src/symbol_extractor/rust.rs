use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path, rust_direct_module_candidate_paths};
use crate::semantic::rust::{
    is_rust_symbol_node, rust_parameters, rust_return_type, rust_semantic_path, rust_signature,
    rust_symbol_name,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::symbol_reference_compat::reference_facts_from_legacy;
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_rust_symbols_with_deadline(
    path: &Path,
    source: &str,
    root: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();
    collect_symbols(path, source, root, deadline, &mut symbols)?;
    Ok(symbols)
}

fn collect_symbols(
    path: &Path,
    source: &str,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
    symbols: &mut Vec<IndexedSymbol>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting Rust symbols")?;
    }
    if is_rust_symbol_node(node)
        && let Some(symbol) = indexed_symbol(path, source, node, deadline)?
    {
        symbols.push(symbol);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_symbols(path, source, child, deadline, symbols)?;
    }
    Ok(())
}

fn indexed_symbol(
    path: &Path,
    source: &str,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<IndexedSymbol>> {
    let Some(name) = rust_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = rust_semantic_path(node, source, &name)? else {
        return Ok(None);
    };
    let scope_path = semantic_path
        .rsplit_once("::")
        .map(|(scope_path, _)| scope_path.to_string());
    let references_by_name = collect_direct_local_calls(path, node, source, deadline)?;
    let call_arities_by_name = BTreeMap::new();

    Ok(Some(IndexedSymbol {
        symbol_id: semantic_path.clone(),
        base_name: symbol_base_name(&semantic_path),
        semantic_path,
        scope_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: rust_signature(node, source),
        is_overload: false,
        parameters: rust_parameters(node, source),
        return_type: rust_return_type(node, source),
        docstring: None,
        reference_facts: reference_facts_from_legacy(&references_by_name, &call_arities_by_name),
        references_by_name,
        call_arities_by_name,
    }))
}

fn collect_direct_local_calls(
    path: &Path,
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<BTreeSet<String>> {
    if symbol_node.kind() != "function_item" {
        return Ok(BTreeSet::new());
    }
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(BTreeSet::new());
    };
    let local_functions = local_module_function_paths(symbol_node, source)?.unwrap_or_default();
    let local_function_names =
        local_module_function_names(symbol_node, source)?.unwrap_or_default();
    let qualified_functions = source_file_module_function_paths(symbol_node, source)?;
    let imported_functions = source_file_imported_function_paths(symbol_node, source)?;
    let out_of_line_modules = source_file_out_of_line_module_names(path, symbol_node, source)?;
    if local_functions.is_empty()
        && imported_functions.is_empty()
        && qualified_functions.is_empty()
        && out_of_line_modules.is_empty()
    {
        return Ok(BTreeSet::new());
    }

    let mut bindings = BTreeSet::new();
    collect_function_bindings(symbol_node, source, &mut bindings)?;
    let context = RustDirectCallContext {
        source,
        deadline,
        local_functions: &local_functions,
        local_function_names: &local_function_names,
        imported_functions: &imported_functions,
        qualified_functions: &qualified_functions,
        out_of_line_modules: &out_of_line_modules,
        module_components: rust_inline_module_path_components(symbol_node, source)?,
        bindings: &bindings,
    };
    let mut references = BTreeSet::new();
    collect_direct_local_calls_from_node(body, &context, &mut references)?;
    Ok(references)
}

struct RustDirectCallContext<'a> {
    source: &'a str,
    deadline: Option<&'a WorkspaceScanDeadline>,
    local_functions: &'a BTreeMap<String, String>,
    local_function_names: &'a BTreeSet<String>,
    imported_functions: &'a BTreeMap<String, String>,
    qualified_functions: &'a BTreeMap<String, String>,
    out_of_line_modules: &'a BTreeSet<String>,
    module_components: Option<Vec<String>>,
    bindings: &'a BTreeSet<String>,
}

fn source_file_out_of_line_module_names(
    path: &Path,
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeSet<String>> {
    let mut root = symbol_node;
    while let Some(parent) = root.parent() {
        root = parent;
    }
    Ok(rust_direct_module_candidate_paths(path, root, source)?
        .into_keys()
        .collect())
}

fn local_module_function_names(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<Option<BTreeSet<String>>> {
    let Some(container) = local_module_function_container(symbol_node) else {
        return Ok(None);
    };

    let mut names = BTreeSet::new();
    let mut cursor = container.walk();
    for child in container.named_children(&mut cursor) {
        if child.kind() != "function_item" {
            continue;
        }
        if let Some(name) = rust_symbol_name(child, source)? {
            names.insert(name);
        }
    }
    Ok(Some(names))
}

fn source_file_imported_function_paths(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, String>> {
    let mut root = symbol_node;
    while let Some(parent) = root.parent() {
        root = parent;
    }

    let mut paths_by_local_name = BTreeMap::<String, Vec<String>>::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        if node.kind() != "use_declaration" {
            continue;
        }
        let Some(argument) = node.child_by_field_name("argument") else {
            continue;
        };
        let mut bindings = Vec::new();
        collect_rust_function_import_bindings(argument, &[], source, &mut bindings)?;
        for (local_name, target_path) in bindings {
            paths_by_local_name
                .entry(local_name)
                .or_default()
                .push(target_path);
        }
    }

    Ok(paths_by_local_name
        .into_iter()
        .filter_map(|(name, paths)| (paths.len() == 1).then(|| (name, paths[0].clone())))
        .collect())
}

fn collect_rust_function_import_bindings(
    node: Node<'_>,
    prefix: &[String],
    source: &str,
    bindings: &mut Vec<(String, String)>,
) -> Result<()> {
    match node.kind() {
        "scoped_use_list" => {
            let Some(path) = node.child_by_field_name("path") else {
                return Ok(());
            };
            let Some(path_components) = rust_import_path_components(path, source)? else {
                return Ok(());
            };
            let Some(prefix) = rust_join_import_path_components(prefix, &path_components) else {
                return Ok(());
            };
            let Some(list) = node.child_by_field_name("list") else {
                return Ok(());
            };
            collect_rust_function_import_bindings(list, &prefix, source, bindings)?;
        }
        "use_list" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_rust_function_import_bindings(child, prefix, source, bindings)?;
            }
        }
        "use_as_clause" => {
            let Some(path) = node.child_by_field_name("path") else {
                return Ok(());
            };
            let Some(alias) = node.child_by_field_name("alias") else {
                return Ok(());
            };
            let Some(path_components) = rust_import_path_components(path, source)? else {
                return Ok(());
            };
            let Some(target_components) =
                rust_join_import_path_components(prefix, &path_components)
            else {
                return Ok(());
            };
            let alias = node_text(alias, source)?.trim();
            if let Some(binding) = rust_function_import_binding(&target_components, alias) {
                bindings.push(binding);
            }
        }
        "scoped_identifier" | "identifier" => {
            let Some(path_components) = rust_import_path_components(node, source)? else {
                return Ok(());
            };
            let Some(target_components) =
                rust_join_import_path_components(prefix, &path_components)
            else {
                return Ok(());
            };
            let Some(local_name) = target_components.last() else {
                return Ok(());
            };
            if let Some(binding) = rust_function_import_binding(&target_components, local_name) {
                bindings.push(binding);
            }
        }
        _ => {}
    }
    Ok(())
}

fn rust_import_path_components(node: Node<'_>, source: &str) -> Result<Option<Vec<String>>> {
    if !matches!(
        node.kind(),
        "crate" | "self" | "identifier" | "scoped_identifier"
    ) {
        return Ok(None);
    }
    let spelling = node_text(node, source)?.trim();
    let components = spelling
        .split("::")
        .filter(|component| !component.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if components.is_empty() || spelling.split("::").any(str::is_empty) {
        return Ok(None);
    }
    Ok(Some(components))
}

fn rust_join_import_path_components(prefix: &[String], path: &[String]) -> Option<Vec<String>> {
    if prefix.is_empty() {
        return Some(path.to_vec());
    }
    (!matches!(path.first().map(String::as_str), Some("crate" | "self")))
        .then(|| prefix.iter().chain(path).cloned().collect::<Vec<String>>())
}

fn rust_function_import_binding(
    target_components: &[String],
    local_name: &str,
) -> Option<(String, String)> {
    if local_name.is_empty()
        || target_components.len() < 3
        || !matches!(
            target_components.first().map(String::as_str),
            Some("crate" | "self")
        )
        || target_components
            .iter()
            .any(|component| component.is_empty())
    {
        return None;
    }
    Some((local_name.to_string(), target_components[1..].join("::")))
}

fn local_module_function_paths(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<Option<BTreeMap<String, String>>> {
    let Some(container) = local_module_function_container(symbol_node) else {
        return Ok(None);
    };

    let mut paths_by_name = BTreeMap::<String, Vec<String>>::new();
    let mut cursor = container.walk();
    for child in container.named_children(&mut cursor) {
        if child.kind() != "function_item" {
            continue;
        }
        let Some(name) = rust_symbol_name(child, source)? else {
            continue;
        };
        let Some(path) = rust_semantic_path(child, source, &name)? else {
            continue;
        };
        paths_by_name.entry(name).or_default().push(path);
    }

    Ok(Some(
        paths_by_name
            .into_iter()
            .filter_map(|(name, paths)| (paths.len() == 1).then(|| (name, paths[0].clone())))
            .collect(),
    ))
}

fn source_file_module_function_paths(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, String>> {
    let mut root = symbol_node;
    while let Some(parent) = root.parent() {
        root = parent;
    }

    let mut paths_by_name = BTreeMap::<String, Vec<String>>::new();
    collect_source_file_module_function_paths(root, source, &mut paths_by_name)?;
    Ok(paths_by_name
        .into_iter()
        .filter_map(|(path, candidates)| {
            (candidates.len() == 1).then(|| (path, candidates[0].clone()))
        })
        .collect())
}

fn collect_source_file_module_function_paths(
    node: Node<'_>,
    source: &str,
    paths_by_name: &mut BTreeMap<String, Vec<String>>,
) -> Result<()> {
    if node.kind() == "function_item"
        && local_module_function_container(node).is_some()
        && let Some(name) = rust_symbol_name(node, source)?
        && let Some(path) = rust_semantic_path(node, source, &name)?
    {
        paths_by_name.entry(path.clone()).or_default().push(path);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_source_file_module_function_paths(child, source, paths_by_name)?;
    }
    Ok(())
}

fn local_module_function_container(symbol_node: Node<'_>) -> Option<Node<'_>> {
    let parent = symbol_node.parent()?;
    if parent.kind() == "source_file" {
        return Some(parent);
    }
    (parent.kind() == "declaration_list"
        && parent
            .parent()
            .is_some_and(|owner| owner.kind() == "mod_item"))
    .then_some(parent)
}

fn collect_function_bindings(
    symbol_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(parameters) = symbol_node.child_by_field_name("parameters") {
        let mut cursor = parameters.walk();
        for parameter in parameters.named_children(&mut cursor) {
            if let Some(pattern) = parameter.child_by_field_name("pattern") {
                collect_pattern_bindings(pattern, source, bindings)?;
            }
        }
    }
    if let Some(body) = symbol_node.child_by_field_name("body") {
        collect_body_bindings(body, source, bindings)?;
    }
    Ok(())
}

fn collect_body_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    if matches!(
        node.kind(),
        "closure_expression" | "function_item" | "function_signature_item"
    ) {
        return Ok(());
    }
    if let Some(pattern) = node.child_by_field_name("pattern") {
        collect_pattern_bindings(pattern, source, bindings)?;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_body_bindings(child, source, bindings)?;
    }
    Ok(())
}

fn collect_pattern_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    if node.kind() == "identifier" {
        let name = node_text(node, source)?.trim();
        if !name.is_empty() {
            bindings.insert(name.to_string());
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_pattern_bindings(child, source, bindings)?;
    }
    Ok(())
}

fn collect_direct_local_calls_from_node(
    node: Node<'_>,
    context: &RustDirectCallContext<'_>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(deadline) = context.deadline {
        deadline.check("collecting Rust direct calls")?;
    }
    if matches!(
        node.kind(),
        "closure_expression" | "function_item" | "function_signature_item"
    ) {
        return Ok(());
    }
    if node.kind() == "call_expression"
        && let Some(function) = node.child_by_field_name("function")
    {
        if function.kind() == "identifier" {
            let name = node_text(function, context.source)?.trim();
            if !name.is_empty() && !context.bindings.contains(name) {
                if let Some(path) = context.local_functions.get(name) {
                    references.insert(path.clone());
                } else if !context.local_function_names.contains(name)
                    && let Some(path) = context.imported_functions.get(name)
                {
                    references.insert(path.clone());
                }
            }
        } else if function.kind() == "scoped_identifier"
            && let Some(module_components) = context.module_components.as_deref()
            && let Some(path) =
                rust_qualified_call_target_path(function, module_components, context.source)?
        {
            if let Some(path) = context.qualified_functions.get(&path) {
                references.insert(path.clone());
            } else if is_out_of_line_module_call(&path, context.out_of_line_modules) {
                references.insert(path);
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_direct_local_calls_from_node(child, context, references)?;
    }
    Ok(())
}

fn is_out_of_line_module_call(path: &str, out_of_line_modules: &BTreeSet<String>) -> bool {
    let mut components = path.split("::");
    let Some(module_name) = components.next() else {
        return false;
    };
    let Some(next_component) = components.next() else {
        return false;
    };
    !next_component.is_empty()
        && components.all(|component| !component.is_empty())
        && out_of_line_modules.contains(module_name)
}

fn rust_inline_module_path_components(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<Option<Vec<String>>> {
    let mut components = Vec::new();
    let mut current = symbol_node.parent();
    while let Some(parent) = current {
        if parent.kind() == "mod_item" {
            let Some(name) = rust_symbol_name(parent, source)? else {
                return Ok(None);
            };
            components.push(name);
        }
        current = parent.parent();
    }
    components.reverse();
    Ok(Some(components))
}

fn rust_qualified_call_target_path(
    function: Node<'_>,
    module_components: &[String],
    source: &str,
) -> Result<Option<String>> {
    let spelling = node_text(function, source)?.trim();
    if spelling.is_empty() {
        return Ok(None);
    }
    let components = spelling.split("::").collect::<Vec<_>>();
    if components.iter().any(|component| component.is_empty()) {
        return Ok(None);
    }

    let mut module_components = module_components.to_vec();
    let mut components = components.into_iter();
    match components.next() {
        Some("crate") => module_components.clear(),
        Some("self") => {}
        Some("super") => {
            let mut parent_count = 1;
            while matches!(components.clone().next(), Some("super")) {
                components.next();
                parent_count += 1;
            }
            if parent_count > module_components.len() {
                return Ok(None);
            }
            module_components.truncate(module_components.len() - parent_count);
        }
        Some(first) => module_components.push(first.to_string()),
        None => return Ok(None),
    }
    module_components.extend(components.map(str::to_string));
    Ok((!module_components.is_empty()).then(|| module_components.join("::")))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_rust_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_unshadowed_direct_calls_to_local_module_functions() {
        let source = r#"
fn root_caller() { root_helper(); }
fn root_helper() {}

mod api {
    fn caller() {
        helper();
    }

    fn helper() {}
}
"#;
        let path = Path::new("src/api.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "api::caller")
            .unwrap();
        assert_eq!(
            caller.references_by_name,
            ["api::helper".to_string()].into()
        );
        assert_eq!(caller.reference_facts.len(), 1);
        assert_eq!(caller.reference_facts[0].spelling, "api::helper");
        assert!(caller.call_arities_by_name.is_empty());

        let root_caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "root_caller")
            .unwrap();
        assert_eq!(
            root_caller.references_by_name,
            ["root_helper".to_string()].into()
        );
    }

    #[test]
    fn indexes_qualified_calls_between_inline_modules() {
        let source = r#"
fn root_helper() {}

fn crate_caller() {
    crate::root_helper();
}

fn caller() {
    api::helper();
}

mod api {
    fn helper() {}

    fn self_caller() {
        self::helper();
    }

    mod nested {
        fn caller() {
            super::helper();
        }

        mod leaf {
            fn caller() {
                super::super::helper();
            }
        }
    }
}
"#;
        let path = Path::new("src/api.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        for (caller_path, callee_path) in [
            ("crate_caller", "root_helper"),
            ("caller", "api::helper"),
            ("api::self_caller", "api::helper"),
            ("api::nested::caller", "api::helper"),
            ("api::nested::leaf::caller", "api::helper"),
        ] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert_eq!(
                caller.references_by_name,
                [callee_path.to_string()].into(),
                "unexpected references for {caller_path}",
            );
        }
    }

    #[test]
    fn indexes_unshadowed_direct_calls_to_root_function_imports() {
        let source = r#"
mod api;
use crate::api::helper;
use crate::api::helper as aliased_helper;

fn caller() { helper(); }
fn alias_caller() { aliased_helper(); }
"#;
        let path = Path::new("src/lib.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        for caller_path in ["caller", "alias_caller"] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert_eq!(
                caller.references_by_name,
                ["api::helper".to_string()].into()
            );
        }
    }

    #[test]
    fn indexes_unshadowed_direct_calls_to_self_function_imports() {
        let source = r#"
mod nested;
use self::nested::value;
use self::{nested::value as grouped_value};

fn caller() { value(); }
fn grouped_caller() { grouped_value(); }
"#;
        let path = Path::new("src/api/mod.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        for caller_path in ["caller", "grouped_caller"] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert_eq!(
                caller.references_by_name,
                ["nested::value".to_string()].into()
            );
        }
    }

    #[test]
    fn prefers_local_functions_over_root_function_imports() {
        let source = r#"
mod api;
use crate::api::helper;

fn helper() {}
fn caller() { helper(); }
"#;
        let path = Path::new("src/lib.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "caller")
            .unwrap();
        assert_eq!(caller.references_by_name, ["helper".to_string()].into());
    }

    #[test]
    fn ignores_ambiguous_and_shadowed_root_function_import_calls() {
        let source = r#"
mod api;
use crate::api::helper;
use crate::api::helper as helper;
use crate::api::other::*;

fn caller() { helper(); }
fn shadowed(helper: fn()) { helper(); }
"#;
        let path = Path::new("src/lib.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        for caller_path in ["caller", "shadowed"] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert!(caller.references_by_name.is_empty());
        }
    }

    #[test]
    fn indexes_qualified_calls_to_declared_out_of_line_modules() {
        let source = r#"
mod api;

fn caller() {
    api::helper();
}
"#;
        let path = Path::new("src/lib.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "caller")
            .unwrap();
        assert_eq!(
            caller.references_by_name,
            ["api::helper".to_string()].into()
        );
    }

    #[test]
    fn resolves_qualified_calls_from_nested_functions_relative_to_their_module() {
        let source = r#"
fn root_helper() {}

fn outer() {
    fn nested() {
        self::root_helper();
        super::root_helper();
    }
}

mod outer {
    fn root_helper() {}
}
"#;
        let path = Path::new("src/api.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let nested = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "outer::nested")
            .unwrap();
        assert_eq!(
            nested.references_by_name,
            ["root_helper".to_string()].into(),
        );
    }

    #[test]
    fn ignores_shadowed_and_nonlocal_rust_calls() {
        let source = r#"
mod api {
    fn caller(helper: fn()) {
        helper();
        let helper = || {};
        helper();
        if let Some(helper) = Some(|| {}) {
            helper();
        }
        crate::outside();
    }

    fn helper() {}
}
"#;
        let path = Path::new("src/api.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "api::caller")
            .unwrap();
        assert!(caller.references_by_name.is_empty());
        assert!(caller.reference_facts.is_empty());
    }

    #[test]
    fn indexes_rust_declarations_and_inherent_impl_methods_without_references() {
        let source = r#"
pub mod metrics {
    pub struct Counter;
    pub type Count = u64;
    pub const DEFAULT: Count = 1;
    pub static ACTIVE: bool = true;
    pub enum Event { Tick }
    pub trait Render { fn render(&self) -> String; }

    impl Counter {
        pub fn increment(&mut self, amount: Count) -> Count { amount }
    }
}
"#;
        let path = Path::new("src/metrics.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let paths = symbols
            .iter()
            .map(|symbol| symbol.semantic_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "metrics",
                "metrics::Counter",
                "metrics::Count",
                "metrics::DEFAULT",
                "metrics::ACTIVE",
                "metrics::Event",
                "metrics::Render",
                "metrics::Render::render",
                "metrics::Counter::increment",
            ]
        );

        let increment = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "metrics::Counter::increment")
            .unwrap();
        assert_eq!(increment.parameters, vec!["&mut self", "amount: Count"]);
        assert_eq!(increment.return_type.as_deref(), Some("Count"));
        assert!(increment.references_by_name.is_empty());
        assert!(increment.reference_facts.is_empty());
        assert_eq!(
            increment.byte_range.0,
            source.find("pub fn increment").unwrap()
        );
        assert_eq!(
            &source[increment.byte_range.0..increment.byte_range.1],
            "pub fn increment(&mut self, amount: Count) -> Count { amount }"
        );
    }
}
