use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::normalize_path;
use crate::semantic::elixir::collect_elixir_declarations;
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_elixir_symbols_with_deadline(
    path: &Path,
    source: &str,
    root: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<IndexedSymbol>> {
    let declarations = collect_elixir_declarations(root, source, deadline)?;
    let file_path = normalize_path(path);
    Ok(declarations
        .into_iter()
        .map(|(node, symbol)| IndexedSymbol {
            symbol_id: symbol.symbol_id.clone(),
            base_name: symbol_base_name(&symbol.semantic_path),
            semantic_path: symbol.semantic_path,
            scope_path: symbol.scope_path,
            file_path: file_path.clone(),
            node_kind: symbol.node_kind,
            byte_range: symbol.byte_range,
            signature: symbol.signature,
            is_overload: false,
            parameters: symbol.parameters,
            return_type: symbol.return_type,
            docstring: symbol.docstring,
            extension_receiver: None,
            reference_facts: Vec::new(),
            references_by_name: Default::default(),
            call_arities_by_name: Default::default(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_elixir_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_named_elixir_functions_with_module_paths() {
        let source =
            "defmodule Demo do\n  def run(value), do: value\n  defp helper(), do: :ok\nend\n";
        let path = Path::new("sample.ex");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_elixir_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec!["Demo.run", "Demo.helper"]
        );
        assert_eq!(symbols[0].base_name, "run");
        assert_eq!(symbols[0].parameters, vec!["value"]);
        assert_eq!(symbols[0].scope_path.as_deref(), Some("Demo"));
    }
}
