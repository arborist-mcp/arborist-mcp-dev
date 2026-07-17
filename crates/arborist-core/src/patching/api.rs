use std::path::Path;

use anyhow::{Context, Result};

use crate::language::{normalize_absolute_path, normalize_path, read_source, write_source_atomic};
use crate::model::{PatchAstNodeResult, PatchPreviewResult, Position};

use super::{
    build_patch_result, prepare_patch_replacement, semantic_target_at_position, splice_source,
    validate_bypass_reason, validate_patch_replacement,
};

pub fn patch_ast_node_from_path(
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchAstNodeResult> {
    let path = normalize_absolute_path(path)?;
    let disk_source = read_source(&path)?;
    let result = patch_ast_node(
        &path,
        &disk_source,
        semantic_target,
        new_code,
        bypass_reason,
    )?;

    if result.applied {
        write_source_atomic(&path, &result.updated_source)
            .with_context(|| format!("failed to write patched source to {}", path.display()))?;
    }

    Ok(result)
}

pub fn patch_ast_node_at_position_from_path(
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchAstNodeResult> {
    let path = normalize_absolute_path(path)?;
    let disk_source = read_source(&path)?;
    patch_ast_node_at_position(&path, &disk_source, position, new_code, bypass_reason)
}

pub fn preview_patch_ast_node_from_path(
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchPreviewResult> {
    let path = normalize_absolute_path(path)?;
    let disk_source = read_source(&path)?;
    preview_patch_ast_node(
        &path,
        &disk_source,
        semantic_target,
        new_code,
        bypass_reason,
    )
}

pub fn preview_patch_ast_node_at_position_from_path(
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchPreviewResult> {
    let path = normalize_absolute_path(path)?;
    let disk_source = read_source(&path)?;
    preview_patch_ast_node_at_position(&path, &disk_source, position, new_code, bypass_reason)
}

pub fn patch_ast_node(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchAstNodeResult> {
    let path = normalize_absolute_path(path)?;
    validate_patch_replacement(new_code)?;
    validate_bypass_reason(bypass_reason)?;
    let prepared = prepare_patch_replacement(&path, source, semantic_target, new_code)?;
    let updated_source = splice_source(
        source,
        prepared.start_byte..prepared.end_byte,
        &prepared.replacement,
    );
    build_patch_result(
        &path,
        semantic_target,
        updated_source,
        bypass_reason,
        prepared.start_byte,
        prepared.replacement.len(),
        prepared.validation_issues,
    )
}

pub fn preview_patch_ast_node(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchPreviewResult> {
    let path = normalize_absolute_path(path)?;
    let patch = patch_ast_node(&path, source, semantic_target, new_code, bypass_reason)?;
    build_patch_preview_result(&path, source, patch)
}

pub fn patch_ast_node_at_position(
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchAstNodeResult> {
    let path = normalize_absolute_path(path)?;
    let semantic_target = semantic_target_at_position(&path, source, position)?;
    patch_ast_node(&path, source, &semantic_target, new_code, bypass_reason)
}

pub fn preview_patch_ast_node_at_position(
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchPreviewResult> {
    let path = normalize_absolute_path(path)?;
    let semantic_target = semantic_target_at_position(&path, source, position)?;
    preview_patch_ast_node(&path, source, &semantic_target, new_code, bypass_reason)
}

fn build_patch_preview_result(
    path: &Path,
    source: &str,
    patch: PatchAstNodeResult,
) -> Result<PatchPreviewResult> {
    let unified_diff = unified_diff(path, source, &patch.updated_source);
    let result = PatchPreviewResult {
        patch,
        changed: !unified_diff.is_empty(),
        unified_diff,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn unified_diff(path: &Path, old_source: &str, new_source: &str) -> String {
    if old_source == new_source {
        return String::new();
    }

    let old_lines: Vec<&str> = old_source.lines().collect();
    let new_lines: Vec<&str> = new_source.lines().collect();
    let mut prefix_len = 0;
    while prefix_len < old_lines.len()
        && prefix_len < new_lines.len()
        && old_lines[prefix_len] == new_lines[prefix_len]
    {
        prefix_len += 1;
    }

    let mut suffix_len = 0;
    while suffix_len + prefix_len < old_lines.len()
        && suffix_len + prefix_len < new_lines.len()
        && old_lines[old_lines.len() - 1 - suffix_len]
            == new_lines[new_lines.len() - 1 - suffix_len]
    {
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
        diff.push('-');
        diff.push_str(line);
        diff.push('\n');
    }
    for line in new_changed {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    diff
}
