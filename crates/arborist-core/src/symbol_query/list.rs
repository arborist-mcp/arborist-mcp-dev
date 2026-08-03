use anyhow::Result;

use super::SymbolQueryContext;
use crate::model::{
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, TraceDirection,
};
use crate::symbol_trace::TraceQueryDeadline;
use crate::symbols;

impl SymbolQueryContext {
    pub fn list_symbols_context_with_timeout(
        &self,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolListContextResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.list_symbols_context_with_deadline(limit, file_path_contains, node_kind, &deadline)
    }

    pub(crate) fn list_symbols_context_with_deadline(
        &self,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolListContextResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::list_symbols_context_with_overrides_filtered_with_deadline(
                    workspace_root,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::list_symbols_context_from_index_with_overrides_filtered_with_deadline(
                    db_path,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                    deadline,
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_symbols_neighborhood_context_with_timeout(
        &self,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolListNeighborhoodContextResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.list_symbols_neighborhood_context_with_deadline(
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
            &deadline,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn list_symbols_neighborhood_context_with_deadline(
        &self,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolListNeighborhoodContextResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::list_symbols_neighborhood_context_with_overrides_filtered_with_deadline(
                    workspace_root,
                    overrides,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::list_symbols_neighborhood_context_from_index_with_overrides_filtered_with_deadline(
                    db_path,
                    overrides,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                    deadline,
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_symbols_discovery_context_with_timeout(
        &self,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolListDiscoveryContextResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.list_symbols_discovery_context_with_deadline(
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
            &deadline,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn list_symbols_discovery_context_with_deadline(
        &self,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolListDiscoveryContextResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::list_symbols_discovery_context_with_overrides_filtered_with_deadline(
                    workspace_root,
                    overrides,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::list_symbols_discovery_context_from_index_with_overrides_filtered_with_deadline(
                    db_path,
                    overrides,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                    deadline,
                )
            },
        )
    }

    pub fn list_symbols_with_timeout(
        &self,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolListResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.list_symbols_with_deadline(limit, file_path_contains, node_kind, &deadline)
    }

    pub(crate) fn list_symbols_with_deadline(
        &self,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolListResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::list_symbols_with_overrides_filtered_with_deadline(
                    workspace_root,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::list_symbols_from_index_with_overrides_filtered_with_deadline(
                    db_path,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                    deadline,
                )
            },
        )
    }

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
