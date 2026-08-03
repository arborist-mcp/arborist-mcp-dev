use std::path::Path;

use anyhow::Result;

use super::SymbolQueryContext;
use crate::model::{
    Position, TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult,
};
use crate::symbol_trace::TraceQueryDeadline;
use crate::symbols;

impl SymbolQueryContext {
    pub fn trace_symbol_graph(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<TraceSymbolGraphResult> {
        self.trace_symbol_graph_with_timeout(symbol_path, direction, None)
    }

    pub fn trace_symbol_graph_with_timeout(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        timeout_ms: Option<u64>,
    ) -> Result<TraceSymbolGraphResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.trace_symbol_graph_with_deadline(symbol_path, direction, &deadline)
    }

    pub(crate) fn trace_symbol_graph_with_deadline(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        deadline: &TraceQueryDeadline,
    ) -> Result<TraceSymbolGraphResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::trace_symbol_graph_with_overrides_with_deadline(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::trace_symbol_graph_from_index_with_overrides_with_deadline(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                    deadline,
                )
            },
        )
    }

    pub fn trace_symbol_graph_at_position(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
    ) -> Result<TraceSymbolGraphResult> {
        self.trace_symbol_graph_at_position_with_timeout(file_path, position, direction, None)
    }

    pub fn trace_symbol_graph_at_position_with_timeout(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        timeout_ms: Option<u64>,
    ) -> Result<TraceSymbolGraphResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.trace_symbol_graph_at_position_with_deadline(file_path, position, direction, &deadline)
    }

    pub(crate) fn trace_symbol_graph_at_position_with_deadline(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        deadline: &TraceQueryDeadline,
    ) -> Result<TraceSymbolGraphResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::trace_symbol_graph_at_position_with_overrides_with_deadline(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::trace_symbol_graph_at_position_from_index_with_overrides_with_deadline(
                    db_path, overrides, file_path, position, direction, deadline,
                )
            },
        )
    }

    pub fn trace_symbol_neighborhood(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        self.trace_symbol_neighborhood_with_timeout(
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            None,
        )
    }

    pub fn trace_symbol_neighborhood_with_timeout(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.trace_symbol_neighborhood_with_deadline(
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            &deadline,
        )
    }

    pub(crate) fn trace_symbol_neighborhood_with_deadline(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        deadline: &TraceQueryDeadline,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::trace_symbol_neighborhood_with_overrides_with_deadline(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::trace_symbol_neighborhood_from_index_with_overrides_with_deadline(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                    deadline,
                )
            },
        )
    }

    pub fn trace_symbol_neighborhood_at_position(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        self.trace_symbol_neighborhood_at_position_with_timeout(
            file_path, position, direction, max_depth, max_nodes, None,
        )
    }

    pub fn trace_symbol_neighborhood_at_position_with_timeout(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.trace_symbol_neighborhood_at_position_with_deadline(
            file_path, position, direction, max_depth, max_nodes, &deadline,
        )
    }

    pub(crate) fn trace_symbol_neighborhood_at_position_with_deadline(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        deadline: &TraceQueryDeadline,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::trace_symbol_neighborhood_at_position_with_overrides_with_deadline(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                    max_depth,
                    max_nodes,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::trace_symbol_neighborhood_at_position_from_index_with_overrides_with_deadline(
                    db_path,
                    overrides,
                    file_path,
                    position,
                    direction,
                    max_depth,
                    max_nodes,
                    deadline,
                )
            },
        )
    }
}
