use std::path::Path;

use anyhow::Result;

use super::SymbolQueryContext;
use crate::model::{
    Position, SymbolContextResult, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, TraceDirection,
};
use crate::symbol_trace::TraceQueryDeadline;
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.read_symbol_with_deadline(symbol_path, &deadline)
    }

    pub(crate) fn read_symbol_with_deadline(
        &self,
        symbol_path: &str,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolReadResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::read_symbol_with_overrides_with_deadline(
                    workspace_root,
                    overrides,
                    symbol_path,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::read_symbol_from_index_with_overrides_with_deadline(
                    db_path,
                    overrides,
                    symbol_path,
                    deadline,
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.read_symbol_at_position_with_deadline(file_path, position, &deadline)
    }

    pub(crate) fn read_symbol_at_position_with_deadline(
        &self,
        file_path: &Path,
        position: &Position,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolReadResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::read_symbol_at_position_with_overrides_with_deadline(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::read_symbol_at_position_from_index_with_overrides_with_deadline(
                    db_path, overrides, file_path, position, deadline,
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.read_symbol_context_with_deadline(symbol_path, direction, &deadline)
    }

    pub(crate) fn read_symbol_context_with_deadline(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolContextResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::read_symbol_context_with_overrides_with_deadline(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::read_symbol_context_from_index_with_overrides_with_deadline(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                    deadline,
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.read_symbol_context_at_position_with_deadline(
            file_path, position, direction, &deadline,
        )
    }

    pub(crate) fn read_symbol_context_at_position_with_deadline(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolContextResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::read_symbol_context_at_position_with_overrides_with_deadline(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                    deadline,
                )
            },
            |db_path, overrides, deadline| {
                symbols::read_symbol_context_at_position_from_index_with_overrides_with_deadline(
                    db_path, overrides, file_path, position, direction, deadline,
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.read_symbol_neighborhood_context_with_deadline(
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            &deadline,
        )
    }

    pub(crate) fn read_symbol_neighborhood_context_with_deadline(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolNeighborhoodContextResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::read_symbol_neighborhood_context_with_overrides_with_deadline(
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
                symbols::read_symbol_neighborhood_context_from_index_with_overrides_with_deadline(
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.read_symbol_neighborhood_context_at_position_with_deadline(
            file_path, position, direction, max_depth, max_nodes, &deadline,
        )
    }

    pub(crate) fn read_symbol_neighborhood_context_at_position_with_deadline(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolNeighborhoodContextResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::read_symbol_neighborhood_context_at_position_with_overrides_with_deadline(
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
                symbols::read_symbol_neighborhood_context_at_position_from_index_with_overrides_with_deadline(
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.read_symbol_discovery_context_with_deadline(
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            &deadline,
        )
    }

    pub(crate) fn read_symbol_discovery_context_with_deadline(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::read_symbol_discovery_context_with_overrides_with_deadline(
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
                symbols::read_symbol_discovery_context_from_index_with_overrides_with_deadline(
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        self.read_symbol_discovery_context_at_position_with_deadline(
            file_path, position, direction, max_depth, max_nodes, &deadline,
        )
    }

    pub(crate) fn read_symbol_discovery_context_at_position_with_deadline(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        deadline: &TraceQueryDeadline,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        self.dispatch_with_deadline(
            deadline,
            |workspace_root, overrides, deadline| {
                symbols::read_symbol_discovery_context_at_position_with_overrides_with_deadline(
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
                symbols::read_symbol_discovery_context_at_position_from_index_with_overrides_with_deadline(
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
