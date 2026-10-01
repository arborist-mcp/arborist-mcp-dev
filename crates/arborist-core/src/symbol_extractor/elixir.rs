use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
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
    let declaration_paths = declarations
        .iter()
        .map(|(_, symbol)| symbol.semantic_path.clone())
        .collect::<BTreeSet<_>>();
    let file_path = normalize_path(path);
    declarations
        .into_iter()
        .map(|(node, symbol)| {
            let references_by_name = collect_elixir_direct_calls(
                node,
                source,
                symbol.scope_path.as_deref(),
                &declaration_paths,
                deadline,
            )?;
            Ok(IndexedSymbol {
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
                references_by_name,
                call_arities_by_name: Default::default(),
            })
        })
        .collect()
}

fn collect_elixir_direct_calls(
    symbol_node: Node<'_>,
    source: &str,
    scope_path: Option<&str>,
    declaration_paths: &BTreeSet<String>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<BTreeSet<String>> {
    let mut references = BTreeSet::new();
    if let Some(body) = symbol_node.child_by_field_name("do_block") {
        collect_direct_calls_from_node(
            body,
            source,
            scope_path,
            declaration_paths,
            deadline,
            &mut references,
        )?;
    }
    if let Some(arguments) = symbol_node.child_by_field_name("arguments") {
        // In a one-line definition, `do: expression` is represented as a
        // keyword pair inside the definition arguments rather than a
        // declaration-level `do_block`.
        let mut cursor = arguments.walk();
        for argument in arguments.named_children(&mut cursor).skip(1) {
            if argument.kind() != "keywords" {
                continue;
            }
            let mut pairs = argument.walk();
            for pair in argument.named_children(&mut pairs) {
                if pair.kind() != "pair"
                    || pair
                        .child_by_field_name("key")
                        .and_then(|key| node_text(key, source).ok())
                        .is_none_or(|key| key.trim() != "do")
                {
                    continue;
                }
                if let Some(value) = pair.child_by_field_name("value") {
                    collect_direct_calls_from_node(
                        value,
                        source,
                        scope_path,
                        declaration_paths,
                        deadline,
                        &mut references,
                    )?;
                }
            }
        }
    }
    Ok(references)
}

fn collect_direct_calls_from_node(
    node: Node<'_>,
    source: &str,
    scope_path: Option<&str>,
    declaration_paths: &BTreeSet<String>,
    deadline: Option<&WorkspaceScanDeadline>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting Elixir references")?;
    }
    if node.kind() == "call" {
        let target = node.child_by_field_name("target");
        let target_name = target
            .filter(|target| target.kind() == "identifier")
            .map(|target| node_text(target, source))
            .transpose()?
            .map(|name| name.trim().to_string());
        if let Some(target_name) = target_name
            && !matches!(target_name.as_str(), "def" | "defp" | "defmodule")
            && !target_name.is_empty()
        {
            let reference = scope_path
                .filter(|scope| !scope.is_empty())
                .map(|scope| format!("{scope}.{target_name}"))
                .unwrap_or(target_name);
            if declaration_paths.contains(reference.as_str()) {
                references.insert(reference);
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "call"
            && child
                .child_by_field_name("target")
                .and_then(|target| node_text(target, source).ok())
                .is_some_and(|target| matches!(target.trim(), "def" | "defp" | "defmodule"))
        {
            continue;
        }
        collect_direct_calls_from_node(
            child,
            source,
            scope_path,
            declaration_paths,
            deadline,
            references,
        )?;
    }
    Ok(())
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

    #[test]
    fn indexes_same_module_direct_call_references() {
        let source = "defmodule Demo do\n  def compute(value), do: value\n  def caller(value), do: compute(value)\nend\n";
        let path = Path::new("sample.ex");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_elixir_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Demo.caller")
            .unwrap();
        assert_eq!(
            caller.references_by_name,
            BTreeSet::from(["Demo.compute".into()])
        );
    }

    #[test]
    fn indexes_multiline_direct_call_references_without_builtins_or_remote_calls() {
        let source = "defmodule Demo do\n  def compute(value), do: value\n  def caller(value) do\n    if value > 0 do\n      compute(value)\n    else\n      missing(value)\n    end\n  end\nend\n";
        let path = Path::new("sample.ex");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_elixir_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Demo.caller")
            .unwrap();
        assert_eq!(
            caller.references_by_name,
            BTreeSet::from(["Demo.compute".to_string()])
        );
    }

    #[test]
    fn indexes_nested_module_calls_with_their_local_scope() {
        let source = "defmodule Outer do\n  defmodule Inner do\n    def compute(value), do: value\n    def caller(value), do: compute(value)\n  end\nend\n";
        let path = Path::new("sample.ex");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_elixir_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Outer.Inner.caller")
            .unwrap();
        assert_eq!(
            caller.references_by_name,
            BTreeSet::from(["Outer.Inner.compute".to_string()])
        );
    }

    #[test]
    fn malformed_elixir_reference_extraction_does_not_panic() {
        let source = "defmodule Broken do\n  def caller(value), do: compute(\n";
        let path = Path::new("sample.ex");
        let document = parse_document(path, source).unwrap();
        let result =
            index_elixir_symbols_with_deadline(path, source, document.tree.root_node(), None);
        assert!(result.is_ok());
    }
}
