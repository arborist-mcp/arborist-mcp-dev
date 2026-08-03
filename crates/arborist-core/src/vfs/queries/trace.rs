use std::path::Path;

use anyhow::Result;

use super::super::VirtualFileSystem;
use crate::language::normalize_absolute_path;
use crate::model::{TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult};
use crate::symbol_trace::TraceQueryDeadline;
use crate::symbols::{
    trace_symbol_graph_at_position_with_overrides_with_deadline,
    trace_symbol_graph_with_overrides_with_deadline,
    trace_symbol_neighborhood_at_position_with_overrides_with_deadline,
    trace_symbol_neighborhood_with_overrides_with_deadline,
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        trace_symbol_graph_with_overrides_with_deadline(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            &deadline,
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        trace_symbol_neighborhood_with_overrides_with_deadline(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            &deadline,
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        trace_symbol_graph_at_position_with_overrides_with_deadline(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
            &deadline,
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
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        trace_symbol_neighborhood_at_position_with_overrides_with_deadline(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
            max_depth,
            max_nodes,
            &deadline,
        )
    }
}
