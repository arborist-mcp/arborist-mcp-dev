use crate::model::{SymbolMeta, SymbolSummary, SymbolSummaryInit, TraceEvidenceKeys};
use crate::symbol_dependency::{CIncludeContext, c_include_context_for_file};
use crate::symbol_index_model::symbol_kind_rank;

pub(crate) fn summarize_symbols(
    symbols: &[SymbolMeta],
    semantic_paths: &[String],
    context_file: Option<&str>,
) -> Vec<SymbolSummary> {
    let include_context = context_file.and_then(|file| c_include_context_for_file(file).ok());
    semantic_paths
        .iter()
        .filter_map(|semantic_path| {
            choose_symbol_summary(
                symbols,
                semantic_path,
                context_file,
                include_context.as_ref(),
            )
        })
        .collect()
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

fn choose_symbol_summary(
    symbols: &[SymbolMeta],
    symbol_id: &str,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> Option<SymbolSummary> {
    symbols
        .iter()
        .filter(|symbol| symbol.symbol_id == symbol_id)
        .max_by_key(|symbol| symbol_candidate_rank(symbol, context_file, include_context))
        .map(|symbol| {
            SymbolSummary::new(SymbolSummaryInit {
                symbol_id: symbol.symbol_id.clone(),
                semantic_path: symbol.semantic_path.clone(),
                scope_path: symbol.scope_path.clone(),
                file_path: symbol.file_path.clone(),
                node_kind: symbol.node_kind.clone(),
                origin_type: symbol_origin_type(symbol, context_file, include_context).to_string(),
                byte_range: symbol.byte_range,
                signature: symbol.signature.clone(),
                parameters: symbol.parameters.clone(),
                return_type: symbol.return_type.clone(),
                docstring: symbol.docstring.clone(),
            })
        })
}

fn symbol_origin_type(
    symbol: &SymbolMeta,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> &'static str {
    if context_file.is_some_and(|context_file| symbol.file_path == context_file) {
        return "local_file";
    }

    if include_context.is_some_and(|include_context| {
        include_context
            .companion_source_paths
            .contains(&symbol.file_path)
    }) {
        return "companion_source";
    }

    if include_context
        .is_some_and(|include_context| include_context.include_paths.contains(&symbol.file_path))
    {
        return "include_header";
    }

    "workspace_symbol"
}

fn symbol_candidate_rank(
    symbol: &SymbolMeta,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> usize {
    let mut rank = symbol_kind_rank(&symbol.node_kind);

    if let Some(context_file) = context_file {
        if symbol.file_path == context_file {
            rank += 1000;
        } else if symbol.semantic_path.contains("::") {
            rank = rank.saturating_sub(100);
        }
    }

    if let Some(include_context) = include_context {
        if include_context.include_paths.contains(&symbol.file_path) {
            rank += 200;
        }
        if include_context
            .companion_source_paths
            .contains(&symbol.file_path)
        {
            rank += 300;
        }
    }

    rank
}
