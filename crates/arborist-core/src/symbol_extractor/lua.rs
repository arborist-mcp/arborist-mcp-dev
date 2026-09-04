use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::normalize_path;
use crate::semantic::lua::{
    is_lua_symbol_node, lua_parameters, lua_semantic_path, lua_signature, lua_symbol_name,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_lua_symbols_with_deadline(
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
        deadline.check("extracting Lua symbols")?;
    }
    if is_lua_symbol_node(node)
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
    _deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<IndexedSymbol>> {
    let Some(name) = lua_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = lua_semantic_path(name.as_str())? else {
        return Ok(None);
    };
    let scope_path = semantic_path
        .rsplit_once("::")
        .map(|(scope_path, _)| scope_path.to_string());
    Ok(Some(IndexedSymbol {
        symbol_id: semantic_path.clone(),
        base_name: symbol_base_name(&semantic_path),
        semantic_path,
        scope_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: lua_signature(node, source),
        is_overload: false,
        parameters: lua_parameters(node, source),
        return_type: None,
        docstring: None,
        extension_receiver: None,
        reference_facts: Vec::new(),
        references_by_name: BTreeSet::new(),
        call_arities_by_name: BTreeMap::new(),
    }))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_lua_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_lua_function_declarations() {
        let source = r#"local function compute(value)
    return value + 1
end

function greet(name, greeting)
    return greeting
end
"#;
        let path = Path::new("sample.lua");
        let document = parse_document(path, source).unwrap();
        assert_eq!(document.language_id, crate::LanguageId::Lua);
        let symbols =
            index_lua_symbols_with_deadline(path, source, document.tree.root_node(), None).unwrap();
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec!["compute", "greet"]
        );
        assert_eq!(symbols[0].parameters, vec!["value"]);
        assert!(symbols[0].references_by_name.is_empty());
    }
}
