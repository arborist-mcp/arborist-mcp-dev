use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::language::{
    contains_kind, contains_node, language_for_id, normalize_absolute_path, normalize_path,
    parse_document, position_from, read_source,
};
use crate::model::LanguageId;
use crate::model::QueryCaptureResult;
use crate::semantic::{c_semantic_path, c_symbol_id_for_node, semantic_parent_path, semantic_path};

pub fn execute_tree_query_from_path(path: &Path, query: &str) -> Result<Vec<QueryCaptureResult>> {
    let path = normalize_absolute_path(path)?;
    let source = read_source(&path)?;
    execute_tree_query(&path, &source, query)
}

pub fn execute_tree_query(
    path: &Path,
    source: &str,
    query: &str,
) -> Result<Vec<QueryCaptureResult>> {
    let document = parse_document(path, source)?;
    let language = language_for_id(document.language_id);
    let root = document.tree.root_node();
    let compiled = Query::new(&language, query)
        .with_context(|| format!("invalid Tree-sitter query for {}", normalize_path(path)))?;

    let mut cursor = QueryCursor::new();
    let mut captures = Vec::new();

    let mut query_captures = cursor.captures(&compiled, root, source.as_bytes());
    while let Some((query_match, capture_index)) = query_captures.next() {
        let capture = query_match.captures[*capture_index];
        let node = capture.node;
        let (owner_symbol_id, owner_semantic_path, owner_scope_path) =
            capture_owner(path, source, root, document.language_id, node)?;
        captures.push(QueryCaptureResult {
            capture_name: compiled.capture_names()[capture.index as usize].to_string(),
            node_kind: node.kind().to_string(),
            text: node.utf8_text(source.as_bytes())?.to_string(),
            owner_symbol_id,
            owner_semantic_path,
            owner_scope_path,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_point: position_from(node.start_position()),
            end_point: position_from(node.end_position()),
        });
    }

    Ok(captures)
}

fn capture_owner(
    path: &Path,
    source: &str,
    root: tree_sitter::Node<'_>,
    language_id: LanguageId,
    node: tree_sitter::Node<'_>,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    match language_id {
        LanguageId::Python => python_capture_owner(source, node),
        LanguageId::C => c_capture_owner(path, source, root, node),
    }
}

fn python_capture_owner(
    source: &str,
    node: tree_sitter::Node<'_>,
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

fn python_owner_symbol_node(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
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
    root: tree_sitter::Node<'_>,
    node: tree_sitter::Node<'_>,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let mut cursor = root.walk();

    for child in root.named_children(&mut cursor) {
        if !contains_node(child, node) {
            continue;
        }
        if !matches!(child.kind(), "function_definition" | "type_definition")
            && !(child.kind() == "declaration" && contains_kind(child, "function_declarator"))
        {
            continue;
        }

        let Some(owner_semantic_path) = c_semantic_path(path, child, source)? else {
            continue;
        };
        let owner_scope_path = semantic_parent_path(&owner_semantic_path);
        let owner_symbol_id = c_symbol_id_for_node(path, child, source)?
            .or_else(|| Some(owner_semantic_path.clone()));
        return Ok((owner_symbol_id, Some(owner_semantic_path), owner_scope_path));
    }

    Ok((None, None, None))
}
