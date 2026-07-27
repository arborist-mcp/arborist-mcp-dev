use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::language::{self, ensure_path_inside_workspace, read_source};
use crate::model::*;
use crate::patching::patch_ast_node;
use crate::symbol_trace::TraceQueryDeadline;
use crate::{patching, symbols};

use super::super::{
    validate_graph_backed_patch_result, validate_patch_commit_with_trace,
    validate_patch_with_graph_context_at_position_with_timeout,
    validate_patch_with_graph_context_with_timeout,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context_from_path(
    workspace_root: &Path,
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<GraphBackedPatchResult> {
    validate_patch_with_graph_context_from_path_with_timeout(
        workspace_root,
        path,
        semantic_target,
        new_code,
        bypass_reason,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context_from_path_with_timeout(
    workspace_root: &Path,
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<GraphBackedPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let workspace_root = language::normalize_absolute_path(workspace_root)?;
    let path = language::normalize_absolute_path(path)?;
    ensure_path_inside_workspace(&workspace_root, &path)?;
    deadline.check("patch source read")?;
    let source = read_source(&path)?;
    let timeout_ms = deadline.remaining_timeout_ms("graph-backed patch validation")?;
    validate_patch_with_graph_context_with_timeout(
        &workspace_root,
        &path,
        &source,
        semantic_target,
        new_code,
        bypass_reason,
        direction,
        max_depth,
        max_nodes,
        timeout_ms,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context_from_index(
    db_path: &Path,
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<GraphBackedPatchResult> {
    validate_patch_with_graph_context_from_index_with_timeout(
        db_path,
        path,
        source,
        semantic_target,
        new_code,
        bypass_reason,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context_from_index_with_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<GraphBackedPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let path = language::normalize_absolute_path(path)?;
    deadline.check("indexed patch validation")?;
    let patch = patch_ast_node(&path, source, semantic_target, new_code, bypass_reason)?;
    let trace_target = patch.resolved_symbol_id.clone();

    if !patch.validation.syntax_errors.is_empty() {
        deadline.check("indexed patch validation result")?;
        let result = GraphBackedPatchResult {
            patch,
            trace_target,
            trace: None,
            neighborhood: None,
            trace_validation: None,
            trace_error: Some(
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors().to_string(),
            ),
        };
        validate_graph_backed_patch_result(&result)?;
        return Ok(result);
    }

    if !patch.applied {
        deadline.check("indexed patch validation result")?;
        let result = GraphBackedPatchResult {
            patch,
            trace_target,
            trace: None,
            neighborhood: None,
            trace_validation: None,
            trace_error: Some(
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection().to_string(),
            ),
        };
        validate_graph_backed_patch_result(&result)?;
        return Ok(result);
    }

    deadline.check("indexed patch graph overrides")?;
    let overrides = BTreeMap::from([(patch.file.clone(), patch.updated_source.clone())]);
    let timeout_ms = deadline.remaining_timeout_ms("indexed patch graph trace")?;
    let trace = symbols::trace_symbol_graph_from_index_with_overrides_and_timeout(
        db_path,
        &overrides,
        &trace_target,
        direction,
        timeout_ms,
    )?;
    let timeout_ms = deadline.remaining_timeout_ms("indexed patch graph neighborhood")?;
    let neighborhood = symbols::trace_symbol_neighborhood_from_index_with_overrides_and_timeout(
        db_path,
        &overrides,
        &trace_target,
        direction,
        max_depth,
        max_nodes,
        timeout_ms,
    )?;
    deadline.check("indexed patch graph trace validation")?;
    let trace_validation = validate_patch_commit_with_trace(&patch, &trace)?;

    let result = GraphBackedPatchResult {
        patch,
        trace_target,
        trace: Some(trace),
        neighborhood: Some(neighborhood),
        trace_validation: Some(trace_validation),
        trace_error: None,
    };
    deadline.check("indexed graph-backed patch result")?;
    validate_graph_backed_patch_result(&result)?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context_from_index_path_with_timeout(
    db_path: &Path,
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<GraphBackedPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let path = language::normalize_absolute_path(path)?;
    deadline.check("indexed patch source read")?;
    let source = read_source(&path)?;
    let timeout_ms = deadline.remaining_timeout_ms("indexed graph-backed patch validation")?;
    validate_patch_with_graph_context_from_index_with_timeout(
        db_path,
        &path,
        &source,
        semantic_target,
        new_code,
        bypass_reason,
        direction,
        max_depth,
        max_nodes,
        timeout_ms,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context_at_position_from_path(
    workspace_root: &Path,
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<GraphBackedPatchResult> {
    validate_patch_with_graph_context_at_position_from_path_with_timeout(
        workspace_root,
        path,
        position,
        new_code,
        bypass_reason,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context_at_position_from_path_with_timeout(
    workspace_root: &Path,
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<GraphBackedPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let workspace_root = language::normalize_absolute_path(workspace_root)?;
    let path = language::normalize_absolute_path(path)?;
    ensure_path_inside_workspace(&workspace_root, &path)?;
    deadline.check("patch source read")?;
    let source = read_source(&path)?;
    let timeout_ms = deadline.remaining_timeout_ms("position graph-backed patch validation")?;
    validate_patch_with_graph_context_at_position_with_timeout(
        &workspace_root,
        &path,
        &source,
        position,
        new_code,
        bypass_reason,
        direction,
        max_depth,
        max_nodes,
        timeout_ms,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context_at_position_from_index(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<GraphBackedPatchResult> {
    validate_patch_with_graph_context_at_position_from_index_with_timeout(
        db_path,
        path,
        source,
        position,
        new_code,
        bypass_reason,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context_at_position_from_index_with_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<GraphBackedPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("indexed patch position resolution")?;
    let semantic_target = patching::semantic_target_at_position(path, source, position)?;
    let timeout_ms = deadline.remaining_timeout_ms("indexed graph-backed patch validation")?;
    validate_patch_with_graph_context_from_index_with_timeout(
        db_path,
        path,
        source,
        &semantic_target,
        new_code,
        bypass_reason,
        direction,
        max_depth,
        max_nodes,
        timeout_ms,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context_at_position_from_index_path_with_timeout(
    db_path: &Path,
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<GraphBackedPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let path = language::normalize_absolute_path(path)?;
    deadline.check("indexed patch source read")?;
    let source = read_source(&path)?;
    let timeout_ms = deadline.remaining_timeout_ms("indexed position patch validation")?;
    validate_patch_with_graph_context_at_position_from_index_with_timeout(
        db_path,
        &path,
        &source,
        position,
        new_code,
        bypass_reason,
        direction,
        max_depth,
        max_nodes,
        timeout_ms,
    )
}
