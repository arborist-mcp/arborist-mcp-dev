use anyhow::Result;

use crate::model::{SymbolMeta, SymbolSummary, SymbolSummaryInit, TraceEvidenceKeys};
use crate::symbol_dependency::c_include_context_for_file;
use crate::symbol_trace::TraceQueryDeadline;

mod selection;

pub(crate) fn summarize_symbols_with_deadline(
    symbols: &[SymbolMeta],
    semantic_paths: &[String],
    context_file: Option<&str>,
    deadline: &TraceQueryDeadline,
) -> Result<Vec<SymbolSummary>> {
    let include_context = context_file.and_then(|file| c_include_context_for_file(file).ok());
    let mut summaries = Vec::with_capacity(semantic_paths.len());

    for semantic_path in semantic_paths {
        deadline.check("summarizing trace symbols")?;
        if let Some(summary) = selection::choose_symbol_summary(
            symbols,
            semantic_path,
            context_file,
            include_context.as_ref(),
        ) {
            summaries.push(summary);
        }
    }

    deadline.check("summarizing trace symbols")?;
    Ok(summaries)
}

pub(crate) fn symbol_summary_from_meta(symbol: &SymbolMeta) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: symbol.symbol_id.clone(),
        semantic_path: symbol.semantic_path.clone(),
        scope_path: symbol.scope_path.clone(),
        file_path: symbol.file_path.clone(),
        node_kind: symbol.node_kind.clone(),
        origin_type: symbol.origin_type.clone(),
        byte_range: symbol.byte_range,
        signature: symbol.signature.clone(),
        parameters: symbol.parameters.clone(),
        return_type: symbol.return_type.clone(),
        docstring: symbol.docstring.clone(),
    })
}

pub(crate) fn trace_evidence_keys(
    symbol: &SymbolMeta,
    callers: &[SymbolSummary],
    callees: &[SymbolSummary],
) -> TraceEvidenceKeys {
    TraceEvidenceKeys {
        symbol: symbol.evidence_key.clone(),
        callers: callers
            .iter()
            .map(|summary| summary.evidence_key.clone())
            .collect(),
        callees: callees
            .iter()
            .map(|summary| summary.evidence_key.clone())
            .collect(),
    }
}
