use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::normalize_path;
use crate::semantic::csharp::{
    csharp_parameters, csharp_return_type, csharp_semantic_path, csharp_signature,
    csharp_symbol_name, is_csharp_symbol_node,
};
use crate::semantic::semantic_parent_path;
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_csharp_symbols_with_deadline(
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
        deadline.check("extracting C# symbols")?;
    }
    if is_csharp_symbol_node(node)
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
    let Some(name) = csharp_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = csharp_semantic_path(root, node, source, &name)? else {
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
        signature: csharp_signature(node, source),
        is_overload: false,
        parameters: csharp_parameters(node, source),
        return_type: csharp_return_type(node, source),
        docstring: None,
        reference_facts: Vec::new(),
        references_by_name: BTreeSet::new(),
        call_arities_by_name: BTreeMap::new(),
    }))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_csharp_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_namespace_qualified_csharp_declarations_without_reference_facts() {
        let source = r#"
namespace Demo.Core;

public class Counter {
    public Counter(int initial) {}
    public int Increment(int amount) => amount;
    public int Increment(long amount) => (int)amount;
}
public struct Point { public int X; }
public interface IRenderer { string Render(); }
public enum Kind { Basic }
public record Entry(string Name);
"#;
        let path = Path::new("Counter.cs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_csharp_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Demo::Core::Counter",
                "Demo::Core::Counter::Counter",
                "Demo::Core::Counter::Increment",
                "Demo::Core::Counter::Increment",
                "Demo::Core::Point",
                "Demo::Core::IRenderer",
                "Demo::Core::IRenderer::Render",
                "Demo::Core::Kind",
                "Demo::Core::Entry",
            ]
        );
        let increment = symbols
            .iter()
            .find(|symbol| {
                symbol.semantic_path == "Demo::Core::Counter::Increment"
                    && symbol.parameters == ["int amount"]
            })
            .unwrap();
        assert_eq!(increment.return_type.as_deref(), Some("int"));
        assert!(
            symbols
                .iter()
                .all(|symbol| symbol.reference_facts.is_empty())
        );
        assert!(
            symbols
                .iter()
                .all(|symbol| symbol.references_by_name.is_empty())
        );
    }
}
