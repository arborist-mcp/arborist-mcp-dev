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

type ReferenceNames = BTreeSet<String>;
type CallAritiesByName = BTreeMap<String, BTreeSet<usize>>;
type DirectCalls = (ReferenceNames, CallAritiesByName);

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
    let Some(name) = java_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = java_semantic_path(root, node, source, &name)? else {
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

fn collect_direct_local_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<DirectCalls> {
    if !matches!(
        symbol_node.kind(),
        "method_declaration" | "constructor_declaration"
    ) {
        return Ok((BTreeSet::new(), BTreeMap::new()));
    }
    let Some(body) = symbol_node.child_by_field_name("body") else {
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
        deadline.check("collecting Java direct calls")?;
    }
    if node.kind() == "class_body" {
        return Ok(());
    }
    if node.kind() == "method_invocation"
        && node.child_by_field_name("object").is_none()
        && let Some(name) = node.child_by_field_name("name")
    {
        let name = crate::language::node_text(name, source)?.trim();
        if !name.is_empty()
            && let Some(arguments) = node.child_by_field_name("arguments")
        {
            let mut cursor = arguments.walk();
            let arity = arguments.named_children(&mut cursor).count();
            references.insert(name.to_string());
            call_arities_by_name
                .entry(name.to_string())
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

    use super::index_java_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_package_qualified_java_declarations_and_unqualified_direct_calls() {
        let source = r#"
package com.example;

public class Counter {
    public Counter(int initial) {}
    public Counter(String label) {}
    public int increment(int amount) { return amount; }
    public int increment(long amount) { return (int) amount; }
    public int callIncrement() { return increment(1); }
    public int ambiguousIncrement() { return increment(1L); }
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
                "com::example::Counter::callIncrement",
                "com::example::Counter::ambiguousIncrement",
                "com::example::Renderer",
                "com::example::Renderer::render",
                "com::example::Kind",
            ]
        );
        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Counter::callIncrement")
            .unwrap();
        assert_eq!(caller.references_by_name, ["increment".to_string()].into());
        assert_eq!(
            caller.call_arities_by_name,
            [("increment".to_string(), [1].into())].into()
        );
        assert_eq!(caller.reference_facts.len(), 1);

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
