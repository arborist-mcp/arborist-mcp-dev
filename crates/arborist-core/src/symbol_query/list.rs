use anyhow::Result;

use super::SymbolQueryContext;
use crate::model::{
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, TraceDirection,
};
use crate::symbols;

impl SymbolQueryContext {
    pub fn list_symbols(
        &self,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::list_symbols_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::list_symbols_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    pub fn list_symbols_context(
        &self,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::list_symbols_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::list_symbols_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_symbols_neighborhood_context(
        &self,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListNeighborhoodContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::list_symbols_neighborhood_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::list_symbols_neighborhood_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
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
    pub fn list_symbols_discovery_context(
        &self,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListDiscoveryContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::list_symbols_discovery_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::list_symbols_discovery_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
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
