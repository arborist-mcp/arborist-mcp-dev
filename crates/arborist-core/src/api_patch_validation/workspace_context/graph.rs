use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::language::{self, ensure_path_inside_workspace};
use crate::model::*;
use crate::patching::patch_ast_node;
use crate::{patching, symbols};

use super::super::{validate_graph_backed_patch_result, validate_patch_commit_with_trace};

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<GraphBackedPatchResult> {
    let workspace_root = language::normalize_absolute_path(workspace_root)?;
    let path = language::normalize_absolute_path(path)?;
    ensure_path_inside_workspace(&workspace_root, &path)?;

    let patch = patch_ast_node(&path, source, semantic_target, new_code, bypass_reason)?;
    let trace_target = patch.resolved_symbol_id.clone();

    if !patch.validation.syntax_errors.is_empty() {
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

    let mut overrides = BTreeMap::new();
    overrides.insert(patch.file.clone(), patch.updated_source.clone());
    let trace = symbols::trace_symbol_graph_with_overrides(
        &workspace_root,
        &overrides,
        &trace_target,
        direction,
    )?;
    let neighborhood = symbols::trace_symbol_neighborhood_with_overrides(
        &workspace_root,
        &overrides,
        &trace_target,
        direction,
        max_depth,
        max_nodes,
    )?;
    let trace_validation = validate_patch_commit_with_trace(&patch, &trace)?;

    let result = GraphBackedPatchResult {
        patch,
        trace_target,
        trace: Some(trace),
        neighborhood: Some(neighborhood),
        trace_validation: Some(trace_validation),
        trace_error: None,
    };
    validate_graph_backed_patch_result(&result)?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_graph_context_at_position(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<GraphBackedPatchResult> {
    let semantic_target = patching::semantic_target_at_position(path, source, position)?;
    validate_patch_with_graph_context(
        workspace_root,
        path,
        source,
        &semantic_target,
        new_code,
        bypass_reason,
        direction,
        max_depth,
        max_nodes,
    )
}
