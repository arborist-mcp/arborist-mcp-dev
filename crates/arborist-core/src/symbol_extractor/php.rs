use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
use crate::semantic::php::{
    is_php_symbol_node, php_parameters, php_return_type, php_signature, php_symbol_name,
    php_symbol_path,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_php_symbols_with_deadline(
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
        deadline.check("extracting PHP symbols")?;
    }
    if is_php_symbol_node(node)
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
    let Some(_name) = php_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = php_symbol_path(node, source)? else {
        return Ok(None);
    };
    let scope_path = semantic_path
        .rsplit_once("::")
        .map(|(scope_path, _)| scope_path.to_string());
    let references_by_name = collect_php_direct_calls(node, source, deadline)?;
    Ok(Some(IndexedSymbol {
        symbol_id: semantic_path.clone(),
        base_name: symbol_base_name(&semantic_path),
        semantic_path,
        scope_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: php_signature(node, source),
        is_overload: false,
        parameters: php_parameters(node, source),
        return_type: php_return_type(node, source)?,
        docstring: None,
        extension_receiver: None,
        reference_facts: Vec::new(),
        references_by_name,
        call_arities_by_name: BTreeMap::new(),
    }))
}

fn collect_php_direct_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<BTreeSet<String>> {
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(BTreeSet::new());
    };
    let mut references = BTreeSet::new();
    collect_php_direct_calls_from_node(body, source, deadline, &mut references)?;
    Ok(references)
}

fn collect_php_direct_calls_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting PHP references")?;
    }
    // Nested function declarations own their calls; the enclosing function should
    // not attribute calls reachable only through them to itself.
    if node.kind() == "function_definition" || node.kind() == "method_declaration" {
        return Ok(());
    }
    if node.kind() == "function_call_expression"
        && let Some(function_node) = node.child_by_field_name("function")
        && function_node.kind() == "name"
    {
        let name = node_text(function_node, source)?.trim();
        if !name.is_empty() {
            references.insert(name.to_string());
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(deadline) = deadline {
            deadline.check("extracting PHP references")?;
        }
        if child.kind() == "function_definition" || child.kind() == "method_declaration" {
            continue;
        }
        collect_php_direct_calls_from_node(child, source, deadline, references)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::index_php_symbols_with_deadline;

    use crate::language::parse_document;

    #[test]
    fn indexes_php_functions_and_methods() {
        let source = r#"<?php
function compute(int $value): int {
    return $value + 1;
}

class Greeter {
    public function greet(string $name): string {
        return "hi " . $name;
    }
}

function caller() {
    return compute(1);
}
"#;
        let path = Path::new("sample.php");
        let document = parse_document(path, source).unwrap();
        assert_eq!(document.language_id, crate::LanguageId::Php);
        let symbols =
            index_php_symbols_with_deadline(path, source, document.tree.root_node(), None).unwrap();
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec!["compute", "Greeter::greet", "caller"]
        );
        let compute = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "compute")
            .unwrap();
        assert_eq!(compute.parameters, vec!["$value"]);
        assert_eq!(compute.return_type.as_deref(), Some("int"));
        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "caller")
            .unwrap();
        assert_eq!(
            caller.references_by_name,
            BTreeSet::from(["compute".to_string()])
        );
        let greet = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Greeter::greet")
            .unwrap();
        assert!(greet.references_by_name.is_empty());
    }
}
