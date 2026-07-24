use crate::model::{SymbolMeta, TraceDirection, TraceSymbolGraphResult};
use crate::symbol_summary::{summarize_symbols, trace_evidence_keys};

use super::TraceQueryDeadline;
use anyhow::Result;

pub(crate) fn trace_from_symbol(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_from_symbol_with_timeout(resolved_symbols, indexed_files, symbol, direction, None)
}

pub(crate) fn trace_from_symbol_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("starting graph expansion")?;
    let symbol = symbol.clone().with_origin_type("trace_root");

    let callers = if matches!(direction, TraceDirection::Callers | TraceDirection::Both) {
        deadline.check("expanding callers")?;
        summarize_symbols(resolved_symbols, &symbol.references, None)
    } else {
        Vec::new()
    };

    let callees = if matches!(direction, TraceDirection::Callees | TraceDirection::Both) {
        deadline.check("expanding callees")?;
        summarize_symbols(
            resolved_symbols,
            &symbol.dependencies,
            Some(&symbol.file_path),
        )
    } else {
        Vec::new()
    };
    deadline.check("validating graph output")?;

    let result = TraceSymbolGraphResult {
        evidence_keys: trace_evidence_keys(&symbol, &callers, &callees),
        symbol,
        callers,
        callees,
        indexed_files,
    };
    result.validate_public_output()?;
    Ok(result)
}
