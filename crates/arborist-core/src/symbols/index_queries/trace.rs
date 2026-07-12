use std::path::Path;

use anyhow::Result;

use crate::language::normalize_absolute_path;
use crate::model::Position;
use crate::model::{TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult};
use crate::symbol_index_state::load_symbol_index;
use crate::symbol_query_execution::{
    trace_from_symbols, trace_neighborhood_from_symbols,
    trace_symbol_graph_at_position_from_symbols,
    trace_symbol_neighborhood_at_position_from_symbols,
};

pub fn trace_symbol_graph_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    trace_from_symbols(&resolved_symbols, indexed_files, symbol_path, direction)
}

pub fn trace_symbol_neighborhood_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    trace_neighborhood_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
    )
}

pub fn trace_symbol_graph_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    trace_symbol_graph_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        None,
    )
}

pub fn trace_symbol_neighborhood_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    trace_symbol_neighborhood_at_position_from_symbols(
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
