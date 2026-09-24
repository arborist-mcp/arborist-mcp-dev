use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
use crate::semantic::shell::{
    is_shell_symbol_node, shell_parameters, shell_semantic_path, shell_signature, shell_symbol_name,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_shell_symbols_with_deadline(
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
        deadline.check("extracting shell symbols")?;
    }
    if is_shell_symbol_node(node)
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
    let Some(name) = shell_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = shell_semantic_path(name.as_str())? else {
        return Ok(None);
    };
    let scope_path = semantic_path
        .rsplit_once("::")
        .map(|(scope_path, _)| scope_path.to_string());
    let references_by_name = collect_shell_direct_calls(node, source, deadline)?;
    Ok(Some(IndexedSymbol {
        symbol_id: semantic_path.clone(),
        base_name: symbol_base_name(&semantic_path),
        semantic_path,
        scope_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: shell_signature(node, source),
        is_overload: false,
        parameters: shell_parameters(node, source),
        return_type: None,
        docstring: None,
        extension_receiver: None,
        reference_facts: Vec::new(),
        references_by_name,
        call_arities_by_name: BTreeMap::new(),
    }))
}

fn collect_shell_direct_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<BTreeSet<String>> {
    if symbol_node.kind() != "function_definition" {
        return Ok(BTreeSet::new());
    }
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(BTreeSet::new());
    };
    let mut references = BTreeSet::new();
    collect_shell_direct_calls_from_node(body, source, deadline, &mut references)?;
    Ok(references)
}

fn collect_shell_direct_calls_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting shell references")?;
    }
    if node.kind() == "function_definition" {
        return Ok(());
    }
    if let Some(name) = shell_command_name(node, source)? {
        references.insert(name);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(deadline) = deadline {
            deadline.check("extracting shell references")?;
        }
        if child.kind() == "function_definition" {
            continue;
        }
        collect_shell_direct_calls_from_node(child, source, deadline, references)?;
    }
    Ok(())
}

/// Returns the bare command name when `node` is a command invoked by a plain
/// word (for example `helper` in `helper "$value"`). Commands whose name is a
/// string, expansion, or path are not treated as direct calls.
pub(crate) fn shell_command_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if node.kind() != "command" {
        return Ok(None);
    }
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(None);
    };
    // The command name field is a `command_name` node that wraps the concrete
    // literal (word, string, expansion, concatenation, ...).
    let literal = if name_node.kind() == "command_name" {
        match name_node.named_child(0) {
            Some(child) => child,
            None => return Ok(None),
        }
    } else {
        name_node
    };
    if literal.kind() != "word" {
        return Ok(None);
    }
    let name = node_text(literal, source)?.trim();
    if name.is_empty() {
        return Ok(None);
    }
    Ok(Some(name.to_string()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::index_shell_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_shell_function_definitions() {
        let source = r#"compute() {
    value=1
}

function greet {
    name=2
}
"#;
        let path = Path::new("sample.sh");
        let document = parse_document(path, source).unwrap();
        assert_eq!(document.language_id, crate::LanguageId::Shell);
        let symbols =
            index_shell_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec!["compute", "greet"]
        );
    }

    #[test]
    fn indexes_shell_direct_call_references() {
        let source = r#"helper() {
    value=1
}

orchestrate() {
    helper
    missing_helper
}
"#;
        let path = Path::new("sample.sh");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_shell_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let orchestrate = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "orchestrate")
            .unwrap();
        assert_eq!(
            orchestrate.references_by_name,
            BTreeSet::from(["helper".to_string(), "missing_helper".to_string()])
        );
    }
}
