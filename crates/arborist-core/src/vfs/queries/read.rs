use std::path::Path;

use anyhow::Result;

use super::super::VirtualFileSystem;
use crate::language::normalize_absolute_path;
use crate::model::{
    SymbolContextResult, SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult,
    SymbolReadResult, TraceDirection,
};
use crate::symbol_trace::TraceQueryDeadline;
use crate::symbols::{
    read_symbol_at_position_with_overrides_with_deadline,
    read_symbol_context_at_position_with_overrides_with_deadline,
    read_symbol_context_with_overrides_with_deadline,
    read_symbol_discovery_context_at_position_with_overrides_with_deadline,
    read_symbol_discovery_context_with_overrides_with_deadline,
    read_symbol_neighborhood_context_at_position_with_overrides_with_deadline,
    read_symbol_neighborhood_context_with_overrides_with_deadline,
    read_symbol_with_overrides_with_deadline,
};

impl VirtualFileSystem {
    pub fn read_symbol(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
    ) -> Result<SymbolReadResult> {
        self.read_symbol_with_timeout(workspace_root, symbol_path, None)
    }

    pub fn read_symbol_with_timeout(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolReadResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        read_symbol_with_overrides_with_deadline(
            &workspace_root,
            &overrides,
            symbol_path,
            &deadline,
        )
    }

    pub fn read_symbol_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
    ) -> Result<SymbolReadResult> {
        self.read_symbol_at_position_with_timeout(workspace_root, file_path, position, None)
    }

    pub fn read_symbol_at_position_with_timeout(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolReadResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        read_symbol_at_position_with_overrides_with_deadline(
            &workspace_root,
            &overrides,
            file_path,
            position,
            &deadline,
        )
    }

    pub fn read_symbol_context(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        self.read_symbol_context_with_timeout(workspace_root, symbol_path, direction, None)
    }

    pub fn read_symbol_context_with_timeout(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolContextResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        read_symbol_context_with_overrides_with_deadline(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            &deadline,
        )
    }

    pub fn read_symbol_context_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        self.read_symbol_context_at_position_with_timeout(
            workspace_root,
            file_path,
            position,
            direction,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn read_symbol_context_at_position_with_timeout(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolContextResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        read_symbol_context_at_position_with_overrides_with_deadline(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
            &deadline,
        )
    }

    pub fn read_symbol_neighborhood_context(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolNeighborhoodContextResult> {
        self.read_symbol_neighborhood_context_with_timeout(
            workspace_root,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn read_symbol_neighborhood_context_with_timeout(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolNeighborhoodContextResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        read_symbol_neighborhood_context_with_overrides_with_deadline(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            &deadline,
        )
    }

    pub fn read_symbol_neighborhood_context_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolNeighborhoodContextResult> {
        self.read_symbol_neighborhood_context_at_position_with_timeout(
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
    pub fn read_symbol_neighborhood_context_at_position_with_timeout(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolNeighborhoodContextResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        read_symbol_neighborhood_context_at_position_with_overrides_with_deadline(
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

    pub fn read_symbol_discovery_context(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        self.read_symbol_discovery_context_with_timeout(
            workspace_root,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn read_symbol_discovery_context_with_timeout(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        read_symbol_discovery_context_with_overrides_with_deadline(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            &deadline,
        )
    }

    pub fn read_symbol_discovery_context_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        self.read_symbol_discovery_context_at_position_with_timeout(
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
    pub fn read_symbol_discovery_context_at_position_with_timeout(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        let deadline = TraceQueryDeadline::new(timeout_ms)?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        deadline.check("virtual symbol query setup")?;
        let overrides =
            self.virtual_overrides_for_workspace_with_deadline(&workspace_root, &deadline)?;
        deadline.check("virtual symbol query setup")?;
        read_symbol_discovery_context_at_position_with_overrides_with_deadline(
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
