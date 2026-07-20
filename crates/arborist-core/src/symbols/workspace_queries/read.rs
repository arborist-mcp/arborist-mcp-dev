use std::path::Path;

use anyhow::Result;

use crate::model::Position;
use crate::model::{
    SymbolContextResult, SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult,
    SymbolReadResult, TraceDirection,
};
use crate::symbol_index_workspace::load_live_workspace_symbols;
use crate::symbol_query_execution::{
    read_symbol_at_position_from_symbols, read_symbol_context_at_position_from_symbols,
    read_symbol_context_from_symbols, read_symbol_discovery_context_at_position_from_symbols,
    read_symbol_discovery_context_from_symbols, read_symbol_from_symbols,
    read_symbol_neighborhood_context_at_position_from_symbols,
    read_symbol_neighborhood_context_from_symbols,
};

use super::load_live_workspace_symbols_at_path;

pub fn read_symbol(workspace_root: &Path, symbol_path: &str) -> Result<SymbolReadResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    read_symbol_from_symbols(&resolved_symbols, indexed_files, symbol_path, None)
}

pub fn read_symbol_context(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    read_symbol_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        None,
    )
}

pub fn read_symbol_neighborhood_context(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    read_symbol_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_discovery_context(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    read_symbol_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_at_position(
    workspace_root: &Path,
    file_path: &Path,
    position: &Position,
) -> Result<SymbolReadResult> {
    let (file_path, resolved_symbols, indexed_files) =
        load_live_workspace_symbols_at_path(workspace_root, file_path)?;
    read_symbol_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        None,
    )
}

pub fn read_symbol_context_at_position(
    workspace_root: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let (file_path, resolved_symbols, indexed_files) =
        load_live_workspace_symbols_at_path(workspace_root, file_path)?;
    read_symbol_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        None,
    )
}

pub fn read_symbol_neighborhood_context_at_position(
    workspace_root: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    let (file_path, resolved_symbols, indexed_files) =
        load_live_workspace_symbols_at_path(workspace_root, file_path)?;
    read_symbol_neighborhood_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_discovery_context_at_position(
    workspace_root: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let (file_path, resolved_symbols, indexed_files) =
        load_live_workspace_symbols_at_path(workspace_root, file_path)?;
    read_symbol_discovery_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}
