use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, anyhow};

use super::{choose_trace_symbol_with_deadline, read_symbol_from_meta, validate_trace_symbol_path};
use crate::model::{
    Position, SymbolContextResult, SymbolMeta, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, TraceDirection,
};
use crate::symbol_map::resolved_symbol_ref_map;
use crate::symbol_position::resolve_symbol_at_position_with_deadline;
use crate::symbol_read::read_symbol_result_from_meta_with_cache;
use crate::symbol_trace::{
    TraceQueryDeadline, trace_from_symbol_with_timeout, trace_neighborhood_from_symbol_with_timeout,
};

pub(crate) fn read_symbol_context_from_meta_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("symbol context read")?;
    let read = read_symbol_from_meta(symbol, indexed_files, file_overrides)?;
    let timeout_ms = deadline.remaining_timeout_ms("symbol context trace")?;
    let trace = trace_from_symbol_with_timeout(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        timeout_ms,
    )?;
    let result = SymbolContextResult { read, trace };
    deadline.check("symbol context result")?;
    result.validate_public_output()?;
    Ok(result)
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_discovery_context_from_meta_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadDiscoveryContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let mut source_cache = BTreeMap::new();
    deadline.check("symbol discovery read")?;
    let read = read_symbol_result_from_meta_with_cache(
        symbol,
        indexed_files,
        file_overrides,
        &mut source_cache,
    )?;
    let timeout_ms = deadline.remaining_timeout_ms("symbol discovery trace")?;
    let trace = trace_from_symbol_with_timeout(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        timeout_ms,
    )?;
    let timeout_ms = deadline.remaining_timeout_ms("symbol discovery neighborhood")?;
    let neighborhood_context = read_symbol_neighborhood_context_from_meta_with_timeout_and_cache(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
        timeout_ms,
        &mut source_cache,
    )?;
    let result = SymbolReadDiscoveryContextResult {
        read,
        trace,
        neighborhood_context,
    };
    deadline.check("symbol discovery result")?;
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn read_symbol_at_position_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("symbol position resolution")?;
    let symbol = resolve_symbol_at_position_with_deadline(
        resolved_symbols,
        file_path,
        position,
        file_overrides,
        Some(&deadline),
    )?;
    deadline.check("symbol position read")?;
    let result = read_symbol_from_meta(symbol, indexed_files, file_overrides)?;
    deadline.check("symbol position result")?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_context_at_position_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("symbol context position resolution")?;
    let symbol = resolve_symbol_at_position_with_deadline(
        resolved_symbols,
        file_path,
        position,
        file_overrides,
        Some(&deadline),
    )?;
    let timeout_ms = deadline.remaining_timeout_ms("symbol context")?;
    read_symbol_context_from_meta_with_timeout(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        file_overrides,
        timeout_ms,
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
    let symbol = resolve_symbol_at_position_with_deadline(
        resolved_symbols,
        file_path,
        position,
        file_overrides,
        Some(&deadline),
    )?;
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
pub(crate) fn read_symbol_discovery_context_at_position_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadDiscoveryContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("symbol discovery position resolution")?;
    let symbol = resolve_symbol_at_position_with_deadline(
        resolved_symbols,
        file_path,
        position,
        file_overrides,
        Some(&deadline),
    )?;
    let timeout_ms = deadline.remaining_timeout_ms("symbol discovery context")?;
    read_symbol_discovery_context_from_meta_with_timeout(
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

pub(crate) fn read_symbol_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    validate_trace_symbol_path(symbol_path)?;
    deadline.check("symbol resolution")?;

    let symbol = choose_trace_symbol_with_deadline(resolved_symbols, symbol_path, Some(&deadline))?
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    deadline.check("symbol read")?;
    let result = read_symbol_from_meta(symbol, indexed_files, file_overrides)?;
    deadline.check("symbol read result")?;
    Ok(result)
}

pub(crate) fn read_symbol_context_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    validate_trace_symbol_path(symbol_path)?;
    deadline.check("symbol context resolution")?;

    let symbol = choose_trace_symbol_with_deadline(resolved_symbols, symbol_path, Some(&deadline))?
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    let timeout_ms = deadline.remaining_timeout_ms("symbol context")?;
    read_symbol_context_from_meta_with_timeout(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        file_overrides,
        timeout_ms,
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

    let symbol = choose_trace_symbol_with_deadline(resolved_symbols, symbol_path, Some(&deadline))?
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_discovery_context_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadDiscoveryContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    validate_trace_symbol_path(symbol_path)?;
    deadline.check("symbol discovery resolution")?;

    let symbol = choose_trace_symbol_with_deadline(resolved_symbols, symbol_path, Some(&deadline))?
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    let timeout_ms = deadline.remaining_timeout_ms("symbol discovery context")?;
    read_symbol_discovery_context_from_meta_with_timeout(
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::Error;

    use super::{
        read_symbol_at_position_from_symbols_with_timeout,
        read_symbol_context_at_position_from_symbols_with_timeout,
        read_symbol_context_from_symbols_with_timeout,
        read_symbol_discovery_context_at_position_from_symbols_with_timeout,
        read_symbol_discovery_context_from_symbols_with_timeout,
        read_symbol_from_symbols_with_timeout,
    };
    use crate::model::{Position, TraceDirection};

    fn assert_zero_timeout(error: Error) {
        assert!(
            error
                .to_string()
                .contains("invalid trace timeout_ms: value must be greater than zero")
        );
    }

    #[test]
    fn direct_read_variants_reject_zero_timeout_before_symbol_resolution() {
        let symbols = Vec::new();
        assert_zero_timeout(
            read_symbol_from_symbols_with_timeout(&symbols, 0, "missing", None, Some(0))
                .expect_err("base read should reject zero timeout"),
        );
        assert_zero_timeout(
            read_symbol_context_from_symbols_with_timeout(
                &symbols,
                0,
                "missing",
                TraceDirection::Both,
                None,
                Some(0),
            )
            .expect_err("context read should reject zero timeout"),
        );
        assert_zero_timeout(
            read_symbol_discovery_context_from_symbols_with_timeout(
                &symbols,
                0,
                "missing",
                TraceDirection::Both,
                2,
                64,
                None,
                Some(0),
            )
            .expect_err("discovery read should reject zero timeout"),
        );
    }

    #[test]
    fn direct_position_read_variants_reject_zero_timeout_before_resolution() {
        let symbols = Vec::new();
        let file_path = Path::new("missing.py");
        let position = Position { row: 0, column: 0 };
        assert_zero_timeout(
            read_symbol_at_position_from_symbols_with_timeout(
                &symbols,
                0,
                file_path,
                &position,
                None,
                Some(0),
            )
            .expect_err("position read should reject zero timeout"),
        );
        assert_zero_timeout(
            read_symbol_context_at_position_from_symbols_with_timeout(
                &symbols,
                0,
                file_path,
                &position,
                TraceDirection::Both,
                None,
                Some(0),
            )
            .expect_err("position context read should reject zero timeout"),
        );
        assert_zero_timeout(
            read_symbol_discovery_context_at_position_from_symbols_with_timeout(
                &symbols,
                0,
                file_path,
                &position,
                TraceDirection::Both,
                2,
                64,
                None,
                Some(0),
            )
            .expect_err("position discovery read should reject zero timeout"),
        );
    }
}
