use std::path::Path;

use anyhow::Result;

use super::super::VirtualFileSystem;
use crate::model::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    PatchAstNodeResult, TraceBackedPatchResult, TraceDirection,
};
use crate::symbol_trace::TraceQueryDeadline;
use crate::symbols::{
    read_symbol_discovery_context_with_overrides_with_deadline,
    read_symbol_neighborhood_context_with_overrides_with_deadline,
    trace_symbol_graph_with_overrides_with_deadline,
    trace_symbol_neighborhood_with_overrides_with_deadline,
};
use crate::{
    validate_discovery_context_patch_result, validate_graph_backed_patch_result,
    validate_neighborhood_context_patch_result, validate_patch_commit_with_trace,
    validate_trace_backed_patch_result,
};

impl VirtualFileSystem {
    pub(in crate::vfs) fn trace_backed_patch_result_with_deadline(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        deadline: &TraceQueryDeadline,
    ) -> Result<TraceBackedPatchResult> {
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
        let mut overrides =
            self.virtual_overrides_for_workspace_with_deadline(workspace_root, deadline)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let trace = trace_symbol_graph_with_overrides_with_deadline(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            deadline,
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
    pub(in crate::vfs) fn graph_backed_patch_result_with_deadline(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        deadline: &TraceQueryDeadline,
    ) -> Result<GraphBackedPatchResult> {
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
        let mut overrides =
            self.virtual_overrides_for_workspace_with_deadline(workspace_root, deadline)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let trace = trace_symbol_graph_with_overrides_with_deadline(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            deadline,
        )?;
        let neighborhood = trace_symbol_neighborhood_with_overrides_with_deadline(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            max_depth,
            max_nodes,
            deadline,
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
    pub(in crate::vfs) fn neighborhood_context_patch_result_with_deadline(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        deadline: &TraceQueryDeadline,
    ) -> Result<NeighborhoodContextPatchResult> {
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
            deadline.check("virtual neighborhood patch validation result")?;
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
            deadline.check("virtual neighborhood patch validation result")?;
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

        deadline.check("virtual neighborhood patch overrides")?;
        let mut overrides =
            self.virtual_overrides_for_workspace_with_deadline(workspace_root, deadline)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let trace = trace_symbol_graph_with_overrides_with_deadline(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            deadline,
        )?;
        let neighborhood_context = read_symbol_neighborhood_context_with_overrides_with_deadline(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            max_depth,
            max_nodes,
            deadline,
        )?;
        deadline.check("virtual neighborhood patch trace validation")?;
        let trace_validation = validate_patch_commit_with_trace(patch, &trace)?;
        let result = NeighborhoodContextPatchResult {
            patch: patch.clone(),
            trace_target,
            trace: Some(trace),
            neighborhood_context: Some(neighborhood_context),
            trace_validation: Some(trace_validation),
            trace_error: None,
        };
        deadline.check("virtual neighborhood-context patch result")?;
        validate_neighborhood_context_patch_result(&result)?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::vfs) fn discovery_context_patch_result_with_deadline(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        deadline: &TraceQueryDeadline,
    ) -> Result<DiscoveryContextPatchResult> {
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
            deadline.check("virtual discovery patch validation result")?;
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
            deadline.check("virtual discovery patch validation result")?;
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

        deadline.check("virtual discovery patch overrides")?;
        let mut overrides =
            self.virtual_overrides_for_workspace_with_deadline(workspace_root, deadline)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let discovery = read_symbol_discovery_context_with_overrides_with_deadline(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            max_depth,
            max_nodes,
            deadline,
        )?;
        deadline.check("virtual discovery patch trace validation")?;
        let trace_validation = validate_patch_commit_with_trace(patch, &discovery.trace)?;
        let result = DiscoveryContextPatchResult {
            patch: patch.clone(),
            trace_target,
            trace: Some(discovery.trace),
            read: Some(discovery.read),
            neighborhood_context: Some(discovery.neighborhood_context),
            trace_validation: Some(trace_validation),
            trace_error: None,
        };
        deadline.check("virtual discovery-context patch result")?;
        validate_discovery_context_patch_result(&result)?;
        Ok(result)
    }
}
