use std::path::Path;

use anyhow::Result;

use crate::language::normalize_absolute_path;
use crate::model::Position;
use crate::model::{
    SymbolContextResult, SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult,
    SymbolReadResult, TraceDirection,
};
use crate::symbol_query_execution::{
    read_symbol_at_position_from_symbols, read_symbol_context_at_position_from_symbols,
    read_symbol_context_from_symbols, read_symbol_discovery_context_at_position_from_symbols,
    read_symbol_discovery_context_from_symbols, read_symbol_from_symbols,
    read_symbol_neighborhood_context_at_position_from_symbols_with_timeout,
    read_symbol_neighborhood_context_from_symbols_with_timeout,
};
use crate::symbol_trace::TraceQueryDeadline;

use super::{load_normalized_symbol_index, load_normalized_symbol_index_with_timeout};

pub fn read_symbol_from_index(db_path: &Path, symbol_path: &str) -> Result<SymbolReadResult> {
    let (resolved_symbols, indexed_files) = load_normalized_symbol_index(db_path)?;
    read_symbol_from_symbols(&resolved_symbols, indexed_files, symbol_path, None)
}

pub fn read_symbol_context_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let (resolved_symbols, indexed_files) = load_normalized_symbol_index(db_path)?;
    read_symbol_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        None,
    )
}

pub fn read_symbol_neighborhood_context_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    read_symbol_neighborhood_context_from_index_with_timeout(
        db_path,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_neighborhood_context_from_index_with_timeout(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_timeout(db_path, timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol neighborhood read")?;
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

pub fn read_symbol_discovery_context_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let (resolved_symbols, indexed_files) = load_normalized_symbol_index(db_path)?;
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

pub fn read_symbol_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
) -> Result<SymbolReadResult> {
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) = load_normalized_symbol_index(db_path)?;
    read_symbol_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        None,
    )
}

pub fn read_symbol_context_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) = load_normalized_symbol_index(db_path)?;
    read_symbol_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        None,
    )
}

pub fn read_symbol_neighborhood_context_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    read_symbol_neighborhood_context_at_position_from_index_with_timeout(
        db_path, file_path, position, direction, max_depth, max_nodes, None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_neighborhood_context_at_position_from_index_with_timeout(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let file_path = normalize_absolute_path(file_path)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_timeout(db_path, timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol neighborhood read")?;
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

pub fn read_symbol_discovery_context_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) = load_normalized_symbol_index(db_path)?;
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
