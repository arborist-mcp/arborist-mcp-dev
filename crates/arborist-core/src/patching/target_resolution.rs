use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::Node;

use super::python_replacement::{
    normalize_python_replacement_indentation, python_replacement_starts_with_decorator,
};
use crate::language::{
    ParsedDocument, normalize_absolute_path, offset_for_position, parse_document, position_from,
};
use crate::model::{LanguageId, Position, ValidationIssue};
use crate::semantic::{
    ascend_to_symbol, c_semantic_path, c_symbol_id_for_node, find_semantic_node, semantic_path,
};

pub(crate) struct PreparedPatchReplacement {
    pub(crate) start_byte: usize,
    pub(crate) end_byte: usize,
    pub(crate) replacement: String,
    pub(crate) validation_issues: Vec<ValidationIssue>,
}

struct SemanticTargetInfo {
    language_id: LanguageId,
    start_byte: usize,
    end_byte: usize,
    node_kind: String,
    start_point: Position,
    end_point: Position,
}

pub(crate) fn semantic_target_at_position(
    path: &Path,
    source: &str,
    position: &Position,
) -> Result<String> {
    let path = normalize_absolute_path(path)?;
    let document = parse_document(&path, source)?;
    let byte_offset = offset_for_position(source, position)?;
    let node =
        node_at_byte_offset(document.tree.root_node(), source, byte_offset).ok_or_else(|| {
            anyhow!(
                "position {}:{} does not resolve to a syntax node in {}",
                position.row,
                position.column,
                path.display()
            )
        })?;
    let symbol_node = ascend_to_symbol(document.language_id, node).ok_or_else(|| {
        anyhow!(
            "position {}:{} does not resolve to a semantic symbol in {}",
            position.row,
            position.column,
            path.display()
        )
    })?;

    match document.language_id {
        LanguageId::Python => semantic_path(symbol_node, source),
        LanguageId::C | LanguageId::Cpp => c_symbol_id_for_node(&path, symbol_node, source)?
            .ok_or_else(|| anyhow!("position does not resolve to a C symbol id")),
    }
}

pub(crate) fn prepare_patch_replacement(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
) -> Result<PreparedPatchReplacement> {
    let target = semantic_target_info(path, source, semantic_target)?;
    let replacement = match target.language_id {
        LanguageId::Python => normalize_python_replacement_indentation(
            source,
            target.start_byte,
            target.end_byte,
            new_code,
        ),
        LanguageId::C | LanguageId::Cpp => new_code.to_string(),
    };
    let mut validation_issues = Vec::new();
    if target.language_id == LanguageId::Python
        && target.node_kind == "decorated_definition"
        && !python_replacement_starts_with_decorator(&replacement)
    {
        validation_issues.push(ValidationIssue {
            kind: "decorator_guard".to_string(),
            message: "replacement would remove existing Python decorator(s); include decorators in new_code or provide an explicit bypass_reason".to_string(),
            start_byte: target.start_byte,
            end_byte: target.end_byte,
            start_point: target.start_point,
            end_point: target.end_point,
        });
    }

    Ok(PreparedPatchReplacement {
        start_byte: target.start_byte,
        end_byte: target.end_byte,
        replacement,
        validation_issues,
    })
}

pub(super) fn locate_patched_symbol<'tree>(
    document: &'tree ParsedDocument,
    source: &str,
    patch_start: usize,
    replacement_len: usize,
) -> Option<Node<'tree>> {
    let patch_end = replacement_content_end(source, patch_start, replacement_len)?;
    let root = document.tree.root_node();
    let descendant = root
        .named_descendant_for_byte_range(patch_start, patch_end)
        .or_else(|| root.named_descendant_for_byte_range(patch_start, patch_start))?;
    ascend_to_symbol(document.language_id, descendant)
}

pub(super) fn resolve_symbol_path(
    path: &Path,
    language_id: LanguageId,
    node: Node<'_>,
    source: &str,
) -> Result<String> {
    match language_id {
        LanguageId::Python => semantic_path(node, source),
        LanguageId::C | LanguageId::Cpp => c_semantic_path(path, node, source)?
            .ok_or_else(|| anyhow!("failed to resolve patched C symbol path")),
    }
}

pub(super) fn resolve_symbol_id(
    path: &Path,
    language_id: LanguageId,
    node: Node<'_>,
    source: &str,
) -> Result<String> {
    match language_id {
        LanguageId::Python => semantic_path(node, source),
        LanguageId::C | LanguageId::Cpp => c_symbol_id_for_node(path, node, source)?
            .ok_or_else(|| anyhow!("failed to resolve patched C symbol id")),
    }
}

fn semantic_target_info(
    path: &Path,
    source: &str,
    semantic_target: &str,
) -> Result<SemanticTargetInfo> {
    validate_semantic_target(semantic_target)?;
    let document = parse_document(path, source)?;
    let target_node = find_semantic_node(
        document.language_id,
        path,
        &document.tree,
        source,
        semantic_target,
    )?
    .ok_or_else(|| anyhow!("semantic path not found: {semantic_target}"))?;
    let target_node = python_symbol_replacement_node(document.language_id, target_node);

    Ok(SemanticTargetInfo {
        language_id: document.language_id,
        start_byte: target_node.start_byte(),
        end_byte: target_node.end_byte(),
        node_kind: target_node.kind().to_string(),
        start_point: position_from(target_node.start_position()),
        end_point: position_from(target_node.end_position()),
    })
}

fn validate_semantic_target(semantic_target: &str) -> Result<()> {
    if semantic_target.trim().is_empty() {
        bail!("invalid semantic target: selector must not be blank");
    }
    Ok(())
}

fn node_at_byte_offset<'tree>(
    root: Node<'tree>,
    source: &str,
    byte_offset: usize,
) -> Option<Node<'tree>> {
    root.named_descendant_for_byte_range(byte_offset, byte_offset)
        .or_else(|| {
            byte_offset
                .checked_sub(1)
                .and_then(|offset| root.named_descendant_for_byte_range(offset, offset))
        })
        .or_else(|| {
            if byte_offset < source.len() {
                root.descendant_for_byte_range(byte_offset, byte_offset)
            } else {
                byte_offset
                    .checked_sub(1)
                    .and_then(|offset| root.descendant_for_byte_range(offset, offset))
            }
        })
}

fn replacement_content_end(
    source: &str,
    patch_start: usize,
    replacement_len: usize,
) -> Option<usize> {
    let patch_end = patch_start.checked_add(replacement_len)?;
    let replacement = source.get(patch_start..patch_end)?;
    let content_len = replacement.trim_end().len();
    if content_len == 0 {
        return Some(patch_start);
    }
    Some(patch_start + content_len - 1)
}

fn python_symbol_replacement_node<'tree>(
    language_id: LanguageId,
    node: Node<'tree>,
) -> Node<'tree> {
    if language_id == LanguageId::Python
        && let Some(parent) = node.parent()
        && parent.kind() == "decorated_definition"
    {
        return parent;
    }

    node
}
