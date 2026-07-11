use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, anyhow};

use super::{choose_trace_symbol, validate_trace_symbol_path};
use crate::model::{
    Position, SymbolMeta, TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult,
};
use crate::symbol_position::resolve_symbol_at_position;
use crate::symbol_trace::{trace_from_symbol, trace_neighborhood_from_symbol};

pub(crate) fn trace_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    validate_trace_symbol_path(symbol_path)?;

    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    trace_from_symbol(resolved_symbols, indexed_files, symbol, direction)
}

pub(crate) fn trace_neighborhood_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    validate_trace_symbol_path(symbol_path)?;
    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    trace_neighborhood_from_symbol(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
    )
}

pub(crate) fn trace_symbol_graph_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<TraceSymbolGraphResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    trace_from_symbol(resolved_symbols, indexed_files, symbol, direction)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn trace_symbol_neighborhood_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<TraceSymbolNeighborhoodResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    trace_neighborhood_from_symbol(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
    )
}
