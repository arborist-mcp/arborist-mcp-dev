use std::path::Path;

use anyhow::Result;

use super::super::VirtualFileSystem;
use crate::language::normalize_absolute_path;
use crate::model::{TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult};
use crate::symbols::{
    trace_symbol_graph_at_position_with_overrides_and_timeout,
    trace_symbol_graph_with_overrides_and_timeout,
    trace_symbol_neighborhood_at_position_with_overrides_and_timeout,
    trace_symbol_neighborhood_with_overrides_and_timeout,
};

impl VirtualFileSystem {
    pub fn trace_symbol_graph(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<TraceSymbolGraphResult> {
        self.trace_symbol_graph_with_timeout(workspace_root, symbol_path, direction, None)
    }

    pub fn trace_symbol_graph_with_timeout(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        timeout_ms: Option<u64>,
    ) -> Result<TraceSymbolGraphResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_graph_with_overrides_and_timeout(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            timeout_ms,
        )
    }

    pub fn trace_symbol_neighborhood(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        self.trace_symbol_neighborhood_with_timeout(
            workspace_root,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            None,
        )
    }

    pub fn trace_symbol_neighborhood_with_timeout(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_neighborhood_with_overrides_and_timeout(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            timeout_ms,
        )
    }

    pub fn trace_symbol_graph_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
    ) -> Result<TraceSymbolGraphResult> {
        self.trace_symbol_graph_at_position_with_timeout(
            workspace_root,
            file_path,
            position,
            direction,
            None,
        )
    }

    pub fn trace_symbol_graph_at_position_with_timeout(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        timeout_ms: Option<u64>,
    ) -> Result<TraceSymbolGraphResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_graph_at_position_with_overrides_and_timeout(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
            timeout_ms,
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
        self.trace_symbol_neighborhood_at_position_with_timeout(
            workspace_root,
            file_path,
            position,
            direction,
            max_depth,
            max_nodes,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn trace_symbol_neighborhood_at_position_with_timeout(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_neighborhood_at_position_with_overrides_and_timeout(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
            max_depth,
            max_nodes,
            timeout_ms,
        )
    }
}
