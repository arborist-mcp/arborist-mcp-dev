use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::normalize_path;
use crate::semantic::java::{
    is_java_symbol_node, java_parameters, java_return_type, java_semantic_path, java_signature,
    java_symbol_name,
};
use crate::semantic::semantic_parent_path;
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::symbol_reference_compat::reference_facts_from_legacy;
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_java_symbols_with_deadline(
    path: &Path,
    source: &str,
    root: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();
    collect_symbols(path, source, root, root, deadline, &mut symbols)?;
    Ok(symbols)
}

fn collect_symbols(
    path: &Path,
    source: &str,
    root: Node<'_>,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
    symbols: &mut Vec<IndexedSymbol>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting Java symbols")?;
    }
    if is_java_symbol_node(node)
        && let Some(symbol) = indexed_symbol(path, source, root, node)?
    {
        symbols.push(symbol);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_symbols(path, source, root, child, deadline, symbols)?;
    }
    Ok(())
}

fn indexed_symbol(
    path: &Path,
    source: &str,
    root: Node<'_>,
    node: Node<'_>,
) -> Result<Option<IndexedSymbol>> {
    let Some(name) = java_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = java_semantic_path(root, node, source, &name)? else {
        return Ok(None);
    };
    let references_by_name = BTreeSet::new();
    let call_arities_by_name = BTreeMap::new();

    Ok(Some(IndexedSymbol {
        symbol_id: String::new(),
        base_name: symbol_base_name(&semantic_path),
        scope_path: semantic_parent_path(&semantic_path),
        semantic_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: java_signature(node, source),
        is_overload: false,
        parameters: java_parameters(node, source),
        return_type: java_return_type(node, source),
        docstring: None,
        reference_facts: reference_facts_from_legacy(&references_by_name, &call_arities_by_name),
        references_by_name,
        call_arities_by_name,
    }))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_java_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_package_qualified_java_declarations_without_reference_facts() {
        let source = r#"
package com.example;

public class Counter {
    public Counter(int initial) {}
    public Counter(String label) {}
    public int increment(int amount) { return amount; }
    public int increment(long amount) { return (int) amount; }
}
interface Renderer { String render(); }
enum Kind { BASIC }
"#;
        let path = Path::new("Counter.java");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_java_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "com::example::Counter",
                "com::example::Counter::Counter",
                "com::example::Counter::Counter",
                "com::example::Counter::increment",
                "com::example::Counter::increment",
                "com::example::Renderer",
                "com::example::Renderer::render",
                "com::example::Kind",
            ]
        );
        assert!(
            symbols
                .iter()
                .all(|symbol| symbol.references_by_name.is_empty())
        );
        assert!(
            symbols
                .iter()
                .all(|symbol| symbol.call_arities_by_name.is_empty())
        );

        let method = symbols
            .iter()
            .find(|symbol| {
                symbol.semantic_path == "com::example::Counter::increment"
                    && symbol.parameters == ["int amount"]
            })
            .unwrap();
        assert_eq!(method.return_type.as_deref(), Some("int"));
    }
}
