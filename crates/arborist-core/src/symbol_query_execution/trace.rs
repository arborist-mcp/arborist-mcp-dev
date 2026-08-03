use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, anyhow};

use super::{choose_trace_symbol_with_deadline, validate_trace_symbol_path};
use crate::model::{
    Position, SymbolMeta, TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult,
};
use crate::symbol_position::resolve_symbol_at_position_with_deadline;
use crate::symbol_trace::{
    TraceQueryDeadline, trace_from_symbol_with_deadline,
    trace_neighborhood_from_symbol_with_deadline,
};

#[allow(dead_code)]
pub(crate) fn trace_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        None,
    )
}

pub(crate) fn trace_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    validate_trace_symbol_path(symbol_path)?;
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    trace_from_symbols_with_deadline(
        resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        &deadline,
    )
}

pub(crate) fn trace_from_symbols_with_deadline(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    deadline: &TraceQueryDeadline,
) -> Result<TraceSymbolGraphResult> {
    validate_trace_symbol_path(symbol_path)?;
    deadline.check("selecting trace symbol")?;

    let symbol = choose_trace_symbol_with_deadline(resolved_symbols, symbol_path, Some(deadline))?
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    trace_from_symbol_with_deadline(resolved_symbols, indexed_files, symbol, direction, deadline)
}

pub(crate) fn trace_neighborhood_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    validate_trace_symbol_path(symbol_path)?;
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    trace_neighborhood_from_symbols_with_deadline(
        resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        &deadline,
    )
}

pub(crate) fn trace_neighborhood_from_symbols_with_deadline(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    deadline: &TraceQueryDeadline,
) -> Result<TraceSymbolNeighborhoodResult> {
    validate_trace_symbol_path(symbol_path)?;
    deadline.check("selecting trace symbol")?;

    let symbol = choose_trace_symbol_with_deadline(resolved_symbols, symbol_path, Some(deadline))?
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    trace_neighborhood_from_symbol_with_deadline(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        deadline,
    )
}

#[allow(dead_code)]
pub(crate) fn trace_symbol_graph_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<TraceSymbolGraphResult> {
    trace_symbol_graph_at_position_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
        file_path,
        position,
        direction,
        file_overrides,
        None,
    )
}

pub(crate) fn trace_symbol_graph_at_position_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    trace_symbol_graph_at_position_from_symbols_with_deadline(
        resolved_symbols,
        indexed_files,
        file_path,
        position,
        direction,
        file_overrides,
        &deadline,
    )
}

pub(crate) fn trace_symbol_graph_at_position_from_symbols_with_deadline(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: &TraceQueryDeadline,
) -> Result<TraceSymbolGraphResult> {
    deadline.check("resolving trace position")?;
    let symbol = resolve_symbol_at_position_with_deadline(
        resolved_symbols,
        file_path,
        position,
        file_overrides,
        Some(deadline),
    )?;
    trace_from_symbol_with_deadline(resolved_symbols, indexed_files, symbol, direction, deadline)
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
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
    trace_symbol_neighborhood_at_position_from_symbols_with_timeout(
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
pub(crate) fn trace_symbol_neighborhood_at_position_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    trace_symbol_neighborhood_at_position_from_symbols_with_deadline(
        resolved_symbols,
        indexed_files,
        file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
        &deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn trace_symbol_neighborhood_at_position_from_symbols_with_deadline(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: &TraceQueryDeadline,
) -> Result<TraceSymbolNeighborhoodResult> {
    deadline.check("resolving trace position")?;
    let symbol = resolve_symbol_at_position_with_deadline(
        resolved_symbols,
        file_path,
        position,
        file_overrides,
        Some(deadline),
    )?;
    trace_neighborhood_from_symbol_with_deadline(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        deadline,
    )
}

#[cfg(test)]
mod tests {
    use super::{trace_from_symbols_with_deadline, trace_neighborhood_from_symbols_with_deadline};
    use crate::model::TraceDirection;
    use crate::symbol_trace::TraceQueryDeadline;

    #[test]
    fn graph_and_neighborhood_selection_reuse_the_callers_deadline() {
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let graph_error =
            trace_from_symbols_with_deadline(&[], 0, "helper", TraceDirection::Both, &deadline)
                .expect_err("trace graph should honor an already-expired deadline");
        assert!(graph_error.to_string().contains("selecting trace symbol"));

        let neighborhood_error = trace_neighborhood_from_symbols_with_deadline(
            &[],
            0,
            "helper",
            TraceDirection::Both,
            1,
            1,
            &deadline,
        )
        .expect_err("trace neighborhood should honor an already-expired deadline");
        assert!(
            neighborhood_error
                .to_string()
                .contains("selecting trace symbol")
        );
    }
}
