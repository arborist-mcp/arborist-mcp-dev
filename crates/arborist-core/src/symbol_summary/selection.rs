use crate::diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticsSink};
use crate::model::{SymbolMeta, SymbolSummary, SymbolSummaryInit};
use crate::symbol_dependency::CIncludeContext;
use crate::symbol_index_model::symbol_kind_rank;

pub(super) fn choose_symbol_summary(
    symbols: &[SymbolMeta],
    symbol_id: &str,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
    diagnostics: Option<&mut DiagnosticsSink>,
) -> Option<SymbolSummary> {
    let Some(best) = symbols
        .iter()
        .filter(|symbol| symbol.symbol_id == symbol_id)
        .max_by(|left, right| {
            symbol_candidate_rank(left, context_file, include_context)
                .cmp(&symbol_candidate_rank(right, context_file, include_context))
                .then_with(|| right.file_path.cmp(&left.file_path))
                .then_with(|| right.byte_range.cmp(&left.byte_range))
                .then_with(|| right.symbol_id.cmp(&left.symbol_id))
        })
    else {
        record_reference_diagnostic(
            diagnostics,
            DiagnosticCategory::UnresolvedReference,
            "no indexed symbol matches reference",
            symbol_id,
            context_file,
        );
        return None;
    };

    if diagnostics.is_some() {
        let best_rank = symbol_candidate_rank(best, context_file, include_context);
        let tied_for_best = symbols
            .iter()
            .filter(|symbol| symbol.symbol_id == symbol_id)
            .filter(|symbol| {
                symbol_candidate_rank(symbol, context_file, include_context) == best_rank
            })
            .count();
        if tied_for_best > 1 {
            record_reference_diagnostic(
                diagnostics,
                DiagnosticCategory::AmbiguousReference,
                "reference matched multiple indexed symbols",
                symbol_id,
                context_file,
            );
        }
    }

    Some(SymbolSummary::new(SymbolSummaryInit {
        symbol_id: best.symbol_id.clone(),
        semantic_path: best.semantic_path.clone(),
        scope_path: best.scope_path.clone(),
        file_path: best.file_path.clone(),
        node_kind: best.node_kind.clone(),
        origin_type: symbol_origin_type(best, context_file, include_context).to_string(),
        byte_range: best.byte_range,
        signature: best.signature.clone(),
        parameters: best.parameters.clone(),
        return_type: best.return_type.clone(),
        docstring: best.docstring.clone(),
    }))
}

fn record_reference_diagnostic(
    diagnostics: Option<&mut DiagnosticsSink>,
    category: DiagnosticCategory,
    message: &str,
    symbol_id: &str,
    context_file: Option<&str>,
) {
    if let Some(sink) = diagnostics {
        sink.record(Diagnostic {
            category,
            message: message.to_string(),
            semantic_path: Some(symbol_id.to_string()),
            context_file: context_file.map(str::to_string),
            language_id: None,
        });
    }
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
