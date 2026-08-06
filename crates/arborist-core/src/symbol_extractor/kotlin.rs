use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
use crate::semantic::kotlin::{
    is_kotlin_semantic_symbol_node, kotlin_parameters, kotlin_return_type, kotlin_semantic_path,
    kotlin_signature, kotlin_symbol_name,
};
use crate::semantic::semantic_parent_path;
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::symbol_reference_compat::reference_facts_from_legacy;
use crate::workspace_scan::WorkspaceScanDeadline;

type ReferenceNames = BTreeSet<String>;
type CallAritiesByName = BTreeMap<String, BTreeSet<usize>>;
type DirectCalls = (ReferenceNames, CallAritiesByName);

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
        && let Some(symbol) = indexed_symbol(path, source, root, node, deadline)?
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
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<IndexedSymbol>> {
    let Some(name) = kotlin_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = kotlin_semantic_path(root, node, source, &name)? else {
        return Ok(None);
    };
    let (references_by_name, call_arities_by_name) =
        collect_direct_local_calls(node, source, deadline)?;

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
        reference_facts: reference_facts_from_legacy(&references_by_name, &call_arities_by_name),
        references_by_name,
        call_arities_by_name,
    }))
}

fn collect_direct_local_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<DirectCalls> {
    if symbol_node.kind() != "function_declaration" {
        return Ok((BTreeSet::new(), BTreeMap::new()));
    }
    let Some(body) = symbol_node
        .named_children(&mut symbol_node.walk())
        .find(|child| child.kind() == "function_body")
    else {
        return Ok((BTreeSet::new(), BTreeMap::new()));
    };

    let mut references = BTreeSet::new();
    let mut call_arities_by_name = BTreeMap::new();
    collect_direct_local_calls_from_node(
        body,
        source,
        deadline,
        &mut references,
        &mut call_arities_by_name,
    )?;
    Ok((references, call_arities_by_name))
}

fn collect_direct_local_calls_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    references: &mut ReferenceNames,
    call_arities_by_name: &mut CallAritiesByName,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting Kotlin direct calls")?;
    }
    if matches!(
        node.kind(),
        "function_declaration" | "class_declaration" | "object_declaration"
    ) {
        return Ok(());
    }
    if node.kind() == "call_expression"
        && let Some(callee) = node.named_child(0)
        && callee.kind() == "identifier"
        && let Some(arguments) = node
            .named_children(&mut node.walk())
            .find(|child| child.kind() == "value_arguments")
    {
        let mut cursor = arguments.walk();
        let arity = arguments.named_children(&mut cursor).count();
        let reference = node_text(callee, source)?.trim().to_string();
        if !reference.is_empty() {
            references.insert(reference.clone());
            call_arities_by_name
                .entry(reference)
                .or_default()
                .insert(arity);
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_direct_local_calls_from_node(
            child,
            source,
            deadline,
            references,
            call_arities_by_name,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_kotlin_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_package_qualified_kotlin_declarations_and_direct_calls() {
        let source = r#"
package com.example

typealias UserId = String

fun helper(amount: Int): Int = amount

class Counter {
    val label: String = "counter"
    fun increment(amount: Int): Int = amount
    fun increment(amount: Long): Long = amount
    fun outer() {
        class Local
        fun nested() = 1
        helper(1)
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
                "com::example::helper",
                "com::example::Counter",
                "com::example::Counter::label",
                "com::example::Counter::increment",
                "com::example::Counter::increment",
                "com::example::Counter::outer",
                "com::example::Config",
                "com::example::Config::answer",
            ]
        );
        let outer = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Counter::outer")
            .unwrap();
        assert_eq!(
            outer.references_by_name,
            ["helper".to_string()].into_iter().collect()
        );
        assert_eq!(
            outer.call_arities_by_name,
            [("helper".to_string(), [1usize].into_iter().collect())]
                .into_iter()
                .collect()
        );
        assert!(
            symbols
                .iter()
                .filter(|symbol| symbol.semantic_path != "com::example::Counter::outer")
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
