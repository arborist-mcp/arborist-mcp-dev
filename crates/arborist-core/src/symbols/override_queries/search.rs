use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::model::{
    SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
    SymbolSearchNeighborhoodContextResult, SymbolSearchResult, TraceDirection,
};
use crate::symbol_index_workspace::resolve_workspace_symbols_with_overrides;
use crate::symbol_query_execution::{
    search_context_from_symbols, search_context_from_symbols_with_timeout,
    search_discovery_context_from_symbols, search_from_symbols, search_from_symbols_with_timeout,
    search_neighborhood_context_from_symbols,
};
use crate::symbol_trace::TraceQueryDeadline;

use super::load_normalized_symbol_index_with_overrides;
use super::load_normalized_symbol_index_with_overrides_with_timeout;
use crate::symbol_index_workspace::resolve_workspace_symbols_with_overrides_with_timeout;

pub fn search_symbols_with_overrides_filtered(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    search_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
    )
}

pub fn search_symbols_with_overrides_filtered_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolSearchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols_with_overrides_with_timeout(
        workspace_root,
        file_overrides,
        timeout_ms,
    )?;
    deadline.check("workspace symbol search")?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol search")?;
    search_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        timeout_ms,
    )
}

pub fn search_symbols_from_index_with_overrides_filtered_with_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolSearchResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    deadline.check("index symbol search")?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol search")?;
    search_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        timeout_ms,
    )
}

pub fn search_symbols_context_with_overrides_filtered_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolSearchContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols_with_overrides_with_timeout(
        workspace_root,
        file_overrides,
        timeout_ms,
    )?;
    deadline.check("workspace symbol search")?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol search")?;
    search_context_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        Some(file_overrides),
        timeout_ms,
    )
}

pub fn search_symbols_context_from_index_with_overrides_filtered_with_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolSearchContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    deadline.check("index symbol search")?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol search")?;
    search_context_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        Some(file_overrides),
        timeout_ms,
    )
}

pub fn search_symbols_context_with_overrides_filtered(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchContextResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    search_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        Some(file_overrides),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_discovery_context_with_overrides_filtered(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchDiscoveryContextResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    search_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        Some(file_overrides),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_neighborhood_context_with_overrides_filtered(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchNeighborhoodContextResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    search_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        Some(file_overrides),
    )
}

pub fn search_symbols_from_index_with_overrides_filtered(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchResult> {
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
    search_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
    )
}

pub fn search_symbols_context_from_index_with_overrides_filtered(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchContextResult> {
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
    search_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        Some(file_overrides),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_neighborhood_context_from_index_with_overrides_filtered(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchNeighborhoodContextResult> {
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
    search_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        Some(file_overrides),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_discovery_context_from_index_with_overrides_filtered(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchDiscoveryContextResult> {
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
    search_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        Some(file_overrides),
    )
}
