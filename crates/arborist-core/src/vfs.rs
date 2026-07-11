use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

mod buffer;
mod queries;
mod state;

use crate::language::{ensure_path_inside_workspace, normalize_absolute_path};
use crate::model::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    PatchAstNodeResult, TraceBackedPatchResult, TraceDirection,
};
use crate::patching::{
    build_patch_result, semantic_target_at_position, semantic_target_range, validate_bypass_reason,
    validate_patch_replacement,
};
use crate::symbols::{
    read_symbol_neighborhood_context_with_overrides, read_symbol_with_overrides,
    trace_symbol_graph_with_overrides, trace_symbol_neighborhood_with_overrides,
};
use crate::{
    validate_discovery_context_patch_result, validate_graph_backed_patch_result,
    validate_neighborhood_context_patch_result, validate_patch_commit_with_trace,
    validate_trace_backed_patch_result,
};

use self::state::{VirtualFileEntry, normalized_virtual_path};

#[derive(Default)]
pub struct VirtualFileSystem {
    entries: HashMap<String, VirtualFileEntry>,
    symbol_indexes: HashMap<String, PathBuf>,
}

impl VirtualFileSystem {
    pub fn patch_node(
        &mut self,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
    ) -> Result<PatchAstNodeResult> {
        validate_patch_replacement(new_code)?;
        validate_bypass_reason(bypass_reason)?;

        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let (start_byte, end_byte) = {
            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            semantic_target_range(&entry.path, &entry.source, semantic_target)?
        };

        let previous = self
            .entries
            .get(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
            .clone();

        self.apply_edit(&path, start_byte, end_byte, new_code)?;

        let validation_result = {
            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            build_patch_result(
                &entry.path,
                semantic_target,
                entry.source.clone(),
                bypass_reason,
                start_byte,
                new_code.len(),
            )
        };

        let result = match validation_result {
            Ok(result) => result,
            Err(error) => {
                self.entries.insert(normalized, previous);
                return Err(error).context("failed to validate virtual patch");
            }
        };

        if !result.applied {
            self.entries.insert(normalized, previous);
        }

        Ok(result)
    }

    pub fn patch_node_at_position(
        &mut self,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
    ) -> Result<PatchAstNodeResult> {
        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let semantic_target = {
            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            semantic_target_at_position(&entry.path, &entry.source, position)?
        };

        self.patch_node(&path, &semantic_target, new_code, bypass_reason)
    }

    pub fn validate_patch_with_trace_context(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
    ) -> Result<TraceBackedPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        self.trace_backed_patch_result(&workspace_root, &patch, direction)
    }

    pub fn validate_patch_with_trace_context_at_position(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
    ) -> Result<TraceBackedPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        self.trace_backed_patch_result(&workspace_root, &patch, direction)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_graph_context(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<GraphBackedPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        self.graph_backed_patch_result(&workspace_root, &patch, direction, max_depth, max_nodes)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_graph_context_at_position(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<GraphBackedPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        self.graph_backed_patch_result(&workspace_root, &patch, direction, max_depth, max_nodes)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_neighborhood_context(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<NeighborhoodContextPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        self.neighborhood_context_patch_result(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_neighborhood_context_at_position(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<NeighborhoodContextPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        self.neighborhood_context_patch_result(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_discovery_context(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<DiscoveryContextPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        self.discovery_context_patch_result(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_discovery_context_at_position(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<DiscoveryContextPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        self.discovery_context_patch_result(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
        )
    }

    fn trace_backed_patch_result(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
    ) -> Result<TraceBackedPatchResult> {
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
            let result = TraceBackedPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_syntax_errors().to_string(),
                ),
            };
            validate_trace_backed_patch_result(&result)?;
            return Ok(result);
        }

        if !patch.applied {
            let result = TraceBackedPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
                        .to_string(),
                ),
            };
            validate_trace_backed_patch_result(&result)?;
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
        let trace_validation = validate_patch_commit_with_trace(patch, &trace)?;
        let result = TraceBackedPatchResult {
            patch: patch.clone(),
            trace_target,
            trace: Some(trace),
            trace_validation: Some(trace_validation),
            trace_error: None,
        };
        validate_trace_backed_patch_result(&result)?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn graph_backed_patch_result(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<GraphBackedPatchResult> {
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
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

        let mut overrides = self.virtual_overrides_for_workspace(workspace_root)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let trace = trace_symbol_graph_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
        )?;
        let neighborhood = trace_symbol_neighborhood_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            max_depth,
            max_nodes,
        )?;
        let trace_validation = validate_patch_commit_with_trace(patch, &trace)?;
        let result = GraphBackedPatchResult {
            patch: patch.clone(),
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
    fn neighborhood_context_patch_result(
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
    fn discovery_context_patch_result(
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

#[cfg(test)]
mod tests;
