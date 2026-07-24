use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{normalize_path, visit_tree};
use crate::patching::collect_python_references;
use crate::semantic::{
    python_display_byte_range, python_display_header, python_docstring, python_parameters,
    python_return_type, semantic_parent_path, semantic_path,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};

pub(super) fn index_python_symbols(
    path: &Path,
    source: &str,
    root: Node<'_>,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();
    let normalized_path = normalize_path(path);

    let mut callback = |node: Node<'_>| {
        if !matches!(node.kind(), "class_definition" | "function_definition") {
            return;
        }

        let mut references = BTreeSet::new();
        let reference_node = python_reference_node(node);
        let _ = collect_python_references(path, reference_node, source, &mut references);
        let signature = python_display_header(node, source).ok();
        let path = match semantic_path(node, source) {
            Ok(path) => path,
            Err(_) => return,
        };
        let scope_path = semantic_parent_path(&path);
        let parameters = python_parameters(node, source).unwrap_or_default();
        let return_type = python_return_type(node, source).ok().flatten();
        let docstring = python_docstring(node, source).ok().flatten();

        symbols.push(IndexedSymbol {
            symbol_id: String::new(),
            base_name: symbol_base_name(&path),
            semantic_path: path,
            scope_path,
            file_path: normalized_path.clone(),
            node_kind: node.kind().to_string(),
            byte_range: python_display_byte_range(node),
            signature,
            parameters,
            return_type,
            docstring,
            references_by_name: references,
            call_arities_by_name: std::collections::BTreeMap::new(),
        });
    };

    visit_tree(root, &mut callback);
    Ok(symbols)
}

fn python_reference_node(node: Node<'_>) -> Node<'_> {
    node.parent()
        .filter(|parent| parent.kind() == "decorated_definition")
        .unwrap_or(node)
}
