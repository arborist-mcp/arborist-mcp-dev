use std::path::Path;

use anyhow::Result;

use super::super::VirtualFileSystem;
use crate::language::normalize_absolute_path;
use crate::model::{
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, TraceDirection,
};
use crate::symbols::{
    list_symbols_context_with_overrides_filtered,
    list_symbols_discovery_context_with_overrides_filtered,
    list_symbols_neighborhood_context_with_overrides_filtered,
    list_symbols_with_overrides_filtered,
};

impl VirtualFileSystem {
    pub fn list_symbols(
        &mut self,
        workspace_root: &Path,
        limit: usize,
    ) -> Result<SymbolListResult> {
        self.list_symbols_filtered(workspace_root, limit, None, None)
    }

    pub fn list_symbols_context(
        &mut self,
        workspace_root: &Path,
        limit: usize,
    ) -> Result<SymbolListContextResult> {
        self.list_symbols_context_filtered(workspace_root, limit, None, None)
    }

    pub fn list_symbols_neighborhood_context(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolListNeighborhoodContextResult> {
        self.list_symbols_neighborhood_context_filtered(
            workspace_root,
            limit,
            direction,
            max_depth,
            max_nodes,
            None,
            None,
        )
    }

    pub fn list_symbols_discovery_context(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolListDiscoveryContextResult> {
        self.list_symbols_discovery_context_filtered(
            workspace_root,
            limit,
            direction,
            max_depth,
            max_nodes,
            None,
            None,
        )
    }

    pub fn list_symbols_filtered(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        list_symbols_with_overrides_filtered(
            &workspace_root,
            &overrides,
            limit,
            file_path_contains,
            node_kind,
        )
    }

    pub fn list_symbols_context_filtered(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        list_symbols_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            limit,
            file_path_contains,
            node_kind,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_symbols_neighborhood_context_filtered(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListNeighborhoodContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        list_symbols_neighborhood_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_symbols_discovery_context_filtered(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListDiscoveryContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        list_symbols_discovery_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    }
}
