use std::collections::BTreeMap;

use anyhow::{Result, anyhow};

use crate::model::{
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, SymbolMeta, SymbolReadResult, SymbolSearchContextResult,
    SymbolSearchDiscoveryContextResult, SymbolSearchNeighborhoodContextResult, SymbolSearchResult,
    TraceDirection,
};
use crate::symbol_index_model::symbol_kind_rank;
use crate::symbol_map::resolved_symbol_map;
use crate::symbol_read::read_symbol_result_from_meta;
use crate::symbol_search::{
    normalize_optional_search_filter, search_match_detail, symbol_matches_search_filters,
};
use crate::symbol_summary::symbol_summary_from_meta;

mod read;
mod trace;

pub(crate) use read::{
    read_symbol_at_position_from_symbols, read_symbol_context_at_position_from_symbols,
    read_symbol_context_from_symbols, read_symbol_discovery_context_at_position_from_symbols,
    read_symbol_discovery_context_from_symbols, read_symbol_from_symbols,
    read_symbol_neighborhood_context_at_position_from_symbols,
    read_symbol_neighborhood_context_from_symbols,
};
pub(crate) use trace::{
    trace_from_symbols, trace_neighborhood_from_symbols,
    trace_symbol_graph_at_position_from_symbols,
    trace_symbol_neighborhood_at_position_from_symbols,
};

pub(crate) fn read_symbol_from_meta(
    symbol: &SymbolMeta,
    indexed_files: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadResult> {
    read_symbol_result_from_meta(symbol, indexed_files, file_overrides)
}

pub(crate) fn search_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchResult> {
    let query = query.trim();
    if query.is_empty() {
        return Err(anyhow!("query must not be blank"));
    }
    let file_path_contains =
        normalize_optional_search_filter(file_path_contains, "file_path_contains")?;
    let node_kind = normalize_optional_search_filter(node_kind, "node_kind")?;

    let normalized_query = query.to_ascii_lowercase();
    let mut ranked_matches = resolved_symbols
        .iter()
        .filter_map(|symbol| {
            if !symbol_matches_search_filters(
                symbol,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ) {
                return None;
            }
            let detail = search_match_detail(symbol, query, &normalized_query)?;
            Some((detail, symbol))
        })
        .collect::<Vec<_>>();
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
    }

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
    let search = search_from_symbols(
        resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
    )?;
    let mut contexts = Vec::with_capacity(search.matches.len());

    for symbol in &search.matches {
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

    let result = SymbolSearchNeighborhoodContextResult { search, contexts };
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn list_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    let file_path_contains =
        normalize_optional_search_filter(file_path_contains, "file_path_contains")?;
    let node_kind = normalize_optional_search_filter(node_kind, "node_kind")?;

    let mut symbols = resolved_symbols
        .iter()
        .filter(|symbol| {
            symbol_matches_search_filters(
                symbol,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            )
        })
        .map(symbol_summary_from_meta)
        .collect::<Vec<_>>();
    symbols.sort_by(|left, right| {
        left.file_path
            .cmp(&right.file_path)
            .then_with(|| left.semantic_path.cmp(&right.semantic_path))
            .then_with(|| left.byte_range.cmp(&right.byte_range))
            .then_with(|| left.symbol_id.cmp(&right.symbol_id))
    });

    let total_symbols = symbols.len();
    symbols.truncate(limit);
    let result = SymbolListResult {
        indexed_files,
        total_symbols,
        truncated: total_symbols > symbols.len(),
        symbols,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn list_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolListContextResult> {
    let list = list_from_symbols(
        resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
    )?;
    let resolved_map = resolved_symbol_map(resolved_symbols);
    let mut reads = Vec::with_capacity(list.symbols.len());

    for symbol in &list.symbols {
        let meta = resolved_map.get(&symbol.symbol_id).ok_or_else(|| {
            anyhow!(
                "symbol not found in workspace index while reading listed symbol: {}",
                symbol.symbol_id
            )
        })?;
        reads.push(read_symbol_result_from_meta(
            meta,
            indexed_files,
            file_overrides,
        )?);
    }

    let result = SymbolListContextResult { list, reads };
    result.validate_public_output()?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn list_discovery_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolListDiscoveryContextResult> {
    let list = list_from_symbols(
        resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
    )?;
    let resolved_map = resolved_symbol_map(resolved_symbols);
    let mut reads = Vec::with_capacity(list.symbols.len());
    let mut contexts = Vec::with_capacity(list.symbols.len());

    for symbol in &list.symbols {
        let meta = resolved_map.get(&symbol.symbol_id).ok_or_else(|| {
            anyhow!(
                "symbol not found in workspace index while reading listed symbol: {}",
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

    let result = SymbolListDiscoveryContextResult {
        list,
        reads,
        contexts,
    };
    result.validate_public_output()?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn list_neighborhood_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolListNeighborhoodContextResult> {
    let list = list_from_symbols(
        resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
    )?;
    let mut contexts = Vec::with_capacity(list.symbols.len());

    for symbol in &list.symbols {
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

    let result = SymbolListNeighborhoodContextResult { list, contexts };
    result.validate_public_output()?;
    Ok(result)
}

fn validate_trace_symbol_path(symbol_path: &str) -> Result<()> {
    if symbol_path.trim().is_empty() {
        return Err(anyhow!("invalid symbol_path: selector must not be blank"));
    }

    Ok(())
}

fn choose_trace_symbol<'a>(symbols: &'a [SymbolMeta], symbol_path: &str) -> Option<&'a SymbolMeta> {
    symbols
        .iter()
        .filter(|symbol| symbol.symbol_id == symbol_path || symbol.semantic_path == symbol_path)
        .max_by_key(|symbol| symbol_kind_rank(&symbol.node_kind))
}
