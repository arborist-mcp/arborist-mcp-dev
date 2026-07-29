use std::path::Path;

use anyhow::Result;

use super::VirtualFileSystem;
use super::state::normalized_virtual_path;
use crate::language::{ensure_path_inside_workspace, normalize_absolute_path};
use crate::model::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    TraceBackedPatchResult, TraceDirection,
};
use crate::symbol_trace::TraceQueryDeadline;
mod apply;
mod results;

#[cfg(test)]
pub(super) use apply::VirtualPatchTarget;

impl VirtualFileSystem {
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
