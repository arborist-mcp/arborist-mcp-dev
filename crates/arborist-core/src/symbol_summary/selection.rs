use crate::model::{SymbolMeta, SymbolSummary, SymbolSummaryInit};
use crate::symbol_dependency::CIncludeContext;
use crate::symbol_index_model::symbol_kind_rank;

pub(super) fn choose_symbol_summary(
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
