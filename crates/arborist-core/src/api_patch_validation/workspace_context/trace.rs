use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::language::{self, ensure_path_inside_workspace};
use crate::model::*;
use crate::symbol_trace::TraceQueryDeadline;
use crate::{patching, symbols};

use super::super::{
    trace_patch_impact_summary, validate_patch_commit_with_trace,
    validate_trace_backed_patch_result,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_trace_context(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
) -> Result<TraceBackedPatchResult> {
    validate_patch_with_trace_context_with_timeout(
        workspace_root,
        path,
        source,
        semantic_target,
        new_code,
        bypass_reason,
        direction,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_trace_context_with_timeout(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceBackedPatchResult> {
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
        let result = TraceBackedPatchResult {
            patch,
            trace_target,
            trace: None,
            trace_validation: None,
            impact: None,
            trace_error: Some(
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors().to_string(),
            ),
        };
        validate_trace_backed_patch_result(&result)?;
        return Ok(result);
    }

    if !patch.applied {
        deadline.check("patch validation result")?;
        let result = TraceBackedPatchResult {
            patch,
            trace_target,
            trace: None,
            trace_validation: None,
            impact: None,
            trace_error: Some(
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection().to_string(),
            ),
        };
        validate_trace_backed_patch_result(&result)?;
        return Ok(result);
    }

    let baseline_overrides = BTreeMap::from([(patch.file.clone(), source.to_string())]);
    let timeout_ms = deadline.remaining_timeout_ms("baseline patch trace")?;
    let baseline = symbols::trace_symbol_graph_with_overrides_and_timeout(
        &workspace_root,
        &baseline_overrides,
        &trace_target,
        direction,
        timeout_ms,
    )?;
    let overrides = BTreeMap::from([(patch.file.clone(), patch.updated_source.clone())]);
    let timeout_ms = deadline.remaining_timeout_ms("updated patch trace")?;
    let trace = symbols::trace_symbol_graph_with_overrides_and_timeout(
        &workspace_root,
        &overrides,
        &trace_target,
        direction,
        timeout_ms,
    )?;
    deadline.check("patch trace validation")?;
    let trace_validation = validate_patch_commit_with_trace(&patch, &trace)?;
    let impact = trace_patch_impact_summary(&baseline, &trace);

    let result = TraceBackedPatchResult {
        patch,
        trace_target,
        trace: Some(trace),
        trace_validation: Some(trace_validation),
        impact: Some(impact),
        trace_error: None,
    };
    deadline.check("trace-backed patch result")?;
    validate_trace_backed_patch_result(&result)?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_trace_context_at_position(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
) -> Result<TraceBackedPatchResult> {
    validate_patch_with_trace_context_at_position_with_timeout(
        workspace_root,
        path,
        source,
        position,
        new_code,
        bypass_reason,
        direction,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_trace_context_at_position_with_timeout(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceBackedPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("patch position resolution")?;
    let semantic_target = patching::semantic_target_at_position_with_deadline(
        path,
        source,
        position,
        Some(&deadline),
    )?;
    let timeout_ms = deadline.remaining_timeout_ms("trace-backed patch validation")?;
    validate_patch_with_trace_context_with_timeout(
        workspace_root,
        path,
        source,
        &semantic_target,
        new_code,
        bypass_reason,
        direction,
        timeout_ms,
    )
}
