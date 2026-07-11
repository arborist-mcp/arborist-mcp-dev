use std::path::Path;

use anyhow::Result;

use super::VirtualFileSystem;

use crate::language::normalize_absolute_path;
use crate::model::{
    SymbolContextResult, SymbolListContextResult, SymbolListDiscoveryContextResult,
    SymbolListNeighborhoodContextResult, SymbolListResult, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, SymbolSearchContextResult,
    SymbolSearchDiscoveryContextResult, SymbolSearchNeighborhoodContextResult, SymbolSearchResult,
    TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult,
};
use crate::symbols::{
    list_symbols_context_with_overrides_filtered,
    list_symbols_discovery_context_with_overrides_filtered,
    list_symbols_neighborhood_context_with_overrides_filtered,
    list_symbols_with_overrides_filtered, read_symbol_at_position_with_overrides,
    read_symbol_context_at_position_with_overrides, read_symbol_context_with_overrides,
    read_symbol_discovery_context_at_position_with_overrides,
    read_symbol_discovery_context_with_overrides,
    read_symbol_neighborhood_context_at_position_with_overrides,
    read_symbol_neighborhood_context_with_overrides, read_symbol_with_overrides,
    search_symbols_context_with_overrides_filtered,
    search_symbols_discovery_context_with_overrides_filtered,
    search_symbols_neighborhood_context_with_overrides_filtered,
    search_symbols_with_overrides_filtered, trace_symbol_graph_at_position_with_overrides,
    trace_symbol_graph_with_overrides, trace_symbol_neighborhood_at_position_with_overrides,
    trace_symbol_neighborhood_with_overrides,
};

impl VirtualFileSystem {
    pub fn trace_symbol_graph(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<TraceSymbolGraphResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_graph_with_overrides(&workspace_root, &overrides, symbol_path, direction)
    }

    pub fn trace_symbol_neighborhood(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_neighborhood_with_overrides(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
        )
    }

    pub fn trace_symbol_graph_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
    ) -> Result<TraceSymbolGraphResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_graph_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
        )
    }

    pub fn trace_symbol_neighborhood_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_neighborhood_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
            max_depth,
            max_nodes,
        )
    }

    pub fn read_symbol(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
    ) -> Result<SymbolReadResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_with_overrides(&workspace_root, &overrides, symbol_path)
    }

    pub fn read_symbol_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
    ) -> Result<SymbolReadResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_at_position_with_overrides(&workspace_root, &overrides, file_path, position)
    }

    pub fn read_symbol_context(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_context_with_overrides(&workspace_root, &overrides, symbol_path, direction)
    }

    pub fn read_symbol_context_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_context_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
        )
    }

    pub fn read_symbol_neighborhood_context(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolNeighborhoodContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_neighborhood_context_with_overrides(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
        )
    }

    pub fn read_symbol_neighborhood_context_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolNeighborhoodContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_neighborhood_context_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
            max_depth,
            max_nodes,
        )
    }

    pub fn read_symbol_discovery_context(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_discovery_context_with_overrides(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
        )
    }

    pub fn read_symbol_discovery_context_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_discovery_context_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
            max_depth,
            max_nodes,
        )
    }

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
