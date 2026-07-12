use std::path::Path;

use anyhow::Result;

use super::super::VirtualFileSystem;
use crate::language::normalize_absolute_path;
use crate::model::{
    SymbolContextResult, SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult,
    SymbolReadResult, TraceDirection,
};
use crate::symbols::{
    read_symbol_at_position_with_overrides, read_symbol_context_at_position_with_overrides,
    read_symbol_context_with_overrides, read_symbol_discovery_context_at_position_with_overrides,
    read_symbol_discovery_context_with_overrides,
    read_symbol_neighborhood_context_at_position_with_overrides,
    read_symbol_neighborhood_context_with_overrides, read_symbol_with_overrides,
};

impl VirtualFileSystem {
    pub fn read_symbol(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
    ) -> Result<SymbolReadResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_with_overrides(&workspace_root, &overrides, symbol_path)
    }

    pub fn read_symbol_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
    ) -> Result<SymbolReadResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_at_position_with_overrides(&workspace_root, &overrides, file_path, position)
    }

    pub fn read_symbol_context(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_context_with_overrides(&workspace_root, &overrides, symbol_path, direction)
    }

    pub fn read_symbol_context_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_context_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
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
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_neighborhood_context_with_overrides(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
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
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_neighborhood_context_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
            max_depth,
            max_nodes,
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
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_discovery_context_with_overrides(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
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
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_discovery_context_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
            max_depth,
            max_nodes,
        )
    }
}
