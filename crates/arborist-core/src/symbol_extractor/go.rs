use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::normalize_path;
use crate::semantic::go::{
    go_parameters, go_return_type, go_semantic_path, go_signature, go_symbol_name,
    is_go_symbol_node,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::symbol_reference_compat::reference_facts_from_legacy;
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_go_symbols_with_deadline(
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
        deadline.check("extracting Go symbols")?;
    }
    if is_go_symbol_node(node)
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
    let Some(name) = go_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = go_semantic_path(node, source, &name)? else {
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
        signature: go_signature(node, source),
        is_overload: false,
        parameters: go_parameters(node, source),
        return_type: go_return_type(node, source),
        docstring: None,
        reference_facts: reference_facts_from_legacy(&references_by_name, &call_arities_by_name),
        references_by_name,
        call_arities_by_name,
    }))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_go_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_go_named_types_functions_and_methods_without_references() {
        let source = r#"
package metrics

type Counter struct { value int }
type Alias = Counter

func NewCounter(value int) Counter { return Counter{value: value} }
func (counter *Counter) Increment(amount int) int { return counter.value + amount }
"#;
        let path = Path::new("metrics.go");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_go_symbols_with_deadline(path, source, document.tree.root_node(), None).unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec!["Counter", "Alias", "NewCounter", "Counter::Increment"]
        );
        let increment = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Counter::Increment")
            .unwrap();
        assert_eq!(increment.parameters, vec!["amount int"]);
        assert_eq!(increment.return_type.as_deref(), Some("int"));
        assert!(increment.references_by_name.is_empty());
        assert!(increment.reference_facts.is_empty());
    }
}
