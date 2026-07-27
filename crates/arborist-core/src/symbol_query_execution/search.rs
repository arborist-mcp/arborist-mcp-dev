use std::collections::BTreeMap;

use anyhow::{Result, anyhow};

use super::read::{
    read_symbol_neighborhood_context_from_meta_with_timeout,
    read_symbol_neighborhood_context_from_symbols,
};
use crate::model::{
    SymbolMeta, SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
    SymbolSearchNeighborhoodContextResult, SymbolSearchResult, TraceDirection,
};
use crate::symbol_map::resolved_symbol_map;
use crate::symbol_query::validate_symbol_limit;
use crate::symbol_read::read_symbol_result_from_meta;
use crate::symbol_search::{
    normalize_optional_search_filter, search_match_detail, symbol_matches_search_filters,
};
use crate::symbol_summary::symbol_summary_from_meta;
use crate::symbol_trace::TraceQueryDeadline;

pub(crate) fn search_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchResult> {
    search_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        None,
    )
}

pub(crate) fn search_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolSearchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    validate_symbol_limit(limit)?;
    let query = query.trim();
    if query.is_empty() {
        return Err(anyhow!("query must not be blank"));
    }
    let file_path_contains =
        normalize_optional_search_filter(file_path_contains, "file_path_contains")?;
    let node_kind = normalize_optional_search_filter(node_kind, "node_kind")?;

    let normalized_query = query.to_lowercase();
    let mut ranked_matches = Vec::new();
    for symbol in resolved_symbols {
        deadline.check("symbol search")?;
        if !symbol_matches_search_filters(
            symbol,
            file_path_contains.as_deref(),
            node_kind.as_deref(),
        ) {
            continue;
        }
        if let Some(detail) = search_match_detail(symbol, query, &normalized_query) {
            ranked_matches.push((detail, symbol));
        }
    }
    deadline.check("symbol search ranking")?;
    ranked_matches.sort_by(|left, right| {
        right
            .0
            .score
            .cmp(&left.0.score)
            .then_with(|| left.1.semantic_path.cmp(&right.1.semantic_path))
            .then_with(|| left.1.file_path.cmp(&right.1.file_path))
            .then_with(|| left.1.byte_range.cmp(&right.1.byte_range))
    });

    let total_matches = ranked_matches.len();
    let limited_matches = ranked_matches
        .into_iter()
        .take(limit)
        .map(|(detail, symbol)| (symbol_summary_from_meta(symbol), detail))
        .collect::<Vec<_>>();
    let truncated = total_matches > limited_matches.len();
    let match_details = limited_matches
        .iter()
        .map(|(_, detail)| detail.clone())
        .collect::<Vec<_>>();
    let matches = limited_matches
        .into_iter()
        .map(|(summary, _)| summary)
        .collect::<Vec<_>>();
    let result = SymbolSearchResult {
        query: query.to_string(),
        indexed_files,
        total_matches,
        truncated,
        matches,
        match_details,
    };
    deadline.check("symbol search result")?;
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn search_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolSearchContextResult> {
    search_context_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        file_overrides,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn search_context_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolSearchContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("symbol search")?;
    let search = search_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        timeout_ms,
    )?;
    let resolved_map = resolved_symbol_map(resolved_symbols);
    let mut reads = Vec::with_capacity(search.matches.len());

    for symbol in &search.matches {
        deadline.check("symbol search context")?;
        let meta = resolved_map.get(&symbol.symbol_id).ok_or_else(|| {
            anyhow!(
                "symbol not found in workspace index while reading search match: {}",
                symbol.symbol_id
            )
        })?;
        reads.push(read_symbol_result_from_meta(
            meta,
            indexed_files,
            file_overrides,
        )?);
    }

    deadline.check("symbol search context result")?;
    let result = SymbolSearchContextResult { search, reads };
    result.validate_public_output()?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn search_discovery_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolSearchDiscoveryContextResult> {
    let search = search_from_symbols(
        resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
    )?;
    let resolved_map = resolved_symbol_map(resolved_symbols);
    let mut reads = Vec::with_capacity(search.matches.len());
    let mut contexts = Vec::with_capacity(search.matches.len());

    for symbol in &search.matches {
        let meta = resolved_map.get(&symbol.symbol_id).ok_or_else(|| {
            anyhow!(
                "symbol not found in workspace index while reading search match: {}",
                symbol.symbol_id
            )
        })?;
        reads.push(read_symbol_result_from_meta(
            meta,
            indexed_files,
            file_overrides,
        )?);
        contexts.push(read_symbol_neighborhood_context_from_symbols(
            resolved_symbols,
            indexed_files,
            &symbol.symbol_id,
            direction,
            max_depth,
            max_nodes,
            file_overrides,
        )?);
    }

    let result = SymbolSearchDiscoveryContextResult {
        search,
        reads,
        contexts,
    };
    result.validate_public_output()?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn search_neighborhood_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolSearchNeighborhoodContextResult> {
    search_neighborhood_context_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
        query,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        file_overrides,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn search_neighborhood_context_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolSearchNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("symbol search")?;
    let search = search_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        timeout_ms,
    )?;
    let mut contexts = Vec::with_capacity(search.matches.len());

    for symbol in &search.matches {
        deadline.check("search neighborhood contexts")?;
        let meta = resolved_symbols
            .iter()
            .find(|candidate| candidate.symbol_id == symbol.symbol_id)
            .ok_or_else(|| {
                anyhow!(
                    "symbol not found in workspace index while reading search match: {}",
                    symbol.symbol_id
                )
            })?;
        let timeout_ms = deadline.remaining_timeout_ms("search neighborhood context")?;
        contexts.push(read_symbol_neighborhood_context_from_meta_with_timeout(
            resolved_symbols,
            indexed_files,
            meta,
            direction,
            max_depth,
            max_nodes,
            file_overrides,
            timeout_ms,
        )?);
    }

    let result = SymbolSearchNeighborhoodContextResult { search, contexts };
    deadline.check("search neighborhood context result")?;
    result.validate_public_output()?;
    Ok(result)
}
