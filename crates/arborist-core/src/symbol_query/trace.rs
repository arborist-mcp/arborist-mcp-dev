use std::path::Path;

use anyhow::Result;

use super::SymbolQueryContext;
use crate::model::{
    Position, TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult,
};
use crate::symbols;

impl SymbolQueryContext {
    pub fn trace_symbol_graph(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<TraceSymbolGraphResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::trace_symbol_graph_with_overrides(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                )
            },
            |db_path, overrides| {
                symbols::trace_symbol_graph_from_index_with_overrides(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
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
        self.dispatch(
            |workspace_root, overrides| {
                symbols::trace_symbol_graph_at_position_with_overrides(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                )
            },
            |db_path, overrides| {
                symbols::trace_symbol_graph_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position, direction,
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
        self.dispatch(
            |workspace_root, overrides| {
                symbols::trace_symbol_neighborhood_with_overrides(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
            |db_path, overrides| {
                symbols::trace_symbol_neighborhood_from_index_with_overrides(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
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
        self.dispatch(
            |workspace_root, overrides| {
                symbols::trace_symbol_neighborhood_at_position_with_overrides(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
            |db_path, overrides| {
                symbols::trace_symbol_neighborhood_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position, direction, max_depth, max_nodes,
                )
            },
        )
    }
}
