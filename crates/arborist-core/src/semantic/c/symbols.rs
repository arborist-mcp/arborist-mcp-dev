use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{c_semantic_path, collect_c_scope_symbols};

pub(crate) fn c_symbol_nodes<'tree>(
    path: &Path,
    root: Node<'tree>,
    source: &str,
) -> Result<Vec<Node<'tree>>> {
    let mut symbols = Vec::new();
    collect_c_scope_symbols(root, &mut symbols);
    if !symbols
        .iter()
        .any(|node| node.kind() == "using_declaration")
    {
        return Ok(symbols);
    }

    let mut deduplicated = Vec::new();
    for node in symbols {
        if node.kind() != "using_declaration" {
            deduplicated.push(node);
            continue;
        }
        if c_semantic_path(path, node, source)?.is_none() {
            continue;
        }
        deduplicated.push(node);
    }

    Ok(deduplicated)
}
