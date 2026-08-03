use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::Node;

use super::python_replacement::{
    normalize_python_replacement_indentation, python_replacement_starts_with_decorator,
};
use crate::deadline::DeadlineCheck;
use crate::language::{
    ParsedDocument, builtin_language_registry, normalize_absolute_path, offset_for_position,
    parse_document, position_from,
};
use crate::model::{LanguageId, Position, ValidationIssue};
use crate::semantic::{ascend_to_symbol, find_semantic_node_with_deadline};

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

pub(crate) fn semantic_target_at_position_with_deadline(
    path: &Path,
    source: &str,
    position: &Position,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<String> {
    let path = normalize_absolute_path(path)?;
    check_deadline(deadline, "position target parse")?;
    let document = parse_document(&path, source)?;
    check_deadline(deadline, "position target lookup")?;
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

    check_deadline(deadline, "position target resolution")?;
    builtin_language_registry()
        .adapter(document.language_id)
        .expect("every LanguageId must have a builtin language adapter")
        .symbol_id_for_node(&path, symbol_node, source, deadline)?
        .ok_or_else(|| anyhow!("position does not resolve to a C symbol id"))
}

pub(crate) fn prepare_patch_replacement(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
) -> Result<PreparedPatchReplacement> {
    prepare_patch_replacement_with_deadline(path, source, semantic_target, new_code, None)
}

pub(crate) fn prepare_patch_replacement_with_deadline(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<PreparedPatchReplacement> {
    let target = semantic_target_info(path, source, semantic_target, deadline)?;
    check_deadline(deadline, "patch replacement preparation")?;
    let replacement = match target.language_id {
        LanguageId::Python => normalize_python_replacement_indentation(
            source,
            target.start_byte,
            target.end_byte,
            target.node_kind == "decorated_definition",
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

    check_deadline(deadline, "patch replacement validation")?;
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
    builtin_language_registry()
        .adapter(language_id)
        .expect("every LanguageId must have a builtin language adapter")
        .semantic_path_for_node(path, node, source)?
        .ok_or_else(|| anyhow!("failed to resolve patched C symbol path"))
}

pub(super) fn resolve_symbol_id(
    path: &Path,
    language_id: LanguageId,
    node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<String> {
    builtin_language_registry()
        .adapter(language_id)
        .expect("every LanguageId must have a builtin language adapter")
        .symbol_id_for_node(path, node, source, deadline)?
        .ok_or_else(|| anyhow!("failed to resolve patched C symbol id"))
}

fn semantic_target_info(
    path: &Path,
    source: &str,
    semantic_target: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<SemanticTargetInfo> {
    validate_semantic_target(semantic_target)?;
    check_deadline(deadline, "semantic target parse")?;
    let document = parse_document(path, source)?;
    check_deadline(deadline, "semantic target lookup")?;
    let target_node = find_semantic_node_with_deadline(
        document.language_id,
        path,
        &document.tree,
        source,
        semantic_target,
        deadline,
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

fn check_deadline(deadline: Option<&dyn DeadlineCheck>, phase: &str) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::{Result, bail};

    use super::semantic_target_at_position_with_deadline;
    use crate::deadline::DeadlineCheck;
    use crate::model::Position;

    struct RejectOverloadAliasScan;

    impl DeadlineCheck for RejectOverloadAliasScan {
        fn check(&self, phase: &str) -> Result<()> {
            if phase == "collecting Python overload aliases" {
                bail!("deadline check reached {phase}")
            }
            Ok(())
        }
    }

    #[test]
    fn position_target_resolution_forwards_deadline_to_python_overload_alias_scans() {
        let source = r#"from typing import overload as typed_overload

class Store:
    @typed_overload
    def get(self, key: str) -> str: ...

    def get(self, key):
        return key
"#;

        let error = semantic_target_at_position_with_deadline(
            Path::new("sample.py"),
            source,
            &Position { row: 4, column: 8 },
            Some(&RejectOverloadAliasScan),
        )
        .expect_err(
            "position target resolution must check overload alias scans against its deadline",
        );

        assert!(
            error
                .to_string()
                .contains("collecting Python overload aliases")
        );
    }
}
