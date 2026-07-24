use crate::model::{SymbolSummary, SymbolSummaryInit};
use crate::semantic::{
    python_display_byte_range, python_display_header, python_docstring, python_parameters,
    python_return_type, semantic_parent_path, semantic_path,
};
use anyhow::Result;
use tree_sitter::Node;

pub(in super::super) fn python_symbol_summary(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    origin_type: &str,
) -> Result<Option<SymbolSummary>> {
    let Some(node) = python_symbol_node(node) else {
        return Ok(None);
    };

    let semantic_path = semantic_path(node, source)?;
    let scope_path = semantic_parent_path(&semantic_path);
    let signature = Some(python_display_header(node, source)?);
    let parameters = python_parameters(node, source)?;
    let return_type = python_return_type(node, source)?;
    let docstring = python_docstring(node, source)?;

    Ok(Some(SymbolSummary::new(SymbolSummaryInit {
        symbol_id: semantic_path.clone(),
        semantic_path,
        scope_path,
        file_path: normalized_path.to_string(),
        node_kind: node.kind().to_string(),
        origin_type: origin_type.to_string(),
        byte_range: python_display_byte_range(node),
        signature,
        parameters,
        return_type,
        docstring,
    })))
}

fn python_symbol_node(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "function_definition" | "class_definition" => Some(node),
        "decorated_definition" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| matches!(child.kind(), "function_definition" | "class_definition"))
        }
        _ => None,
    }
}
