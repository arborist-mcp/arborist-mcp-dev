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
        self.read_symbol_with_timeout(symbol_path, None)
    }

    pub fn read_symbol_with_timeout(
        &self,
        symbol_path: &str,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolReadResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_with_overrides_with_timeout(
                    workspace_root,
                    overrides,
                    symbol_path,
                    timeout_ms,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_from_index_with_overrides_with_timeout(
                    db_path,
                    overrides,
                    symbol_path,
                    timeout_ms,
                )
            },
        )
    }

    pub fn read_symbol_at_position(
        &self,
        file_path: &Path,
        position: &Position,
    ) -> Result<SymbolReadResult> {
        self.read_symbol_at_position_with_timeout(file_path, position, None)
    }

    pub fn read_symbol_at_position_with_timeout(
        &self,
        file_path: &Path,
        position: &Position,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolReadResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_at_position_with_overrides_with_timeout(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    timeout_ms,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_at_position_from_index_with_overrides_with_timeout(
                    db_path, overrides, file_path, position, timeout_ms,
                )
            },
        )
    }

    pub fn read_symbol_context(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        self.read_symbol_context_with_timeout(symbol_path, direction, None)
    }

    pub fn read_symbol_context_with_timeout(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_context_with_overrides_with_timeout(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    timeout_ms,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_context_from_index_with_overrides_with_timeout(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                    timeout_ms,
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
        self.read_symbol_context_at_position_with_timeout(file_path, position, direction, None)
    }

    pub fn read_symbol_context_at_position_with_timeout(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_context_at_position_with_overrides_with_timeout(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                    timeout_ms,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_context_at_position_from_index_with_overrides_with_timeout(
                    db_path, overrides, file_path, position, direction, timeout_ms,
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
        self.read_symbol_neighborhood_context_with_timeout(
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            None,
        )
    }

    pub fn read_symbol_neighborhood_context_with_timeout(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolNeighborhoodContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_neighborhood_context_with_overrides_with_timeout(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                    timeout_ms,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_neighborhood_context_from_index_with_overrides_with_timeout(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                    timeout_ms,
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
        self.read_symbol_neighborhood_context_at_position_with_timeout(
            file_path, position, direction, max_depth, max_nodes, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn read_symbol_neighborhood_context_at_position_with_timeout(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolNeighborhoodContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_neighborhood_context_at_position_with_overrides_with_timeout(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                    max_depth,
                    max_nodes,
                    timeout_ms,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_neighborhood_context_at_position_from_index_with_overrides_with_timeout(
                    db_path,
                    overrides,
                    file_path,
                    position,
                    direction,
                    max_depth,
                    max_nodes,
                    timeout_ms,
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
        self.read_symbol_discovery_context_with_timeout(
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            None,
        )
    }

    pub fn read_symbol_discovery_context_with_timeout(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_discovery_context_with_overrides_with_timeout(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                    timeout_ms,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_discovery_context_from_index_with_overrides_with_timeout(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                    timeout_ms,
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
        self.read_symbol_discovery_context_at_position_with_timeout(
            file_path, position, direction, max_depth, max_nodes, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn read_symbol_discovery_context_at_position_with_timeout(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_discovery_context_at_position_with_overrides_with_timeout(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                    max_depth,
                    max_nodes,
                    timeout_ms,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_discovery_context_at_position_from_index_with_overrides_with_timeout(
                    db_path,
                    overrides,
                    file_path,
                    position,
                    direction,
                    max_depth,
                    max_nodes,
                    timeout_ms,
                )
            },
        )
    }
}
