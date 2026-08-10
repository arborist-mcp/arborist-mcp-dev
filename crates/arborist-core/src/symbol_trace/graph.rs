use crate::diagnostics::DiagnosticsSink;
use crate::model::{SymbolMeta, TraceDirection, TraceSymbolGraphResult};
use crate::symbol_summary::{summarize_symbols_with_deadline, trace_evidence_keys_with_deadline};

use super::TraceQueryDeadline;
use anyhow::Result;

pub(crate) fn trace_from_symbol_with_deadline(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    deadline: &TraceQueryDeadline,
) -> Result<TraceSymbolGraphResult> {
    trace_from_symbol_with_deadline_and_diagnostics(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        deadline,
        None,
    )
}

#[allow(clippy::needless_option_as_deref)] // reborrow: the sink is shared by callers and callees expansion
pub(crate) fn trace_from_symbol_with_deadline_and_diagnostics(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    deadline: &TraceQueryDeadline,
    mut diagnostics: Option<&mut DiagnosticsSink>,
) -> Result<TraceSymbolGraphResult> {
    deadline.check("starting graph expansion")?;
    let symbol = symbol.clone().with_origin_type("trace_root");

    let callers = if matches!(direction, TraceDirection::Callers | TraceDirection::Both) {
        deadline.check("expanding callers")?;
        summarize_symbols_with_deadline(
            resolved_symbols,
            &symbol.references,
            None,
            deadline,
            diagnostics.as_deref_mut(),
        )?
    } else {
        Vec::new()
    };

    let callees = if matches!(direction, TraceDirection::Callees | TraceDirection::Both) {
        deadline.check("expanding callees")?;
        summarize_symbols_with_deadline(
            resolved_symbols,
            &symbol.dependencies,
            Some(&symbol.file_path),
            deadline,
            diagnostics.as_deref_mut(),
        )?
    } else {
        Vec::new()
    };
    deadline.check("validating graph output")?;

    let result = TraceSymbolGraphResult {
        evidence_keys: trace_evidence_keys_with_deadline(&symbol, &callers, &callees, deadline)?,
        symbol,
        callers,
        callees,
        indexed_files,
    };
    result.validate_public_output()?;
    deadline.check("validating graph output")?;
    Ok(result)
}
