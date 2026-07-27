use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, anyhow};

use super::{choose_trace_symbol, read_symbol_from_meta, validate_trace_symbol_path};
use crate::model::{
    Position, SymbolContextResult, SymbolMeta, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, TraceDirection,
};
use crate::symbol_map::resolved_symbol_ref_map;
use crate::symbol_position::resolve_symbol_at_position;
use crate::symbol_read::read_symbol_result_from_meta_with_cache;
use crate::symbol_trace::{
    TraceQueryDeadline, trace_from_symbol, trace_neighborhood_from_symbol_with_timeout,
};

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
    read_symbol_neighborhood_context_from_meta_with_timeout(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_neighborhood_context_from_meta_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    let mut source_cache = BTreeMap::new();
    read_symbol_neighborhood_context_from_meta_with_timeout_and_cache(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
        timeout_ms,
        &mut source_cache,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_neighborhood_context_from_meta_with_timeout_and_cache(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
    source_cache: &mut BTreeMap<String, String>,
) -> Result<SymbolNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("neighborhood expansion")?;
    let neighborhood = trace_neighborhood_from_symbol_with_timeout(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        timeout_ms,
    )?;
    let resolved_map = resolved_symbol_ref_map(resolved_symbols);
    let mut reads = Vec::with_capacity(neighborhood.nodes.len());

    for node in &neighborhood.nodes {
        deadline.check("neighborhood context reads")?;
        let symbol = resolved_map
            .get(node.symbol.symbol_id.as_str())
            .ok_or_else(|| {
                anyhow!(
                    "symbol not found in workspace index while reading neighborhood node: {}",
                    node.symbol.symbol_id
                )
            })?;
        reads.push(read_symbol_result_from_meta_with_cache(
            symbol,
            indexed_files,
            file_overrides,
            source_cache,
        )?);
    }

    let result = SymbolNeighborhoodContextResult {
        neighborhood,
        reads,
    };
    deadline.check("neighborhood context result")?;
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
    let mut source_cache = BTreeMap::new();
    let read = read_symbol_result_from_meta_with_cache(
        symbol,
        indexed_files,
        file_overrides,
        &mut source_cache,
    )?;
    let trace = trace_from_symbol(resolved_symbols, indexed_files, symbol, direction)?;
    let neighborhood_context = read_symbol_neighborhood_context_from_meta_with_timeout_and_cache(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
        None,
        &mut source_cache,
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
    read_symbol_neighborhood_context_at_position_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
        file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_neighborhood_context_at_position_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("symbol neighborhood position resolution")?;
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    let timeout_ms = deadline.remaining_timeout_ms("symbol neighborhood context")?;
    read_symbol_neighborhood_context_from_meta_with_timeout(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
        timeout_ms,
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
    read_symbol_neighborhood_context_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_neighborhood_context_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    validate_trace_symbol_path(symbol_path)?;
    deadline.check("symbol neighborhood resolution")?;

    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    let timeout_ms = deadline.remaining_timeout_ms("symbol neighborhood context")?;
    read_symbol_neighborhood_context_from_meta_with_timeout(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
        timeout_ms,
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
