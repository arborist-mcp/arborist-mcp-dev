use anyhow::Result;

use super::SymbolQueryContext;
use crate::model::{
    SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
    SymbolSearchNeighborhoodContextResult, SymbolSearchResult, TraceDirection,
};
use crate::symbols;

impl SymbolQueryContext {
    pub fn search_symbols(
        &self,
        query: &str,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::search_symbols_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    query,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::search_symbols_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    query,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    pub fn search_symbols_context(
        &self,
        query: &str,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::search_symbols_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    query,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::search_symbols_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    query,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_symbols_neighborhood_context(
        &self,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchNeighborhoodContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::search_symbols_neighborhood_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    query,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::search_symbols_neighborhood_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    query,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_symbols_discovery_context(
        &self,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchDiscoveryContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::search_symbols_discovery_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    query,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::search_symbols_discovery_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    query,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }
}
