use std::path::Path;

use anyhow::{Context, Result, anyhow};

use super::VirtualFileSystem;
use super::state::normalized_virtual_path;
use crate::language::{ensure_path_inside_workspace, normalize_absolute_path};
use crate::model::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    PatchAstNodeResult, TraceBackedPatchResult, TraceDirection,
};
use crate::patching::{
    build_patch_result, prepare_patch_replacement, semantic_target_at_position,
    validate_bypass_reason, validate_patch_replacement,
};
use crate::symbol_trace::TraceQueryDeadline;
mod results;

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

        let prepared = {
            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            prepare_patch_replacement(&entry.path, &entry.source, semantic_target, new_code)?
        };

        let previous = self
            .entries
            .get(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
            .clone();

        self.apply_edit(
            &path,
            prepared.start_byte,
            prepared.end_byte,
            &prepared.replacement,
        )?;

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
                prepared.start_byte,
                prepared.replacement.len(),
                prepared.validation_issues,
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
        self.validate_patch_with_trace_context_with_timeout(
            workspace_root,
            path,
            semantic_target,
            new_code,
            bypass_reason,
            direction,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_trace_context_with_timeout(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        timeout_ms: Option<u64>,
    ) -> Result<TraceBackedPatchResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        deadline.check("virtual patch setup")?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        deadline.check("virtual patch validation")?;
        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        let timeout_ms = deadline.remaining_timeout_ms("virtual patch trace")?;
        self.trace_backed_patch_result_with_timeout(&workspace_root, &patch, direction, timeout_ms)
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
        self.validate_patch_with_trace_context_at_position_with_timeout(
            workspace_root,
            path,
            position,
            new_code,
            bypass_reason,
            direction,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_trace_context_at_position_with_timeout(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        timeout_ms: Option<u64>,
    ) -> Result<TraceBackedPatchResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        deadline.check("virtual position patch setup")?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        deadline.check("virtual position patch validation")?;
        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        let timeout_ms = deadline.remaining_timeout_ms("virtual position patch trace")?;
        self.trace_backed_patch_result_with_timeout(&workspace_root, &patch, direction, timeout_ms)
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
        self.validate_patch_with_graph_context_with_timeout(
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
    pub fn validate_patch_with_graph_context_with_timeout(
        &mut self,
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
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        deadline.check("virtual graph patch setup")?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        deadline.check("virtual graph patch validation")?;
        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        let timeout_ms = deadline.remaining_timeout_ms("virtual graph patch trace")?;
        self.graph_backed_patch_result_with_timeout(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
            timeout_ms,
        )
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
        self.validate_patch_with_graph_context_at_position_with_timeout(
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
    pub fn validate_patch_with_graph_context_at_position_with_timeout(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<GraphBackedPatchResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        deadline.check("virtual position graph patch setup")?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        deadline.check("virtual position graph patch validation")?;
        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        let timeout_ms = deadline.remaining_timeout_ms("virtual position graph patch trace")?;
        self.graph_backed_patch_result_with_timeout(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
            timeout_ms,
        )
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
        self.validate_patch_with_neighborhood_context_with_timeout(
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
    pub fn validate_patch_with_neighborhood_context_with_timeout(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<NeighborhoodContextPatchResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        deadline.check("virtual neighborhood patch setup")?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        deadline.check("virtual neighborhood patch validation")?;
        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        let timeout_ms = deadline.remaining_timeout_ms("virtual neighborhood patch context")?;
        self.neighborhood_context_patch_result_with_timeout(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
            timeout_ms,
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
        self.validate_patch_with_neighborhood_context_at_position_with_timeout(
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
    pub fn validate_patch_with_neighborhood_context_at_position_with_timeout(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<NeighborhoodContextPatchResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        deadline.check("virtual position neighborhood patch setup")?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        deadline.check("virtual position neighborhood patch validation")?;
        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        let timeout_ms =
            deadline.remaining_timeout_ms("virtual position neighborhood patch context")?;
        self.neighborhood_context_patch_result_with_timeout(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
            timeout_ms,
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
        self.validate_patch_with_discovery_context_with_timeout(
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
    pub fn validate_patch_with_discovery_context_with_timeout(
        &mut self,
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
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        deadline.check("virtual discovery patch setup")?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        deadline.check("virtual discovery patch validation")?;
        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        let timeout_ms = deadline.remaining_timeout_ms("virtual discovery patch context")?;
        self.discovery_context_patch_result_with_timeout(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
            timeout_ms,
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
        self.validate_patch_with_discovery_context_at_position_with_timeout(
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
    pub fn validate_patch_with_discovery_context_at_position_with_timeout(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<DiscoveryContextPatchResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        deadline.check("virtual position discovery patch setup")?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        deadline.check("virtual position discovery patch validation")?;
        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        let timeout_ms =
            deadline.remaining_timeout_ms("virtual position discovery patch context")?;
        self.discovery_context_patch_result_with_timeout(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
            timeout_ms,
        )
    }
}
