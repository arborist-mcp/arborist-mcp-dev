use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::contains_node;
use crate::model::LanguageId;
use crate::semantic::{c_semantic_path, c_symbol_id_for_node, semantic_parent_path, semantic_path};

pub(super) fn capture_owner(
    path: &Path,
    source: &str,
    language_id: LanguageId,
    node: Node<'_>,
    c_symbols: Option<&[Node<'_>]>,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    match language_id {
        LanguageId::Python => python_capture_owner(source, node),
        LanguageId::C | LanguageId::Cpp => {
            c_capture_owner(path, source, node, c_symbols.unwrap_or_default())
        }
    }
}

fn python_capture_owner(
    source: &str,
    node: Node<'_>,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let Some(owner_node) = python_owner_symbol_node(node) else {
        return Ok((None, None, None));
    };

    let owner_semantic_path = semantic_path(owner_node, source)?;
    let owner_scope_path = semantic_parent_path(&owner_semantic_path);
    Ok((
        Some(owner_semantic_path.clone()),
        Some(owner_semantic_path.clone()),
        owner_scope_path,
    ))
}

fn python_owner_symbol_node(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(node);

    while let Some(candidate) = current {
        match candidate.kind() {
            "function_definition" | "class_definition" => return Some(candidate),
            "decorated_definition" => {
                let mut cursor = candidate.walk();
                return candidate.named_children(&mut cursor).find(|child| {
                    matches!(child.kind(), "function_definition" | "class_definition")
                });
            }
            _ => current = candidate.parent(),
        }
    }

    None
}

fn c_capture_owner(
    path: &Path,
    source: &str,
    node: Node<'_>,
    c_symbols: &[Node<'_>],
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let mut owner_node = None;
    for &child in c_symbols {
        if contains_node(child, node)
            && owner_node.is_none_or(|current: Node<'_>| {
                child.end_byte() - child.start_byte() < current.end_byte() - current.start_byte()
            })
        {
            owner_node = Some(child);
        }
    }

    let Some(owner_node) = owner_node else {
        return Ok((None, None, None));
    };
    let Some(owner_semantic_path) = c_semantic_path(path, owner_node, source)? else {
        return Ok((None, None, None));
    };
    let owner_scope_path = semantic_parent_path(&owner_semantic_path);
    let owner_symbol_id = c_symbol_id_for_node(path, owner_node, source)?
        .or_else(|| Some(owner_semantic_path.clone()));

    Ok((owner_symbol_id, Some(owner_semantic_path), owner_scope_path))
}
