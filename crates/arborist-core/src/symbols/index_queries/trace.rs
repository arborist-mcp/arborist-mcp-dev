use std::path::Path;

use anyhow::Result;

use crate::language::normalize_absolute_path;
use crate::model::Position;
use crate::model::{TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult};
use crate::symbol_query_execution::{
    trace_from_symbols_with_deadline, trace_neighborhood_from_symbols_with_deadline,
    trace_symbol_graph_at_position_from_symbols_with_deadline,
    trace_symbol_neighborhood_at_position_from_symbols_with_deadline,
};
use crate::symbol_trace::TraceQueryDeadline;

use super::load_normalized_symbol_index_with_timeout;

pub fn trace_symbol_graph_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_symbol_graph_from_index_with_timeout(db_path, symbol_path, direction, None)
}

pub fn trace_symbol_graph_from_index_with_timeout(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_timeout(db_path, timeout_ms)?;
    trace_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        &deadline,
    )
}

pub fn trace_symbol_neighborhood_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    trace_symbol_neighborhood_from_index_with_timeout(
        db_path,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn trace_symbol_neighborhood_from_index_with_timeout(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_timeout(db_path, timeout_ms)?;
    trace_neighborhood_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        &deadline,
    )
}

pub fn trace_symbol_graph_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_symbol_graph_at_position_from_index_with_timeout(
        db_path, file_path, position, direction, None,
    )
}

pub fn trace_symbol_graph_at_position_from_index_with_timeout(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let file_path = normalize_absolute_path(file_path)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_timeout(db_path, timeout_ms)?;
    trace_symbol_graph_at_position_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        None,
        &deadline,
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
    trace_symbol_neighborhood_at_position_from_index_with_timeout(
        db_path, file_path, position, direction, max_depth, max_nodes, None,
    )
}

pub fn trace_symbol_neighborhood_at_position_from_index_with_timeout(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let file_path = normalize_absolute_path(file_path)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_timeout(db_path, timeout_ms)?;
    trace_symbol_neighborhood_at_position_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
        &deadline,
    )
}
