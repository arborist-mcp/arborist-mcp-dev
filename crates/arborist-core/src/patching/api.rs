use std::path::Path;

use anyhow::{Context, Result};

use crate::deadline::{CooperativeDeadline, DeadlineCheck};
use crate::language::{
    normalize_absolute_path, normalize_path, read_source, validate_source_length,
    write_source_atomic,
};
use crate::model::{PatchAstNodeResult, PatchPreviewResult, Position};

use super::{
    MAX_PATCH_PREVIEW_TIMEOUT_MS, PatchBuildInput, build_patch_result,
    build_patch_result_with_deadline, patch_deadline, prepare_patch_replacement,
    prepare_patch_replacement_with_deadline, semantic_target_at_position_with_deadline,
    splice_source, validate_bypass_reason, validate_patch_replacement,
};

pub fn patch_ast_node_from_path(
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchAstNodeResult> {
    patch_ast_node_from_path_with_timeout(path, semantic_target, new_code, bypass_reason, None)
}

pub fn patch_ast_node_from_path_with_timeout(
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<PatchAstNodeResult> {
    let deadline = patch_deadline(timeout_ms)?;
    deadline.check("path validation")?;
    let path = normalize_absolute_path(path)?;
    deadline.check("source read")?;
    let disk_source = read_source(&path)?;
    deadline.check("patch validation")?;
    let result = patch_ast_node_with_deadline(
        &path,
        &disk_source,
        semantic_target,
        new_code,
        bypass_reason,
        Some(&deadline),
    )?;

    write_applied_patch_result(&path, &result, &deadline)?;
    Ok(result)
}

pub fn patch_ast_node_at_position_from_path(
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchAstNodeResult> {
    patch_ast_node_at_position_from_path_with_timeout(path, position, new_code, bypass_reason, None)
}

pub fn patch_ast_node_at_position_from_path_with_timeout(
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<PatchAstNodeResult> {
    let deadline = patch_deadline(timeout_ms)?;
    deadline.check("path validation")?;
    let path = normalize_absolute_path(path)?;
    deadline.check("source read")?;
    let disk_source = read_source(&path)?;
    deadline.check("position patch validation")?;
    let result = patch_ast_node_at_position_with_deadline(
        &path,
        &disk_source,
        position,
        new_code,
        bypass_reason,
        Some(&deadline),
    )?;

    write_applied_patch_result(&path, &result, &deadline)?;
    Ok(result)
}

fn write_applied_patch_result(
    path: &Path,
    result: &PatchAstNodeResult,
    deadline: &CooperativeDeadline,
) -> Result<()> {
    if !result.applied {
        return Ok(());
    }

    deadline.check("source write")?;
    // Once persistence starts, report its outcome instead of a timeout after the source changed.
    write_source_atomic(path, &result.updated_source)
        .with_context(|| format!("failed to write patched source to {}", path.display()))
}

pub fn preview_patch_ast_node_from_path(
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchPreviewResult> {
    preview_patch_ast_node_from_path_with_timeout(
        path,
        semantic_target,
        new_code,
        bypass_reason,
        None,
    )
}

pub fn preview_patch_ast_node_from_path_with_timeout(
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<PatchPreviewResult> {
    let deadline = patch_preview_deadline(timeout_ms)?;
    let path = normalize_absolute_path(path)?;
    deadline.check("source read")?;
    let disk_source = read_source(&path)?;
    deadline.check("source validation")?;
    preview_patch_ast_node_with_deadline(
        &path,
        &disk_source,
        semantic_target,
        new_code,
        bypass_reason,
        &deadline,
    )
}

pub fn preview_patch_ast_node_at_position_from_path(
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchPreviewResult> {
    preview_patch_ast_node_at_position_from_path_with_timeout(
        path,
        position,
        new_code,
        bypass_reason,
        None,
    )
}

pub fn preview_patch_ast_node_at_position_from_path_with_timeout(
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<PatchPreviewResult> {
    let deadline = patch_preview_deadline(timeout_ms)?;
    let path = normalize_absolute_path(path)?;
    deadline.check("source read")?;
    let disk_source = read_source(&path)?;
    deadline.check("source validation")?;
    preview_patch_ast_node_at_position_with_deadline(
        &path,
        &disk_source,
        position,
        new_code,
        bypass_reason,
        &deadline,
    )
}

pub fn patch_ast_node(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchAstNodeResult> {
    patch_ast_node_with_timeout(path, source, semantic_target, new_code, bypass_reason, None)
}

pub fn patch_ast_node_with_timeout(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<PatchAstNodeResult> {
    let deadline = patch_deadline(timeout_ms)?;
    patch_ast_node_with_deadline(
        path,
        source,
        semantic_target,
        new_code,
        bypass_reason,
        Some(&deadline),
    )
}

fn patch_ast_node_with_deadline(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<PatchAstNodeResult> {
    check_deadline(deadline, "patch input validation")?;
    let path = normalize_absolute_path(path)?;
    validate_patch_replacement(new_code)?;
    validate_bypass_reason(bypass_reason)?;
    let prepared = match deadline {
        Some(deadline) => prepare_patch_replacement_with_deadline(
            &path,
            source,
            semantic_target,
            new_code,
            Some(deadline),
        )?,
        None => prepare_patch_replacement(&path, source, semantic_target, new_code)?,
    };
    let result_len = source
        .len()
        .checked_sub(prepared.end_byte - prepared.start_byte)
        .and_then(|length| length.checked_add(prepared.replacement.len()))
        .ok_or_else(|| anyhow::anyhow!("updated source size overflowed"))?;
    validate_source_length(&path, result_len)?;
    check_deadline(deadline, "source replacement")?;
    let updated_source = splice_source(
        source,
        prepared.start_byte..prepared.end_byte,
        &prepared.replacement,
    );
    match deadline {
        Some(deadline) => build_patch_result_with_deadline(
            PatchBuildInput {
                path: &path,
                semantic_target,
                updated_source,
                bypass_reason,
                patch_start: prepared.start_byte,
                replacement_len: prepared.replacement.len(),
                preflight_issues: prepared.validation_issues,
            },
            Some(deadline),
        ),
        None => build_patch_result(
            &path,
            semantic_target,
            updated_source,
            bypass_reason,
            prepared.start_byte,
            prepared.replacement.len(),
            prepared.validation_issues,
        ),
    }
}

pub fn preview_patch_ast_node(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchPreviewResult> {
    preview_patch_ast_node_with_timeout(
        path,
        source,
        semantic_target,
        new_code,
        bypass_reason,
        None,
    )
}

pub fn preview_patch_ast_node_with_timeout(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<PatchPreviewResult> {
    let deadline = patch_preview_deadline(timeout_ms)?;
    preview_patch_ast_node_with_deadline(
        path,
        source,
        semantic_target,
        new_code,
        bypass_reason,
        &deadline,
    )
}

fn preview_patch_ast_node_with_deadline(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    deadline: &CooperativeDeadline,
) -> Result<PatchPreviewResult> {
    deadline.check("patch input validation")?;
    let path = normalize_absolute_path(path)?;
    let patch = patch_ast_node_with_deadline(
        &path,
        source,
        semantic_target,
        new_code,
        bypass_reason,
        Some(deadline),
    )?;
    build_patch_preview_result_with_deadline(&path, source, patch, deadline)
}

pub fn patch_ast_node_at_position(
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchAstNodeResult> {
    patch_ast_node_at_position_with_timeout(path, source, position, new_code, bypass_reason, None)
}

pub fn patch_ast_node_at_position_with_timeout(
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<PatchAstNodeResult> {
    let deadline = patch_deadline(timeout_ms)?;
    patch_ast_node_at_position_with_deadline(
        path,
        source,
        position,
        new_code,
        bypass_reason,
        Some(&deadline),
    )
}

fn patch_ast_node_at_position_with_deadline(
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<PatchAstNodeResult> {
    check_deadline(deadline, "position target resolution")?;
    let path = normalize_absolute_path(path)?;
    let semantic_target =
        semantic_target_at_position_with_deadline(&path, source, position, deadline)?;
    patch_ast_node_with_deadline(
        &path,
        source,
        &semantic_target,
        new_code,
        bypass_reason,
        deadline,
    )
}

pub fn preview_patch_ast_node_at_position(
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchPreviewResult> {
    preview_patch_ast_node_at_position_with_timeout(
        path,
        source,
        position,
        new_code,
        bypass_reason,
        None,
    )
}

pub fn preview_patch_ast_node_at_position_with_timeout(
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<PatchPreviewResult> {
    let deadline = patch_preview_deadline(timeout_ms)?;
    preview_patch_ast_node_at_position_with_deadline(
        path,
        source,
        position,
        new_code,
        bypass_reason,
        &deadline,
    )
}

fn preview_patch_ast_node_at_position_with_deadline(
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    deadline: &CooperativeDeadline,
) -> Result<PatchPreviewResult> {
    deadline.check("position target resolution")?;
    let path = normalize_absolute_path(path)?;
    let semantic_target =
        semantic_target_at_position_with_deadline(&path, source, position, Some(deadline))?;
    preview_patch_ast_node_with_deadline(
        &path,
        source,
        &semantic_target,
        new_code,
        bypass_reason,
        deadline,
    )
}

fn build_patch_preview_result_with_deadline(
    path: &Path,
    source: &str,
    patch: PatchAstNodeResult,
    deadline: &CooperativeDeadline,
) -> Result<PatchPreviewResult> {
    deadline.check("unified diff")?;
    let unified_diff =
        unified_diff_with_deadline(path, source, &patch.updated_source, Some(deadline))?;
    let result = PatchPreviewResult {
        patch,
        changed: !unified_diff.is_empty(),
        unified_diff,
    };
    deadline.check("patch preview result")?;
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn unified_diff(path: &Path, old_source: &str, new_source: &str) -> String {
    unified_diff_with_deadline(path, old_source, new_source, None)
        .expect("deadline-free diff generation cannot fail")
}

fn unified_diff_with_deadline(
    path: &Path,
    old_source: &str,
    new_source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<String> {
    check_deadline(deadline, "unified diff")?;
    if old_source == new_source {
        return Ok(String::new());
    }

    let old_lines: Vec<&str> = old_source.lines().collect();
    check_deadline(deadline, "unified diff line collection")?;
    let new_lines: Vec<&str> = new_source.lines().collect();
    check_deadline(deadline, "unified diff line collection")?;
    let mut prefix_len = 0;
    while prefix_len < old_lines.len()
        && prefix_len < new_lines.len()
        && old_lines[prefix_len] == new_lines[prefix_len]
    {
        check_deadline(deadline, "unified diff prefix")?;
        prefix_len += 1;
    }

    let mut suffix_len = 0;
    while suffix_len + prefix_len < old_lines.len()
        && suffix_len + prefix_len < new_lines.len()
        && old_lines[old_lines.len() - 1 - suffix_len]
            == new_lines[new_lines.len() - 1 - suffix_len]
    {
        check_deadline(deadline, "unified diff suffix")?;
        suffix_len += 1;
    }

    let old_changed = &old_lines[prefix_len..old_lines.len() - suffix_len];
    let new_changed = &new_lines[prefix_len..new_lines.len() - suffix_len];
    let old_start = prefix_len + 1;
    let new_start = prefix_len + 1;
    let path = normalize_path(path).trim_start_matches('/').to_string();
    let mut diff = format!(
        "--- a/{path}\n+++ b/{path}\n@@ -{},{} +{},{} @@\n",
        old_start,
        old_changed.len(),
        new_start,
        new_changed.len()
    );

    for line in old_changed {
        check_deadline(deadline, "unified diff removals")?;
        diff.push('-');
        diff.push_str(line);
        diff.push('\n');
    }
    for line in new_changed {
        check_deadline(deadline, "unified diff additions")?;
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    Ok(diff)
}

fn patch_preview_deadline(timeout_ms: Option<u64>) -> Result<CooperativeDeadline> {
    CooperativeDeadline::new(timeout_ms, MAX_PATCH_PREVIEW_TIMEOUT_MS, "patch preview")
}

fn check_deadline(deadline: Option<&dyn DeadlineCheck>, phase: &str) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(())
}
