use std::path::Path;

use anyhow::Result;

use super::VirtualFileSystem;
use crate::model::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    PatchAstNodeResult, TraceBackedPatchResult, TraceDirection,
};
use crate::symbol_trace::TraceQueryDeadline;
use crate::symbols::{
    read_symbol_neighborhood_context_with_overrides, read_symbol_with_overrides,
    trace_symbol_graph_with_overrides, trace_symbol_graph_with_overrides_and_timeout,
    trace_symbol_neighborhood_with_overrides_and_timeout,
};
use crate::{
    validate_discovery_context_patch_result, validate_graph_backed_patch_result,
    validate_neighborhood_context_patch_result, validate_patch_commit_with_trace,
    validate_trace_backed_patch_result,
};

impl VirtualFileSystem {
    pub(super) fn trace_backed_patch_result_with_timeout(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        timeout_ms: Option<u64>,
    ) -> Result<TraceBackedPatchResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
            deadline.check("virtual patch validation result")?;
            let result = TraceBackedPatchResult {
                patch: patch.clone(),
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
            deadline.check("virtual patch validation result")?;
            let result = TraceBackedPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                trace_validation: None,
                impact: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
                        .to_string(),
                ),
            };
            validate_trace_backed_patch_result(&result)?;
            return Ok(result);
        }

        deadline.check("virtual patch overrides")?;
        let mut overrides = self.virtual_overrides_for_workspace(workspace_root)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let timeout_ms = deadline.remaining_timeout_ms("virtual patch trace")?;
        let trace = trace_symbol_graph_with_overrides_and_timeout(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            timeout_ms,
        )?;
        deadline.check("virtual patch trace validation")?;
        let trace_validation = validate_patch_commit_with_trace(patch, &trace)?;
        let result = TraceBackedPatchResult {
            patch: patch.clone(),
            trace_target,
            trace: Some(trace),
            trace_validation: Some(trace_validation),
            impact: None,
            trace_error: None,
        };
        deadline.check("virtual trace-backed patch result")?;
        validate_trace_backed_patch_result(&result)?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn graph_backed_patch_result_with_timeout(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<GraphBackedPatchResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
            deadline.check("virtual graph patch validation result")?;
            let result = GraphBackedPatchResult {
                patch: patch.clone(),
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
            deadline.check("virtual graph patch validation result")?;
            let result = GraphBackedPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                neighborhood: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
                        .to_string(),
                ),
            };
            validate_graph_backed_patch_result(&result)?;
            return Ok(result);
        }

        deadline.check("virtual graph patch overrides")?;
        let mut overrides = self.virtual_overrides_for_workspace(workspace_root)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let timeout_ms = deadline.remaining_timeout_ms("virtual graph patch trace")?;
        let trace = trace_symbol_graph_with_overrides_and_timeout(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            timeout_ms,
        )?;
        let timeout_ms = deadline.remaining_timeout_ms("virtual graph patch neighborhood")?;
        let neighborhood = trace_symbol_neighborhood_with_overrides_and_timeout(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            max_depth,
            max_nodes,
            timeout_ms,
        )?;
        deadline.check("virtual graph patch trace validation")?;
        let trace_validation = validate_patch_commit_with_trace(patch, &trace)?;
        let result = GraphBackedPatchResult {
            patch: patch.clone(),
            trace_target,
            trace: Some(trace),
            neighborhood: Some(neighborhood),
            trace_validation: Some(trace_validation),
            trace_error: None,
        };
        deadline.check("virtual graph-backed patch result")?;
        validate_graph_backed_patch_result(&result)?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn neighborhood_context_patch_result(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<NeighborhoodContextPatchResult> {
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
            let result = NeighborhoodContextPatchResult {
                patch: patch.clone(),
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
            let result = NeighborhoodContextPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                neighborhood_context: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
                        .to_string(),
                ),
            };
            validate_neighborhood_context_patch_result(&result)?;
            return Ok(result);
        }

        let mut overrides = self.virtual_overrides_for_workspace(workspace_root)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let trace = trace_symbol_graph_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
        )?;
        let neighborhood_context = read_symbol_neighborhood_context_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            max_depth,
            max_nodes,
        )?;
        let trace_validation = validate_patch_commit_with_trace(patch, &trace)?;
        let result = NeighborhoodContextPatchResult {
            patch: patch.clone(),
            trace_target,
            trace: Some(trace),
            neighborhood_context: Some(neighborhood_context),
            trace_validation: Some(trace_validation),
            trace_error: None,
        };
        validate_neighborhood_context_patch_result(&result)?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn discovery_context_patch_result(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<DiscoveryContextPatchResult> {
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
            let result = DiscoveryContextPatchResult {
                patch: patch.clone(),
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
            let result = DiscoveryContextPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                read: None,
                neighborhood_context: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
                        .to_string(),
                ),
            };
            validate_discovery_context_patch_result(&result)?;
            return Ok(result);
        }

        let mut overrides = self.virtual_overrides_for_workspace(workspace_root)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let trace = trace_symbol_graph_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
        )?;
        let read = read_symbol_with_overrides(workspace_root, &overrides, &trace_target)?;
        let neighborhood_context = read_symbol_neighborhood_context_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            max_depth,
            max_nodes,
        )?;
        let trace_validation = validate_patch_commit_with_trace(patch, &trace)?;
        let result = DiscoveryContextPatchResult {
            patch: patch.clone(),
            trace_target,
            trace: Some(trace),
            read: Some(read),
            neighborhood_context: Some(neighborhood_context),
            trace_validation: Some(trace_validation),
            trace_error: None,
        };
        validate_discovery_context_patch_result(&result)?;
        Ok(result)
    }
}
