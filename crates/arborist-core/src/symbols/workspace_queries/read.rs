use std::path::Path;

use anyhow::Result;

use crate::model::Position;
use crate::model::{
    SymbolContextResult, SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult,
    SymbolReadResult, TraceDirection,
};
use crate::symbol_index_workspace::{
    load_live_workspace_symbols, load_live_workspace_symbols_with_timeout,
};
use crate::symbol_query_execution::{
    read_symbol_at_position_from_symbols, read_symbol_context_at_position_from_symbols,
    read_symbol_context_from_symbols, read_symbol_discovery_context_at_position_from_symbols,
    read_symbol_discovery_context_from_symbols, read_symbol_from_symbols,
    read_symbol_neighborhood_context_at_position_from_symbols,
    read_symbol_neighborhood_context_at_position_from_symbols_with_timeout,
    read_symbol_neighborhood_context_from_symbols,
    read_symbol_neighborhood_context_from_symbols_with_timeout,
};
use crate::symbol_trace::TraceQueryDeadline;

use super::{
    load_live_workspace_symbols_at_path, load_live_workspace_symbols_at_path_with_timeout,
};

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
    read_symbol_neighborhood_context_with_timeout(
        workspace_root,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_neighborhood_context_with_timeout(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_live_workspace_symbols_with_timeout(workspace_root, timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol neighborhood read")?;
    read_symbol_neighborhood_context_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
        timeout_ms,
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
    read_symbol_neighborhood_context_at_position_with_timeout(
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
    workspace_root: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (file_path, resolved_symbols, indexed_files) =
        load_live_workspace_symbols_at_path_with_timeout(workspace_root, file_path, timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol neighborhood read")?;
    read_symbol_neighborhood_context_at_position_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
        timeout_ms,
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
