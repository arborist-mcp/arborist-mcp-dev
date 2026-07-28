use std::borrow::Cow;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use super::VirtualFileSystem;
use super::state::normalized_virtual_path;
use crate::deadline::DeadlineCheck;
use crate::language::{
    ensure_path_inside_workspace, normalize_absolute_path, validate_source_length,
};
use crate::model::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    PatchAstNodeResult, Position, TraceBackedPatchResult, TraceDirection,
};
use crate::patching::{
    PatchBuildInput, build_patch_result_with_deadline, patch_deadline,
    prepare_patch_replacement_with_deadline, semantic_target_at_position_with_deadline,
    splice_source, validate_bypass_reason, validate_patch_replacement,
};
use crate::symbol_trace::TraceQueryDeadline;
mod results;

pub(super) enum VirtualPatchTarget<'a> {
    Semantic(&'a str),
    Position(&'a Position),
}

impl VirtualFileSystem {
    pub fn patch_node(
        &mut self,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
    ) -> Result<PatchAstNodeResult> {
        self.patch_node_with_timeout(path, semantic_target, new_code, bypass_reason, None)
    }

    pub fn patch_node_with_timeout(
        &mut self,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<PatchAstNodeResult> {
        let deadline = patch_deadline(timeout_ms)?;
        self.patch_node_with_deadline(
            path,
            VirtualPatchTarget::Semantic(semantic_target),
            new_code,
            bypass_reason,
            false,
            &deadline,
        )
    }

    pub fn patch_node_and_commit(
        &mut self,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
    ) -> Result<PatchAstNodeResult> {
        self.patch_node_and_commit_with_timeout(
            path,
            semantic_target,
            new_code,
            bypass_reason,
            None,
        )
    }

    pub fn patch_node_and_commit_with_timeout(
        &mut self,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<PatchAstNodeResult> {
        let deadline = patch_deadline(timeout_ms)?;
        self.patch_node_with_deadline(
            path,
            VirtualPatchTarget::Semantic(semantic_target),
            new_code,
            bypass_reason,
            true,
            &deadline,
        )
    }

    pub fn patch_node_at_position(
        &mut self,
        path: &Path,
        position: &Position,
        new_code: &str,
        bypass_reason: Option<&str>,
    ) -> Result<PatchAstNodeResult> {
        self.patch_node_at_position_with_timeout(path, position, new_code, bypass_reason, None)
    }

    pub fn patch_node_at_position_with_timeout(
        &mut self,
        path: &Path,
        position: &Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<PatchAstNodeResult> {
        let deadline = patch_deadline(timeout_ms)?;
        self.patch_node_with_deadline(
            path,
            VirtualPatchTarget::Position(position),
            new_code,
            bypass_reason,
            false,
            &deadline,
        )
    }

    pub fn patch_node_at_position_and_commit(
        &mut self,
        path: &Path,
        position: &Position,
        new_code: &str,
        bypass_reason: Option<&str>,
    ) -> Result<PatchAstNodeResult> {
        self.patch_node_at_position_and_commit_with_timeout(
            path,
            position,
            new_code,
            bypass_reason,
            None,
        )
    }

    pub fn patch_node_at_position_and_commit_with_timeout(
        &mut self,
        path: &Path,
        position: &Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<PatchAstNodeResult> {
        let deadline = patch_deadline(timeout_ms)?;
        self.patch_node_with_deadline(
            path,
            VirtualPatchTarget::Position(position),
            new_code,
            bypass_reason,
            true,
            &deadline,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn patch_node_with_deadline(
        &mut self,
        path: &Path,
        target: VirtualPatchTarget<'_>,
        new_code: &str,
        bypass_reason: Option<&str>,
        commit: bool,
        deadline: &dyn DeadlineCheck,
    ) -> Result<PatchAstNodeResult> {
        deadline.check("patch input validation")?;
        validate_patch_replacement(new_code)?;
        validate_bypass_reason(bypass_reason)?;

        deadline.check("virtual path validation")?;
        let (path, normalized) = normalized_virtual_path(path)?;
        deadline.check("virtual source load")?;
        self.ensure_loaded(&path, None)?;
        deadline.check("virtual source refresh")?;
        self.refresh_if_clean(&normalized)?;

        let previous = self
            .entries
            .get(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
            .clone();

        deadline.check("patch target resolution")?;
        let semantic_target = {
            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            match target {
                VirtualPatchTarget::Semantic(semantic_target) => Cow::Borrowed(semantic_target),
                VirtualPatchTarget::Position(position) => {
                    Cow::Owned(semantic_target_at_position_with_deadline(
                        &entry.path,
                        &entry.source,
                        position,
                        Some(deadline),
                    )?)
                }
            }
        };

        let prepared = {
            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            prepare_patch_replacement_with_deadline(
                &entry.path,
                &entry.source,
                &semantic_target,
                new_code,
                Some(deadline),
            )?
        };
        let start_byte = prepared.start_byte;
        let end_byte = prepared.end_byte;
        let replacement = prepared.replacement;
        let replacement_len = replacement.len();
        let preflight_issues = prepared.validation_issues;

        let updated_source = {
            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            let result_len = entry
                .source
                .len()
                .checked_sub(end_byte - start_byte)
                .and_then(|length| length.checked_add(replacement_len))
                .ok_or_else(|| anyhow!("updated source size overflowed"))?;
            validate_source_length(&entry.path, result_len)?;
            deadline.check("source replacement")?;
            splice_source(&entry.source, start_byte..end_byte, &replacement)
        };

        let result = {
            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            build_patch_result_with_deadline(
                PatchBuildInput {
                    path: &entry.path,
                    semantic_target: &semantic_target,
                    updated_source,
                    bypass_reason,
                    patch_start: start_byte,
                    replacement_len,
                    preflight_issues,
                },
                Some(deadline),
            )
            .context("failed to validate virtual patch")?
        };

        if !result.applied {
            return Ok(result);
        }

        deadline.check("virtual source edit")?;
        if let Err(error) =
            self.apply_loaded_edit(&path, &normalized, start_byte, end_byte, &replacement)
        {
            self.entries.insert(normalized, previous);
            return Err(error).context("failed to apply virtual patch");
        }
        if let Err(error) = deadline.check("virtual source edit result") {
            self.entries.insert(normalized, previous);
            return Err(error);
        }

        if !commit {
            return Ok(result);
        }

        if let Err(error) = deadline.check("source write") {
            self.entries.insert(normalized, previous);
            return Err(error);
        }

        if let Err(error) = self.commit_loaded_file(&normalized, false) {
            if !self.virtual_patch_source_persisted(&normalized, &result.updated_source) {
                self.entries.insert(normalized, previous);
            }
            return Err(error).context("failed to commit virtual patch");
        }

        Ok(result)
    }

    fn virtual_patch_source_persisted(&self, normalized: &str, expected_source: &str) -> bool {
        self.entries.get(normalized).is_some_and(|entry| {
            !entry.dirty && entry.source == expected_source && entry.disk_source == expected_source
        })
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
