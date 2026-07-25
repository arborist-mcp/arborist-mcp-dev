use std::path::Path;

use super::super::c::CIncludeContext;
use crate::language::detect_language;
use crate::model::LanguageId;
use crate::symbol_index_model::{IndexedSymbol, symbol_kind_rank};

pub(crate) fn indexed_symbol_rank(symbol: &IndexedSymbol) -> usize {
    symbol_kind_rank(&symbol.node_kind)
}

pub(crate) fn indexed_symbol_candidate_rank(
    symbol: &IndexedSymbol,
    source_symbol: &IndexedSymbol,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> usize {
    let mut rank = indexed_symbol_rank(symbol);

    if let Some(context_file) = context_file {
        if symbol.file_path == context_file {
            rank += 1000;
        } else if symbol.semantic_path.contains("::") {
            rank = rank.saturating_sub(100);
        }
    }

    if source_symbol_scope_matches(source_symbol, symbol) {
        rank += 500;
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

fn source_symbol_scope_matches(source_symbol: &IndexedSymbol, candidate: &IndexedSymbol) -> bool {
    detect_language(Path::new(&source_symbol.file_path)).ok() == Some(LanguageId::Cpp)
        && source_symbol.scope_path.is_some()
        && source_symbol.scope_path == candidate.scope_path
}
