use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::language::{self, ensure_path_inside_workspace, read_source};
use crate::model::*;
use crate::symbol_trace::TraceQueryDeadline;
use crate::{patching, symbols};

use super::super::{
    validate_discovery_context_patch_result, validate_patch_commit_with_trace,
    validate_patch_with_discovery_context_at_position_with_timeout,
    validate_patch_with_discovery_context_with_timeout,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_discovery_context_from_path(
    workspace_root: &Path,
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<DiscoveryContextPatchResult> {
    validate_patch_with_discovery_context_from_path_with_timeout(
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
pub fn validate_patch_with_discovery_context_from_path_with_timeout(
    workspace_root: &Path,
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<DiscoveryContextPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let workspace_root = language::normalize_absolute_path(workspace_root)?;
    let path = language::normalize_absolute_path(path)?;
    ensure_path_inside_workspace(&workspace_root, &path)?;
    deadline.check("patch source read")?;
    let source = read_source(&path)?;
    let timeout_ms = deadline.remaining_timeout_ms("discovery-context patch validation")?;
    validate_patch_with_discovery_context_with_timeout(
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
pub fn validate_patch_with_discovery_context_from_index(
    db_path: &Path,
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<DiscoveryContextPatchResult> {
    validate_patch_with_discovery_context_from_index_with_timeout(
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
pub fn validate_patch_with_discovery_context_from_index_with_timeout(
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
) -> Result<DiscoveryContextPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let path = language::normalize_absolute_path(path)?;
    deadline.check("indexed patch validation")?;
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
        deadline.check("indexed patch validation result")?;
        let result = DiscoveryContextPatchResult {
            patch,
            trace_target,
            trace: None,
            read: None,
            neighborhood_context: None,
            trace_validation: None,
            trace_error: Some(
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors().to_string(),
            ),
        };
        validate_discovery_context_patch_result(&result)?;
        return Ok(result);
    }

    if !patch.applied {
        deadline.check("indexed patch validation result")?;
        let result = DiscoveryContextPatchResult {
            patch,
            trace_target,
            trace: None,
            read: None,
            neighborhood_context: None,
            trace_validation: None,
            trace_error: Some(
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection().to_string(),
            ),
        };
        validate_discovery_context_patch_result(&result)?;
        return Ok(result);
    }

    deadline.check("indexed patch discovery overrides")?;
    let overrides = BTreeMap::from([(patch.file.clone(), patch.updated_source.clone())]);
    let timeout_ms = deadline.remaining_timeout_ms("indexed patch discovery context")?;
    let discovery = symbols::read_symbol_discovery_context_from_index_with_overrides_with_timeout(
        db_path,
        &overrides,
        &trace_target,
        direction,
        max_depth,
        max_nodes,
        timeout_ms,
    )?;
    deadline.check("indexed patch discovery trace validation")?;
    let trace_validation = validate_patch_commit_with_trace(&patch, &discovery.trace)?;

    let result = DiscoveryContextPatchResult {
        patch,
        trace_target,
        trace: Some(discovery.trace),
        read: Some(discovery.read),
        neighborhood_context: Some(discovery.neighborhood_context),
        trace_validation: Some(trace_validation),
        trace_error: None,
    };
    deadline.check("indexed discovery-context patch result")?;
    validate_discovery_context_patch_result(&result)?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn validate_patch_with_discovery_context_from_index_path_with_timeout(
    db_path: &Path,
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<DiscoveryContextPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let path = language::normalize_absolute_path(path)?;
    deadline.check("indexed patch source read")?;
    let source = read_source(&path)?;
    let timeout_ms = deadline.remaining_timeout_ms("indexed discovery-context patch validation")?;
    validate_patch_with_discovery_context_from_index_with_timeout(
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
pub fn validate_patch_with_discovery_context_at_position_from_path(
    workspace_root: &Path,
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<DiscoveryContextPatchResult> {
    validate_patch_with_discovery_context_at_position_from_path_with_timeout(
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
pub fn validate_patch_with_discovery_context_at_position_from_path_with_timeout(
    workspace_root: &Path,
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<DiscoveryContextPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let workspace_root = language::normalize_absolute_path(workspace_root)?;
    let path = language::normalize_absolute_path(path)?;
    ensure_path_inside_workspace(&workspace_root, &path)?;
    deadline.check("patch source read")?;
    let source = read_source(&path)?;
    let timeout_ms =
        deadline.remaining_timeout_ms("position discovery-context patch validation")?;
    validate_patch_with_discovery_context_at_position_with_timeout(
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
pub fn validate_patch_with_discovery_context_at_position_from_index(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<DiscoveryContextPatchResult> {
    validate_patch_with_discovery_context_at_position_from_index_with_timeout(
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
pub fn validate_patch_with_discovery_context_at_position_from_index_with_timeout(
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
) -> Result<DiscoveryContextPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("indexed patch position resolution")?;
    let semantic_target = patching::semantic_target_at_position_with_deadline(
        path,
        source,
        position,
        Some(&deadline),
    )?;
    let timeout_ms = deadline.remaining_timeout_ms("indexed discovery-context patch validation")?;
    validate_patch_with_discovery_context_from_index_with_timeout(
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
pub fn validate_patch_with_discovery_context_at_position_from_index_path_with_timeout(
    db_path: &Path,
    path: &Path,
    position: &Position,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<DiscoveryContextPatchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let path = language::normalize_absolute_path(path)?;
    deadline.check("indexed patch source read")?;
    let source = read_source(&path)?;
    let timeout_ms = deadline.remaining_timeout_ms("indexed position patch validation")?;
    validate_patch_with_discovery_context_at_position_from_index_with_timeout(
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
