use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::normalize_path;
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
        && let Some(symbol) = indexed_symbol(path, source, node)?
    {
        symbols.push(symbol);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_symbols(path, source, child, deadline, symbols)?;
    }
    Ok(())
}

fn indexed_symbol(path: &Path, source: &str, node: Node<'_>) -> Result<Option<IndexedSymbol>> {
    let Some(name) = rust_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = rust_semantic_path(node, source, &name)? else {
        return Ok(None);
    };
    let scope_path = semantic_path
        .rsplit_once("::")
        .map(|(scope_path, _)| scope_path.to_string());
    let references_by_name = BTreeSet::new();
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_rust_symbols_with_deadline;
    use crate::language::parse_document;

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
