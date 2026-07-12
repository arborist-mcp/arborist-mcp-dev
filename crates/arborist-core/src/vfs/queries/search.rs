use std::path::Path;

use anyhow::Result;

use super::super::VirtualFileSystem;
use crate::language::normalize_absolute_path;
use crate::model::{
    SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
    SymbolSearchNeighborhoodContextResult, SymbolSearchResult, TraceDirection,
};
use crate::symbols::{
    search_symbols_context_with_overrides_filtered,
    search_symbols_discovery_context_with_overrides_filtered,
    search_symbols_neighborhood_context_with_overrides_filtered,
    search_symbols_with_overrides_filtered,
};

impl VirtualFileSystem {
    pub fn search_symbols(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
    ) -> Result<SymbolSearchResult> {
        self.search_symbols_filtered(workspace_root, query, limit, None, None)
    }

    pub fn search_symbols_filtered(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        search_symbols_with_overrides_filtered(
            &workspace_root,
            &overrides,
            query,
            limit,
            file_path_contains,
            node_kind,
        )
    }

    pub fn search_symbols_context(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
    ) -> Result<SymbolSearchContextResult> {
        self.search_symbols_context_filtered(workspace_root, query, limit, None, None)
    }

    pub fn search_symbols_context_filtered(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        search_symbols_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            query,
            limit,
            file_path_contains,
            node_kind,
        )
    }

    pub fn search_symbols_discovery_context(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolSearchDiscoveryContextResult> {
        self.search_symbols_discovery_context_filtered(
            workspace_root,
            query,
            limit,
            direction,
            max_depth,
            max_nodes,
            None,
            None,
        )
    }

    pub fn search_symbols_neighborhood_context(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolSearchNeighborhoodContextResult> {
        self.search_symbols_neighborhood_context_filtered(
            workspace_root,
            query,
            limit,
            direction,
            max_depth,
            max_nodes,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_symbols_neighborhood_context_filtered(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchNeighborhoodContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        search_symbols_neighborhood_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            query,
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_symbols_discovery_context_filtered(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchDiscoveryContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        search_symbols_discovery_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            query,
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    }
}
