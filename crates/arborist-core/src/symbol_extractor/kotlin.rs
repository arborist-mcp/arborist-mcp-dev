use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::normalize_path;
use crate::semantic::kotlin::{
    is_kotlin_semantic_symbol_node, kotlin_parameters, kotlin_return_type, kotlin_semantic_path,
    kotlin_signature, kotlin_symbol_name,
};
use crate::semantic::semantic_parent_path;
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_kotlin_symbols_with_deadline(
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
        deadline.check("extracting Kotlin symbols")?;
    }
    if is_kotlin_semantic_symbol_node(node)
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
    let Some(name) = kotlin_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = kotlin_semantic_path(root, node, source, &name)? else {
        return Ok(None);
    };

    Ok(Some(IndexedSymbol {
        symbol_id: String::new(),
        base_name: symbol_base_name(&semantic_path),
        scope_path: semantic_parent_path(&semantic_path),
        semantic_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: kotlin_signature(node, source),
        is_overload: false,
        parameters: kotlin_parameters(node, source),
        return_type: kotlin_return_type(node, source),
        docstring: None,
        reference_facts: Vec::new(),
        references_by_name: BTreeSet::new(),
        call_arities_by_name: BTreeMap::new(),
    }))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_kotlin_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_package_qualified_kotlin_declarations_without_reference_facts() {
        let source = r#"
package com.example

typealias UserId = String

class Counter {
    val label: String = "counter"
    fun increment(amount: Int): Int = amount
    fun increment(amount: Long): Long = amount
    fun outer() {
        class Local
        fun nested() = 1
    }
}

object Config {
    val answer = 42
}
"#;
        let path = Path::new("Counter.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let symbols =
            index_kotlin_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "com::example::UserId",
                "com::example::Counter",
                "com::example::Counter::label",
                "com::example::Counter::increment",
                "com::example::Counter::increment",
                "com::example::Counter::outer",
                "com::example::Config",
                "com::example::Config::answer",
            ]
        );
        assert!(
            symbols
                .iter()
                .all(|symbol| symbol.reference_facts.is_empty()
                    && symbol.references_by_name.is_empty()
                    && symbol.call_arities_by_name.is_empty())
        );
        let increment = symbols
            .iter()
            .find(|symbol| {
                symbol.semantic_path == "com::example::Counter::increment"
                    && symbol.parameters == ["amount: Int"]
            })
            .unwrap();
        assert_eq!(increment.return_type.as_deref(), Some("Int"));
    }
}
