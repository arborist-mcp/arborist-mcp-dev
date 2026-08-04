use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
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
    let references_by_name = collect_direct_local_calls(node, source, deadline)?;
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
    let Some(local_functions) = local_module_function_paths(symbol_node, source)? else {
        return Ok(BTreeSet::new());
    };
    if local_functions.is_empty() {
        return Ok(BTreeSet::new());
    }

    let mut bindings = BTreeSet::new();
    collect_function_bindings(symbol_node, source, &mut bindings)?;
    let mut references = BTreeSet::new();
    collect_direct_local_calls_from_node(
        body,
        source,
        deadline,
        &local_functions,
        &bindings,
        &mut references,
    )?;
    Ok(references)
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
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    local_functions: &BTreeMap<String, String>,
    bindings: &BTreeSet<String>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(deadline) = deadline {
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
        && function.kind() == "identifier"
    {
        let name = node_text(function, source)?.trim();
        if !name.is_empty()
            && !bindings.contains(name)
            && let Some(path) = local_functions.get(name)
        {
            references.insert(path.clone());
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_direct_local_calls_from_node(
            child,
            source,
            deadline,
            local_functions,
            bindings,
            references,
        )?;
    }
    Ok(())
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
