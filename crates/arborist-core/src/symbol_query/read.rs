use std::path::Path;

use anyhow::Result;

use super::SymbolQueryContext;
use crate::model::{
    Position, SymbolContextResult, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, TraceDirection,
};
use crate::symbols;

impl SymbolQueryContext {
    pub fn read_symbol(&self, symbol_path: &str) -> Result<SymbolReadResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_with_overrides(workspace_root, overrides, symbol_path)
            },
            |db_path, overrides| {
                symbols::read_symbol_from_index_with_overrides(db_path, overrides, symbol_path)
            },
        )
    }

    pub fn read_symbol_at_position(
        &self,
        file_path: &Path,
        position: &Position,
    ) -> Result<SymbolReadResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_at_position_with_overrides(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position,
                )
            },
        )
    }

    pub fn read_symbol_context(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_context_with_overrides(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_context_from_index_with_overrides(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                )
            },
        )
    }

    pub fn read_symbol_context_at_position(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_context_at_position_with_overrides(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_context_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position, direction,
                )
            },
        )
    }

    pub fn read_symbol_neighborhood_context(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolNeighborhoodContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_neighborhood_context_with_overrides(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_neighborhood_context_from_index_with_overrides(
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

    pub fn read_symbol_neighborhood_context_at_position(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolNeighborhoodContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_neighborhood_context_at_position_with_overrides(
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
                symbols::read_symbol_neighborhood_context_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position, direction, max_depth, max_nodes,
                )
            },
        )
    }

    pub fn read_symbol_discovery_context(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_discovery_context_with_overrides(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_discovery_context_from_index_with_overrides(
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

    pub fn read_symbol_discovery_context_at_position(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_discovery_context_at_position_with_overrides(
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
                symbols::read_symbol_discovery_context_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position, direction, max_depth, max_nodes,
                )
            },
        )
    }
}
