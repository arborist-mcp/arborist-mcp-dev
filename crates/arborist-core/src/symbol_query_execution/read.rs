use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, anyhow};

use super::{choose_trace_symbol, read_symbol_from_meta, validate_trace_symbol_path};
use crate::model::{
    Position, SymbolContextResult, SymbolMeta, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, TraceDirection,
};
use crate::symbol_map::resolved_symbol_map;
use crate::symbol_position::resolve_symbol_at_position;
use crate::symbol_read::read_symbol_result_from_meta;
use crate::symbol_trace::trace_from_symbol;

pub(crate) fn read_symbol_context_from_meta(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolContextResult> {
    let read = read_symbol_from_meta(symbol, indexed_files, file_overrides)?;
    let trace = trace_from_symbol(resolved_symbols, indexed_files, symbol, direction)?;
    let result = SymbolContextResult { read, trace };
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn read_symbol_neighborhood_context_from_meta(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolNeighborhoodContextResult> {
    let neighborhood = super::trace::trace_neighborhood_from_symbols(
        resolved_symbols,
        indexed_files,
        &symbol.symbol_id,
        direction,
        max_depth,
        max_nodes,
    )?;
    let resolved_map = resolved_symbol_map(resolved_symbols);
    let mut reads = Vec::with_capacity(neighborhood.nodes.len());

    for node in &neighborhood.nodes {
        let symbol = resolved_map.get(&node.symbol.symbol_id).ok_or_else(|| {
            anyhow!(
                "symbol not found in workspace index while reading neighborhood node: {}",
                node.symbol.symbol_id
            )
        })?;
        reads.push(read_symbol_result_from_meta(
            symbol,
            indexed_files,
            file_overrides,
        )?);
    }

    let result = SymbolNeighborhoodContextResult {
        neighborhood,
        reads,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn read_symbol_discovery_context_from_meta(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadDiscoveryContextResult> {
    let read = read_symbol_from_meta(symbol, indexed_files, file_overrides)?;
    let trace = trace_from_symbol(resolved_symbols, indexed_files, symbol, direction)?;
    let neighborhood_context = read_symbol_neighborhood_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
    )?;
    let result = SymbolReadDiscoveryContextResult {
        read,
        trace,
        neighborhood_context,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn read_symbol_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    read_symbol_from_meta(symbol, indexed_files, file_overrides)
}

pub(crate) fn read_symbol_context_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolContextResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    read_symbol_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        file_overrides,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_neighborhood_context_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolNeighborhoodContextResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    read_symbol_neighborhood_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_discovery_context_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadDiscoveryContextResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    read_symbol_discovery_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
    )
}

pub(crate) fn read_symbol_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadResult> {
    validate_trace_symbol_path(symbol_path)?;

    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    read_symbol_from_meta(symbol, indexed_files, file_overrides)
}

pub(crate) fn read_symbol_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolContextResult> {
    validate_trace_symbol_path(symbol_path)?;

    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    read_symbol_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        file_overrides,
    )
}

pub(crate) fn read_symbol_neighborhood_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolNeighborhoodContextResult> {
    validate_trace_symbol_path(symbol_path)?;

    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    read_symbol_neighborhood_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
    )
}

pub(crate) fn read_symbol_discovery_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadDiscoveryContextResult> {
    validate_trace_symbol_path(symbol_path)?;

    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    read_symbol_discovery_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
    )
}
