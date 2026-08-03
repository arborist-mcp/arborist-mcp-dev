use std::collections::BTreeMap;

use anyhow::{Result, anyhow};

use super::read::read_symbol_neighborhood_context_from_meta_with_deadline_and_cache;
use crate::model::{
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, SymbolMeta, TraceDirection,
};
use crate::symbol_map::resolved_symbol_ref_map_with_deadline;
use crate::symbol_query::validate_symbol_limit;
use crate::symbol_read::read_symbol_result_from_meta_with_cache;
use crate::symbol_search::{normalize_optional_search_filter, symbol_matches_search_filters};
use crate::symbol_summary::symbol_summary_from_meta;
use crate::symbol_trace::TraceQueryDeadline;

pub(crate) fn list_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    list_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        None,
    )
}

pub(crate) fn list_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    list_from_symbols_with_deadline(
        resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        &deadline,
    )
}

pub(crate) fn list_from_symbols_with_deadline(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolListResult> {
    validate_symbol_limit(limit)?;
    let file_path_contains =
        normalize_optional_search_filter(file_path_contains, "file_path_contains")?;
    let node_kind = normalize_optional_search_filter(node_kind, "node_kind")?;

    let mut symbols = Vec::new();
    for symbol in resolved_symbols {
        deadline.check("symbol listing")?;
        if symbol_matches_search_filters(
            symbol,
            file_path_contains.as_deref(),
            node_kind.as_deref(),
        ) {
            symbols.push(symbol_summary_from_meta(symbol));
        }
    }
    deadline.check("symbol listing sort")?;
    symbols.sort_by(|left, right| {
        left.file_path
            .cmp(&right.file_path)
            .then_with(|| left.semantic_path.cmp(&right.semantic_path))
            .then_with(|| left.byte_range.cmp(&right.byte_range))
            .then_with(|| left.symbol_id.cmp(&right.symbol_id))
    });
    deadline.check("symbol listing result")?;

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
    list_context_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        file_overrides,
        None,
    )
}

pub(crate) fn list_context_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    list_context_from_symbols_with_deadline(
        resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        file_overrides,
        &deadline,
    )
}

pub(crate) fn list_context_from_symbols_with_deadline(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolListContextResult> {
    let list = list_from_symbols_with_deadline(
        resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        deadline,
    )?;
    let resolved_map = resolved_symbol_ref_map_with_deadline(resolved_symbols, Some(deadline))?;
    let mut reads = Vec::with_capacity(list.symbols.len());
    let mut source_cache = BTreeMap::new();

    for symbol in &list.symbols {
        deadline.check("symbol listing context reads")?;
        let meta = resolved_map.get(symbol.symbol_id.as_str()).ok_or_else(|| {
            anyhow!(
                "symbol not found in workspace index while reading listed symbol: {}",
                symbol.symbol_id
            )
        })?;
        reads.push(read_symbol_result_from_meta_with_cache(
            meta,
            indexed_files,
            file_overrides,
            &mut source_cache,
        )?);
    }

    let result = SymbolListContextResult { list, reads };
    deadline.check("symbol listing context result")?;
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
    list_discovery_context_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
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
pub(crate) fn list_discovery_context_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListDiscoveryContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    list_discovery_context_from_symbols_with_deadline(
        resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        file_overrides,
        &deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn list_discovery_context_from_symbols_with_deadline(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolListDiscoveryContextResult> {
    let list = list_from_symbols_with_deadline(
        resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        deadline,
    )?;
    let resolved_map = resolved_symbol_ref_map_with_deadline(resolved_symbols, Some(deadline))?;
    let mut reads = Vec::with_capacity(list.symbols.len());
    let mut contexts = Vec::with_capacity(list.symbols.len());
    let mut source_cache = BTreeMap::new();

    for symbol in &list.symbols {
        deadline.check("symbol discovery context reads")?;
        let meta = resolved_map.get(symbol.symbol_id.as_str()).ok_or_else(|| {
            anyhow!(
                "symbol not found in workspace index while reading listed symbol: {}",
                symbol.symbol_id
            )
        })?;
        reads.push(read_symbol_result_from_meta_with_cache(
            meta,
            indexed_files,
            file_overrides,
            &mut source_cache,
        )?);
        contexts.push(
            read_symbol_neighborhood_context_from_meta_with_deadline_and_cache(
                resolved_symbols,
                indexed_files,
                meta,
                direction,
                max_depth,
                max_nodes,
                file_overrides,
                deadline,
                &mut source_cache,
            )?,
        );
    }

    let result = SymbolListDiscoveryContextResult {
        list,
        reads,
        contexts,
    };
    deadline.check("symbol discovery context result")?;
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
    list_neighborhood_context_from_symbols_with_timeout(
        resolved_symbols,
        indexed_files,
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
pub(crate) fn list_neighborhood_context_from_symbols_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    list_neighborhood_context_from_symbols_with_deadline(
        resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        file_overrides,
        &deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn list_neighborhood_context_from_symbols_with_deadline(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolListNeighborhoodContextResult> {
    let list = list_from_symbols_with_deadline(
        resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        deadline,
    )?;
    let resolved_map = resolved_symbol_ref_map_with_deadline(resolved_symbols, Some(deadline))?;
    let mut contexts = Vec::with_capacity(list.symbols.len());
    let mut source_cache = BTreeMap::new();

    for symbol in &list.symbols {
        deadline.check("symbol neighborhood contexts")?;
        let meta = resolved_map.get(symbol.symbol_id.as_str()).ok_or_else(|| {
            anyhow!(
                "symbol not found in workspace index while reading listed symbol: {}",
                symbol.symbol_id
            )
        })?;
        contexts.push(
            read_symbol_neighborhood_context_from_meta_with_deadline_and_cache(
                resolved_symbols,
                indexed_files,
                meta,
                direction,
                max_depth,
                max_nodes,
                file_overrides,
                deadline,
                &mut source_cache,
            )?,
        );
    }

    let result = SymbolListNeighborhoodContextResult { list, contexts };
    deadline.check("symbol neighborhood context result")?;
    result.validate_public_output()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{list_context_from_symbols_with_deadline, list_from_symbols_with_deadline};
    use crate::symbol_trace::TraceQueryDeadline;

    #[test]
    fn list_context_reuses_the_callers_deadline() {
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let error = list_context_from_symbols_with_deadline(&[], 0, 1, None, None, None, &deadline)
            .expect_err("symbol listing context should honor an already-expired deadline");

        assert!(error.to_string().contains("symbol listing"));
    }

    #[test]
    fn list_reuses_the_callers_deadline() {
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let error = list_from_symbols_with_deadline(&[], 0, 1, None, None, &deadline)
            .expect_err("symbol listing should honor an already-expired deadline");

        assert!(error.to_string().contains("symbol listing"));
    }
}
