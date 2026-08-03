use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::language::{self, ensure_path_inside_workspace};
use crate::model::*;
use crate::symbol_trace::TraceQueryDeadline;
use crate::{patching, symbols};

use super::super::{validate_neighborhood_context_patch_result, validate_patch_commit_with_trace};

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_neighborhood_context(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<NeighborhoodContextPatchResult> {
    validate_patch_with_neighborhood_context_with_timeout(
        workspace_root,
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
pub fn validate_patch_with_neighborhood_context_with_timeout(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<NeighborhoodContextPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let workspace_root = language::normalize_absolute_path(workspace_root)?;
    let path = language::normalize_absolute_path(path)?;
    ensure_path_inside_workspace(&workspace_root, &path)?;

    deadline.check("patch validation")?;
    let patch = super::super::patch_ast_node_with_trace_deadline(
        &deadline,
        &path,
        source,
        semantic_target,
        new_code,
        bypass_reason,
    )?;
    let trace_target = patch.resolved_symbol_id.clone();

    if !patch.validation.syntax_errors.is_empty() {
        deadline.check("patch validation result")?;
        let result = NeighborhoodContextPatchResult {
            patch,
            trace_target,
            trace: None,
            neighborhood_context: None,
            trace_validation: None,
            trace_error: Some(
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors().to_string(),
            ),
        };
        validate_neighborhood_context_patch_result(&result)?;
        return Ok(result);
    }

    if !patch.applied {
        deadline.check("patch validation result")?;
        let result = NeighborhoodContextPatchResult {
            patch,
            trace_target,
            trace: None,
            neighborhood_context: None,
            trace_validation: None,
            trace_error: Some(
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection().to_string(),
            ),
        };
        validate_neighborhood_context_patch_result(&result)?;
        return Ok(result);
    }

    deadline.check("patch neighborhood overrides")?;
    let mut overrides = BTreeMap::new();
    overrides.insert(patch.file.clone(), patch.updated_source.clone());
    let trace = symbols::trace_symbol_graph_with_overrides_with_deadline(
        &workspace_root,
        &overrides,
        &trace_target,
        direction,
        &deadline,
    )?;
    let timeout_ms = deadline.remaining_timeout_ms("patch neighborhood context")?;
    let neighborhood_context =
        symbols::read_symbol_neighborhood_context_with_overrides_with_timeout(
            &workspace_root,
            &overrides,
            &trace_target,
            direction,
            max_depth,
            max_nodes,
            timeout_ms,
        )?;
    deadline.check("patch neighborhood trace validation")?;
    let trace_validation = validate_patch_commit_with_trace(&patch, &trace)?;

    let result = NeighborhoodContextPatchResult {
        patch,
        trace_target,
        trace: Some(trace),
        neighborhood_context: Some(neighborhood_context),
        trace_validation: Some(trace_validation),
        trace_error: None,
    };
    deadline.check("neighborhood-context patch result")?;
    validate_neighborhood_context_patch_result(&result)?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_neighborhood_context_at_position(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<NeighborhoodContextPatchResult> {
    validate_patch_with_neighborhood_context_at_position_with_timeout(
        workspace_root,
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
pub fn validate_patch_with_neighborhood_context_at_position_with_timeout(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<NeighborhoodContextPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("patch position resolution")?;
    let semantic_target = patching::semantic_target_at_position_with_deadline(
        path,
        source,
        position,
        Some(&deadline),
    )?;
    let timeout_ms = deadline.remaining_timeout_ms("neighborhood-context patch validation")?;
    validate_patch_with_neighborhood_context_with_timeout(
        workspace_root,
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
