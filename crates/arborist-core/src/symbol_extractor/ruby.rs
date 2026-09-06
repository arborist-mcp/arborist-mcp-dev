use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
use crate::semantic::ruby::{
    is_ruby_symbol_node, ruby_parameters, ruby_semantic_path, ruby_signature, ruby_symbol_name,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_ruby_symbols_with_deadline(
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
        deadline.check("extracting Ruby symbols")?;
    }
    if is_ruby_symbol_node(node)
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
    let Some(name) = ruby_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = ruby_semantic_path(name.as_str())? else {
        return Ok(None);
    };
    let scope_path = semantic_path
        .rsplit_once("::")
        .map(|(scope_path, _)| scope_path.to_string());
    let references_by_name = collect_ruby_direct_calls(node, source, deadline)?;
    Ok(Some(IndexedSymbol {
        symbol_id: semantic_path.clone(),
        base_name: symbol_base_name(&semantic_path),
        semantic_path,
        scope_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: ruby_signature(node, source),
        is_overload: false,
        parameters: ruby_parameters(node, source),
        return_type: None,
        docstring: None,
        extension_receiver: None,
        reference_facts: Vec::new(),
        references_by_name,
        call_arities_by_name: BTreeMap::new(),
    }))
}

fn collect_ruby_direct_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<BTreeSet<String>> {
    if symbol_node.kind() != "method" {
        return Ok(BTreeSet::new());
    }
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(BTreeSet::new());
    };
    let mut references = BTreeSet::new();
    collect_ruby_direct_calls_from_node(body, source, deadline, &mut references)?;
    Ok(references)
}

fn collect_ruby_direct_calls_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting Ruby references")?;
    }
    if matches!(node.kind(), "method" | "singleton_method") {
        return Ok(());
    }
    if node.kind() == "call"
        && let Some(method_name) = node.child_by_field_name("method")
    {
        let name = node_text(method_name, source)?.trim();
        if !name.is_empty() {
            references.insert(name.to_string());
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(deadline) = deadline {
            deadline.check("extracting Ruby references")?;
        }
        if matches!(child.kind(), "method" | "singleton_method") {
            continue;
        }
        collect_ruby_direct_calls_from_node(child, source, deadline, references)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::index_ruby_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_ruby_method_definitions() {
        let source = r#"def compute(value)
    value + 1
end

def greet(name, greeting)
    name + greeting
end
"#;
        let path = Path::new("sample.rb");
        let document = parse_document(path, source).unwrap();
        assert_eq!(document.language_id, crate::LanguageId::Ruby);
        let symbols =
            index_ruby_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec!["compute", "greet"]
        );
        assert_eq!(symbols[0].parameters, vec!["value"]);
    }

    #[test]
    fn indexes_ruby_direct_call_references() {
        let source = r#"def compute(value)
    value + 1
end

def caller(value)
    compute(value)
end

def nested()
    missing()
end
"#;
        let path = Path::new("sample.rb");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_ruby_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "caller")
            .unwrap();
        assert_eq!(
            caller.references_by_name,
            BTreeSet::from(["compute".to_string()])
        );
        let nested = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "nested")
            .unwrap();
        assert_eq!(
            nested.references_by_name,
            BTreeSet::from(["missing".to_string()])
        );
    }
}
