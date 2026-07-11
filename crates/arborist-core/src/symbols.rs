use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use rusqlite::Connection;

use crate::index_schema::{
    ensure_symbol_tables, load_indexed_files_metadata, require_symbol_index_tables,
    validate_symbol_index_schema_version, validate_symbol_index_workspace,
};
use crate::index_store::{
    SymbolRefreshPersistence, load_file_states, load_indexed_symbols_grouped_by_file,
    load_resolved_symbols, persist_symbol_index, persist_symbol_refresh,
};
use crate::language::{
    c_include_targets, c_local_include_targets, detect_language, ensure_path_inside_workspace,
    normalize_absolute_path, normalize_path, parse_document, path_is_inside_workspace, read_source,
    resolve_local_c_include,
};
use crate::model::{LanguageId, Position};
use crate::model::{
    SymbolContextResult, SymbolIndexStats, SymbolListContextResult,
    SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult, SymbolListResult,
    SymbolMeta, SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult,
    SymbolReadResult, SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
    SymbolSearchNeighborhoodContextResult, SymbolSearchResult, TraceDirection,
    TraceSymbolGraphResult, TraceSymbolNeighborhoodResult,
};
use crate::symbol_dependency::{
    assign_symbol_ids, materialize_resolved_symbol_rows, refresh_resolved_symbol_subgraph,
    resolve_symbol_dependencies,
};
use crate::symbol_extractor::index_symbols_from_document;
use crate::symbol_index_model::{IndexedSymbol, PersistedFileState, symbol_kind_rank};
use crate::symbol_index_state::{
    load_symbol_index, load_symbol_index_with_overrides, source_fingerprint,
};
use crate::symbol_map::resolved_symbol_map;
use crate::symbol_position::resolve_symbol_at_position;
use crate::symbol_read::read_symbol_result_from_meta;
use crate::symbol_search::{
    normalize_optional_search_filter, search_match_detail, symbol_matches_search_filters,
};
use crate::symbol_summary::symbol_summary_from_meta;
use crate::symbol_trace::{trace_from_symbol, trace_neighborhood_from_symbol};
use crate::workspace_scan::{
    WorkspaceScanLimits, collect_source_files, collect_source_files_with_limits,
    should_skip_index_path,
};

type IncrementalWorkspaceSymbols = (
    Vec<IndexedSymbol>,
    Vec<SymbolMeta>,
    Vec<PersistedFileState>,
    usize,
    usize,
    usize,
);

pub fn trace_symbol_graph(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    trace_from_symbols(&resolved_symbols, indexed_files, symbol_path, direction)
}

pub fn trace_symbol_neighborhood(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    trace_neighborhood_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
    )
}

pub fn trace_symbol_graph_at_position(
    workspace_root: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
    trace_symbol_graph_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        None,
    )
}

pub fn trace_symbol_neighborhood_at_position(
    workspace_root: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
    trace_symbol_neighborhood_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol(workspace_root: &Path, symbol_path: &str) -> Result<SymbolReadResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    read_symbol_from_symbols(&resolved_symbols, indexed_files, symbol_path, None)
}

pub fn read_symbol_context(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    read_symbol_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        None,
    )
}

pub fn read_symbol_neighborhood_context(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    read_symbol_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_discovery_context(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    read_symbol_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_at_position(
    workspace_root: &Path,
    file_path: &Path,
    position: &Position,
) -> Result<SymbolReadResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
    read_symbol_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        None,
    )
}

pub fn read_symbol_context_at_position(
    workspace_root: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
    read_symbol_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        None,
    )
}

pub fn read_symbol_neighborhood_context_at_position(
    workspace_root: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
    read_symbol_neighborhood_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_discovery_context_at_position(
    workspace_root: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
    read_symbol_discovery_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn search_symbols(
    workspace_root: &Path,
    query: &str,
    limit: usize,
) -> Result<SymbolSearchResult> {
    search_symbols_filtered(workspace_root, query, limit, None, None)
}

pub fn search_symbols_context(
    workspace_root: &Path,
    query: &str,
    limit: usize,
) -> Result<SymbolSearchContextResult> {
    search_symbols_context_filtered(workspace_root, query, limit, None, None)
}

pub fn search_symbols_discovery_context(
    workspace_root: &Path,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolSearchDiscoveryContextResult> {
    search_symbols_discovery_context_filtered(
        workspace_root,
        query,
        limit,
        direction,
        max_depth,
        max_nodes,
        None,
        None,
    )
}

pub fn search_symbols_neighborhood_context(
    workspace_root: &Path,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolSearchNeighborhoodContextResult> {
    search_symbols_neighborhood_context_filtered(
        workspace_root,
        query,
        limit,
        direction,
        max_depth,
        max_nodes,
        None,
        None,
    )
}

pub fn search_symbols_filtered(
    workspace_root: &Path,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    search_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_neighborhood_context_filtered(
    workspace_root: &Path,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchNeighborhoodContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
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
        None,
    )
}

pub fn search_symbols_context_filtered(
    workspace_root: &Path,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    search_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_discovery_context_filtered(
    workspace_root: &Path,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchDiscoveryContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
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
        None,
    )
}

pub fn list_symbols(workspace_root: &Path, limit: usize) -> Result<SymbolListResult> {
    list_symbols_filtered(workspace_root, limit, None, None)
}

pub fn list_symbols_context(
    workspace_root: &Path,
    limit: usize,
) -> Result<SymbolListContextResult> {
    list_symbols_context_filtered(workspace_root, limit, None, None)
}

pub fn list_symbols_discovery_context(
    workspace_root: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolListDiscoveryContextResult> {
    list_symbols_discovery_context_filtered(
        workspace_root,
        limit,
        direction,
        max_depth,
        max_nodes,
        None,
        None,
    )
}

pub fn list_symbols_neighborhood_context(
    workspace_root: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolListNeighborhoodContextResult> {
    list_symbols_neighborhood_context_filtered(
        workspace_root,
        limit,
        direction,
        max_depth,
        max_nodes,
        None,
        None,
    )
}

pub fn list_symbols_filtered(
    workspace_root: &Path,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    list_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
    )
}

pub fn list_symbols_context_filtered(
    workspace_root: &Path,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    list_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_discovery_context_filtered(
    workspace_root: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListDiscoveryContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    list_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_neighborhood_context_filtered(
    workspace_root: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListNeighborhoodContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    list_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        None,
    )
}

pub fn trace_symbol_graph_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    trace_from_symbols(&resolved_symbols, indexed_files, symbol_path, direction)
}

pub fn trace_symbol_neighborhood_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    trace_neighborhood_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
    )
}

pub fn trace_symbol_graph_at_position_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(&workspace_root, file_overrides)?;
    trace_symbol_graph_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        Some(file_overrides),
    )
}

pub fn trace_symbol_neighborhood_at_position_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(&workspace_root, file_overrides)?;
    trace_symbol_neighborhood_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
    )
}

pub fn read_symbol_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
) -> Result<SymbolReadResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    read_symbol_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        Some(file_overrides),
    )
}

pub fn read_symbol_context_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    read_symbol_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        Some(file_overrides),
    )
}

pub fn read_symbol_neighborhood_context_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    read_symbol_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
    )
}

pub fn read_symbol_discovery_context_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    read_symbol_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
    )
}

pub fn read_symbol_at_position_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
) -> Result<SymbolReadResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(&workspace_root, file_overrides)?;
    read_symbol_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        Some(file_overrides),
    )
}

pub fn read_symbol_context_at_position_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(&workspace_root, file_overrides)?;
    read_symbol_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        Some(file_overrides),
    )
}

pub fn read_symbol_neighborhood_context_at_position_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(&workspace_root, file_overrides)?;
    read_symbol_neighborhood_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
    )
}

pub fn read_symbol_discovery_context_at_position_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(&workspace_root, file_overrides)?;
    read_symbol_discovery_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
    )
}

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

pub fn list_symbols_with_overrides_filtered(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    list_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
    )
}

pub fn list_symbols_context_with_overrides_filtered(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListContextResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    list_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        Some(file_overrides),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_discovery_context_with_overrides_filtered(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListDiscoveryContextResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    list_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
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
pub fn list_symbols_neighborhood_context_with_overrides_filtered(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListNeighborhoodContextResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    list_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        Some(file_overrides),
    )
}

pub fn trace_symbol_graph_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    trace_from_symbols(&resolved_symbols, indexed_files, symbol_path, direction)
}

pub fn trace_symbol_neighborhood_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    trace_neighborhood_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
    )
}

pub fn trace_symbol_graph_at_position_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    trace_symbol_graph_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        Some(file_overrides),
    )
}

pub fn trace_symbol_neighborhood_at_position_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    trace_symbol_neighborhood_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
    )
}

pub fn read_symbol_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
) -> Result<SymbolReadResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    read_symbol_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        Some(file_overrides),
    )
}

pub fn read_symbol_context_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    read_symbol_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        Some(file_overrides),
    )
}

pub fn read_symbol_neighborhood_context_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    read_symbol_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
    )
}

pub fn read_symbol_discovery_context_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    read_symbol_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
    )
}

pub fn read_symbol_at_position_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
) -> Result<SymbolReadResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    read_symbol_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        Some(file_overrides),
    )
}

pub fn read_symbol_context_at_position_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    read_symbol_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        Some(file_overrides),
    )
}

pub fn read_symbol_neighborhood_context_at_position_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    read_symbol_neighborhood_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
    )
}

pub fn read_symbol_discovery_context_at_position_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    read_symbol_discovery_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
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
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
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
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
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
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
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
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
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

pub fn list_symbols_from_index_with_overrides_filtered(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    list_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
    )
}

pub fn list_symbols_context_from_index_with_overrides_filtered(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    list_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        Some(file_overrides),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_neighborhood_context_from_index_with_overrides_filtered(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListNeighborhoodContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    list_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
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
pub fn list_symbols_discovery_context_from_index_with_overrides_filtered(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListDiscoveryContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) =
        load_symbol_index_with_overrides(&db_path, file_overrides)?;
    list_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        Some(file_overrides),
    )
}

pub fn rebuild_symbol_index(workspace_root: &Path, db_path: &Path) -> Result<SymbolIndexStats> {
    rebuild_symbol_index_with_limits(workspace_root, db_path, WorkspaceScanLimits::default())
}

pub fn rebuild_symbol_index_with_limits(
    workspace_root: &Path,
    db_path: &Path,
    limits: WorkspaceScanLimits,
) -> Result<SymbolIndexStats> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let db_path = normalize_absolute_path(db_path)?;
    let (raw_symbols, resolved_symbols, file_states, indexed_files, rebuilt_files, reused_files) =
        resolve_workspace_symbols_incremental_with_limits(&workspace_root, &db_path, limits)?;
    persist_symbol_index(
        &db_path,
        &workspace_root,
        &raw_symbols,
        &resolved_symbols,
        &file_states,
        indexed_files,
    )?;

    let result = SymbolIndexStats {
        db_path: normalize_path(&db_path),
        indexed_files,
        indexed_symbols: resolved_symbols.len(),
        rebuilt_files,
        reused_files,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub fn trace_symbol_graph_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    trace_from_symbols(&resolved_symbols, indexed_files, symbol_path, direction)
}

pub fn trace_symbol_neighborhood_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    trace_neighborhood_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
    )
}

pub fn trace_symbol_graph_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    trace_symbol_graph_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        None,
    )
}

pub fn trace_symbol_neighborhood_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    trace_symbol_neighborhood_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_from_index(db_path: &Path, symbol_path: &str) -> Result<SymbolReadResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    read_symbol_from_symbols(&resolved_symbols, indexed_files, symbol_path, None)
}

pub fn read_symbol_context_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    read_symbol_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        None,
    )
}

pub fn read_symbol_neighborhood_context_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    read_symbol_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_discovery_context_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    read_symbol_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
) -> Result<SymbolReadResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    read_symbol_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        None,
    )
}

pub fn read_symbol_context_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    read_symbol_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        None,
    )
}

pub fn read_symbol_neighborhood_context_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    read_symbol_neighborhood_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn read_symbol_discovery_context_at_position_from_index(
    db_path: &Path,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    read_symbol_discovery_context_at_position_from_symbols(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn search_symbols_from_index(
    db_path: &Path,
    query: &str,
    limit: usize,
) -> Result<SymbolSearchResult> {
    search_symbols_from_index_filtered(db_path, query, limit, None, None)
}

pub fn search_symbols_context_from_index(
    db_path: &Path,
    query: &str,
    limit: usize,
) -> Result<SymbolSearchContextResult> {
    search_symbols_context_from_index_filtered(db_path, query, limit, None, None)
}

pub fn search_symbols_discovery_context_from_index(
    db_path: &Path,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolSearchDiscoveryContextResult> {
    search_symbols_discovery_context_from_index_filtered(
        db_path, query, limit, direction, max_depth, max_nodes, None, None,
    )
}

pub fn search_symbols_neighborhood_context_from_index(
    db_path: &Path,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolSearchNeighborhoodContextResult> {
    search_symbols_neighborhood_context_from_index_filtered(
        db_path, query, limit, direction, max_depth, max_nodes, None, None,
    )
}

pub fn search_symbols_from_index_filtered(
    db_path: &Path,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    search_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_neighborhood_context_from_index_filtered(
    db_path: &Path,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchNeighborhoodContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
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
        None,
    )
}

pub fn search_symbols_context_from_index_filtered(
    db_path: &Path,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    search_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        query,
        limit,
        file_path_contains,
        node_kind,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_discovery_context_from_index_filtered(
    db_path: &Path,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchDiscoveryContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
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
        None,
    )
}

pub fn list_symbols_from_index(db_path: &Path, limit: usize) -> Result<SymbolListResult> {
    list_symbols_from_index_filtered(db_path, limit, None, None)
}

pub fn list_symbols_context_from_index(
    db_path: &Path,
    limit: usize,
) -> Result<SymbolListContextResult> {
    list_symbols_context_from_index_filtered(db_path, limit, None, None)
}

pub fn list_symbols_discovery_context_from_index(
    db_path: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolListDiscoveryContextResult> {
    list_symbols_discovery_context_from_index_filtered(
        db_path, limit, direction, max_depth, max_nodes, None, None,
    )
}

pub fn list_symbols_neighborhood_context_from_index(
    db_path: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolListNeighborhoodContextResult> {
    list_symbols_neighborhood_context_from_index_filtered(
        db_path, limit, direction, max_depth, max_nodes, None, None,
    )
}

pub fn list_symbols_from_index_filtered(
    db_path: &Path,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    list_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
    )
}

pub fn list_symbols_context_from_index_filtered(
    db_path: &Path,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    list_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_discovery_context_from_index_filtered(
    db_path: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListDiscoveryContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    list_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_neighborhood_context_from_index_filtered(
    db_path: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListNeighborhoodContextResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    list_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        None,
    )
}

pub fn refresh_symbol_index_for_file(
    workspace_root: &Path,
    db_path: &Path,
    file_path: &Path,
) -> Result<SymbolIndexStats> {
    refresh_symbol_index_for_file_with_limits(
        workspace_root,
        db_path,
        file_path,
        WorkspaceScanLimits::default(),
    )
}

pub fn refresh_symbol_index_for_file_with_limits(
    workspace_root: &Path,
    db_path: &Path,
    file_path: &Path,
    limits: WorkspaceScanLimits,
) -> Result<SymbolIndexStats> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;

    ensure_path_inside_workspace(&workspace_root, &file_path)?;

    if !db_path.exists() {
        return rebuild_symbol_index_with_limits(&workspace_root, &db_path, limits);
    }

    let connection = Connection::open(&db_path)?;
    require_symbol_index_tables(&connection, &db_path)?;
    validate_symbol_index_workspace(&connection, &workspace_root, &db_path)?;
    load_indexed_files_metadata(&connection)?;
    validate_symbol_index_schema_version(&connection, &db_path)?;
    ensure_symbol_tables(&connection)?;

    let old_resolved_symbols = load_resolved_symbols(&connection)?.0;
    let old_resolved_map = resolved_symbol_map(&old_resolved_symbols);
    let mut grouped_symbols = load_indexed_symbols_grouped_by_file(&connection)?;
    let refresh_paths = if should_skip_index_path(&workspace_root, &file_path) {
        vec![file_path.clone()]
    } else {
        expanded_refresh_file_paths(&workspace_root, &file_path)?
    };

    let mut file_states = load_file_states(&connection)?;
    let mut old_changed_symbols = Vec::new();
    let mut changed_file_paths = BTreeSet::new();
    let mut rebuilt_files = 0;

    for refresh_path in &refresh_paths {
        let normalized_refresh_path = normalize_path(refresh_path);
        let skip_refresh_path = should_skip_index_path(&workspace_root, refresh_path);
        let had_indexed_state = file_states.contains_key(&normalized_refresh_path)
            || grouped_symbols.contains_key(&normalized_refresh_path);
        old_changed_symbols.extend(
            grouped_symbols
                .get(&normalized_refresh_path)
                .cloned()
                .unwrap_or_default(),
        );

        if refresh_path.exists() && !skip_refresh_path {
            let source = read_source(refresh_path)?;
            let document = parse_document(refresh_path, &source)?;
            let fresh_symbols = index_symbols_from_document(refresh_path, &source, &document)?;

            file_states.insert(normalized_refresh_path.clone(), source_fingerprint(&source));
            grouped_symbols.insert(normalized_refresh_path.clone(), fresh_symbols);
            rebuilt_files += 1;
        } else {
            file_states.remove(&normalized_refresh_path);
            grouped_symbols.remove(&normalized_refresh_path);
            if had_indexed_state {
                rebuilt_files += 1;
            }
        }
        changed_file_paths.insert(normalized_refresh_path);
    }

    let mut raw_symbols = grouped_symbols
        .into_values()
        .flat_map(|symbols| symbols.into_iter())
        .collect::<Vec<_>>();
    assign_symbol_ids(&mut raw_symbols)?;
    let new_changed_symbols = raw_symbols
        .iter()
        .filter(|symbol| changed_file_paths.contains(&symbol.file_path))
        .cloned()
        .collect::<Vec<_>>();
    let (resolved_map, impacted_paths) = refresh_resolved_symbol_subgraph(
        &raw_symbols,
        &old_resolved_map,
        &old_changed_symbols,
        &new_changed_symbols,
        &changed_file_paths,
    );
    let resolved_symbols = materialize_resolved_symbol_rows(&raw_symbols, &resolved_map);
    let indexed_files = file_states.len();
    let reused_files = indexed_files.saturating_sub(rebuilt_files);

    persist_symbol_refresh(SymbolRefreshPersistence {
        db_path: &db_path,
        workspace_root: &workspace_root,
        raw_symbols: &raw_symbols,
        symbols: &resolved_symbols,
        resolved_symbols_by_id: &resolved_map,
        file_states: &file_states,
        changed_file_paths: &changed_file_paths,
        impacted_paths: &impacted_paths,
        indexed_files,
    })?;

    let result = SymbolIndexStats {
        db_path: normalize_path(&db_path),
        indexed_files,
        indexed_symbols: resolved_symbols.len(),
        rebuilt_files,
        reused_files,
    };
    result.validate_public_output()?;
    Ok(result)
}

fn expanded_refresh_file_paths(workspace_root: &Path, file_path: &Path) -> Result<Vec<PathBuf>> {
    let mut refresh_paths = BTreeSet::new();
    refresh_paths.insert(file_path.to_path_buf());

    if matches!(detect_language(file_path)?, LanguageId::C) {
        refresh_paths.extend(transitive_c_include_dependents(workspace_root, file_path)?);
    }

    Ok(refresh_paths.into_iter().collect())
}

fn transitive_c_include_dependents(
    workspace_root: &Path,
    target_path: &Path,
) -> Result<BTreeSet<PathBuf>> {
    let reverse_index = reverse_local_c_include_index(workspace_root)?;
    let normalized_target = normalize_path(target_path);
    let mut queue = vec![normalized_target.clone()];
    let mut visited = BTreeSet::from([normalized_target]);
    let mut dependents = BTreeSet::new();

    while let Some(current_path) = queue.pop() {
        let Some(children) = reverse_index.get(&current_path) else {
            continue;
        };

        for dependent_path in children {
            let normalized_dependent = normalize_path(dependent_path);
            if visited.insert(normalized_dependent.clone()) {
                dependents.insert(dependent_path.clone());
                queue.push(normalized_dependent);
            }
        }
    }

    Ok(dependents)
}

fn reverse_local_c_include_index(
    workspace_root: &Path,
) -> Result<BTreeMap<String, BTreeSet<PathBuf>>> {
    let mut reverse_index = BTreeMap::new();

    for path in collect_source_files(workspace_root)? {
        if !matches!(detect_language(&path), Ok(LanguageId::C)) {
            continue;
        }

        let source = read_source(&path)?;
        let document = parse_document(&path, &source)?;
        let local_include_targets = c_local_include_targets(document.tree.root_node(), &source)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        for include_target in c_include_targets(document.tree.root_node(), &source)? {
            let Some(include_path) =
                resolve_local_c_include(&path, &include_target).or_else(|| {
                    local_include_targets
                        .contains(&include_target)
                        .then(|| unresolved_local_c_include_path(&path, &include_target))
                        .flatten()
                })
            else {
                continue;
            };
            if !path_is_inside_workspace(workspace_root, &include_path)? {
                continue;
            }

            reverse_index
                .entry(normalize_path(&include_path))
                .or_insert_with(BTreeSet::new)
                .insert(path.clone());
        }
    }

    Ok(reverse_index)
}

fn unresolved_local_c_include_path(current_path: &Path, include_target: &str) -> Option<PathBuf> {
    let parent = current_path.parent()?;
    normalize_absolute_path(&parent.join(include_target)).ok()
}

fn build_workspace_index(
    paths: &[PathBuf],
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();

    for path in paths {
        let normalized_path = normalize_path(path);
        let source = match file_overrides.and_then(|overrides| overrides.get(&normalized_path)) {
            Some(source) => source.clone(),
            None => read_source(path)?,
        };
        let document = parse_document(path, &source)?;
        symbols.extend(index_symbols_from_document(path, &source, &document)?);
    }

    assign_symbol_ids(&mut symbols)?;
    Ok(symbols)
}

fn load_live_workspace_symbols(workspace_root: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    resolve_workspace_symbols(&workspace_root)
}

fn resolve_workspace_symbols(workspace_root: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    let indexed_paths = collect_source_files(workspace_root)?;
    let indexed_files = indexed_paths.len();
    let raw_symbols = build_workspace_index(&indexed_paths, None)?;
    let resolved_symbols = resolve_symbol_dependencies(&raw_symbols);
    Ok((resolved_symbols, indexed_files))
}

fn resolve_workspace_symbols_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let mut indexed_paths = collect_source_files(&workspace_root)?;
    let mut known_paths: BTreeSet<String> = indexed_paths
        .iter()
        .map(|path| normalize_path(path))
        .collect();

    for override_path in file_overrides.keys() {
        let override_path = normalize_absolute_path(Path::new(override_path))?;
        if !path_is_inside_workspace(&workspace_root, &override_path)?
            || should_skip_index_path(&workspace_root, &override_path)
            || detect_language(&override_path).is_err()
        {
            continue;
        }

        let normalized_path = normalize_path(&override_path);
        if known_paths.insert(normalized_path) {
            indexed_paths.push(override_path);
        }
    }

    indexed_paths.sort();
    let indexed_files = indexed_paths.len();
    let raw_symbols = build_workspace_index(&indexed_paths, Some(file_overrides))?;
    let resolved_symbols = resolve_symbol_dependencies(&raw_symbols);
    Ok((resolved_symbols, indexed_files))
}

fn resolve_workspace_symbols_incremental_with_limits(
    workspace_root: &Path,
    db_path: &Path,
    limits: WorkspaceScanLimits,
) -> Result<IncrementalWorkspaceSymbols> {
    let indexed_paths = collect_source_files_with_limits(workspace_root, limits)?;
    let indexed_files = indexed_paths.len();
    let connection = Connection::open(db_path)?;
    ensure_symbol_tables(&connection)?;

    let persisted_states = load_file_states(&connection)?;
    let persisted_symbols = load_indexed_symbols_grouped_by_file(&connection)?;

    let mut raw_symbols = Vec::new();
    let mut file_states = Vec::new();
    let mut rebuilt_files = 0;
    let mut reused_files = 0;

    for path in indexed_paths {
        let source = read_source(&path)?;
        let normalized_path = normalize_path(&path);
        let fingerprint = source_fingerprint(&source);

        file_states.push(PersistedFileState {
            file_path: normalized_path.clone(),
            fingerprint,
        });

        if persisted_states
            .get(&normalized_path)
            .is_some_and(|stored| *stored == fingerprint)
            && let Some(stored_symbols) = persisted_symbols.get(&normalized_path)
        {
            raw_symbols.extend(stored_symbols.iter().cloned());
            reused_files += 1;
            continue;
        }

        let document = parse_document(&path, &source)?;
        raw_symbols.extend(index_symbols_from_document(&path, &source, &document)?);
        rebuilt_files += 1;
    }

    assign_symbol_ids(&mut raw_symbols)?;
    let resolved_symbols = resolve_symbol_dependencies(&raw_symbols);
    Ok((
        raw_symbols,
        resolved_symbols,
        file_states,
        indexed_files,
        rebuilt_files,
        reused_files,
    ))
}

fn trace_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    validate_trace_symbol_path(symbol_path)?;

    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    trace_from_symbol(resolved_symbols, indexed_files, symbol, direction)
}

fn trace_neighborhood_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    validate_trace_symbol_path(symbol_path)?;
    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    trace_neighborhood_from_symbol(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
    )
}

fn trace_symbol_graph_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<TraceSymbolGraphResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    trace_from_symbol(resolved_symbols, indexed_files, symbol, direction)
}

#[allow(clippy::too_many_arguments)]
fn trace_symbol_neighborhood_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<TraceSymbolNeighborhoodResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    trace_neighborhood_from_symbol(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
    )
}

fn read_symbol_from_meta(
    symbol: &SymbolMeta,
    indexed_files: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadResult> {
    read_symbol_result_from_meta(symbol, indexed_files, file_overrides)
}

fn read_symbol_context_from_meta(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolContextResult> {
    let read = read_symbol_from_meta(symbol, indexed_files, file_overrides)?;
    let trace = trace_from_symbol(resolved_symbols, indexed_files, symbol, direction)?;
    let result = SymbolContextResult { read, trace };
    result.validate_public_output()?;
    Ok(result)
}

fn read_symbol_neighborhood_context_from_meta(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolNeighborhoodContextResult> {
    let neighborhood = trace_neighborhood_from_symbol(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
    )?;
    let resolved_map = resolved_symbol_map(resolved_symbols);
    let mut reads = Vec::with_capacity(neighborhood.nodes.len());

    for node in &neighborhood.nodes {
        let symbol = resolved_map.get(&node.symbol.symbol_id).ok_or_else(|| {
            anyhow!(
                "symbol not found in workspace index while reading neighborhood node: {}",
                node.symbol.symbol_id
            )
        })?;
        reads.push(read_symbol_result_from_meta(
            symbol,
            indexed_files,
            file_overrides,
        )?);
    }

    let result = SymbolNeighborhoodContextResult {
        neighborhood,
        reads,
    };
    result.validate_public_output()?;
    Ok(result)
}

fn read_symbol_discovery_context_from_meta(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadDiscoveryContextResult> {
    let read = read_symbol_from_meta(symbol, indexed_files, file_overrides)?;
    let trace = trace_from_symbol(resolved_symbols, indexed_files, symbol, direction)?;
    let neighborhood_context = read_symbol_neighborhood_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
    )?;
    let result = SymbolReadDiscoveryContextResult {
        read,
        trace,
        neighborhood_context,
    };
    result.validate_public_output()?;
    Ok(result)
}

fn read_symbol_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    read_symbol_from_meta(symbol, indexed_files, file_overrides)
}

fn read_symbol_context_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolContextResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    read_symbol_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        file_overrides,
    )
}

#[allow(clippy::too_many_arguments)]
fn read_symbol_neighborhood_context_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolNeighborhoodContextResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    read_symbol_neighborhood_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
    )
}

#[allow(clippy::too_many_arguments)]
fn read_symbol_discovery_context_at_position_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadDiscoveryContextResult> {
    let symbol = resolve_symbol_at_position(resolved_symbols, file_path, position, file_overrides)?;
    read_symbol_discovery_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
    )
}

fn read_symbol_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadResult> {
    validate_trace_symbol_path(symbol_path)?;

    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    read_symbol_from_meta(symbol, indexed_files, file_overrides)
}

fn read_symbol_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolContextResult> {
    validate_trace_symbol_path(symbol_path)?;

    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    read_symbol_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        file_overrides,
    )
}

fn read_symbol_neighborhood_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolNeighborhoodContextResult> {
    validate_trace_symbol_path(symbol_path)?;

    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    read_symbol_neighborhood_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
    )
}

fn read_symbol_discovery_context_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadDiscoveryContextResult> {
    validate_trace_symbol_path(symbol_path)?;

    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?;
    read_symbol_discovery_context_from_meta(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        file_overrides,
    )
}

fn search_from_symbols(
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

fn search_context_from_symbols(
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
fn search_discovery_context_from_symbols(
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
fn search_neighborhood_context_from_symbols(
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

fn list_from_symbols(
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

fn list_context_from_symbols(
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
fn list_discovery_context_from_symbols(
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
fn list_neighborhood_context_from_symbols(
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::{SymbolMeta, ensure_symbol_tables, transitive_c_include_dependents};
    use crate::index_store::{
        SymbolRefreshPersistence, persist_symbol_index, persist_symbol_refresh,
        persisted_byte_range,
    };
    use crate::symbol_index_model::{IndexedSymbol, PersistedFileState};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn persisted_byte_range_rejects_inverted_ranges() {
        let symbol = SymbolMeta {
            semantic_path: "helper".to_string(),
            byte_range: (8, 4),
            ..Default::default()
        };

        let error = persisted_byte_range(&symbol)
            .expect_err("persisted byte ranges should reject inverted ranges");

        assert!(error.to_string().contains("start 8 is after end 4"));
    }

    #[test]
    fn persist_symbol_index_rolls_back_metadata_on_row_failure() {
        let dir = temporary_dir();
        let db_path = dir.join("symbols.db");
        let workspace = dir.join("workspace");
        let file_path = workspace.join("helper.py");
        let normalized_file = file_path.to_string_lossy().replace('\\', "/");
        seed_indexed_files_metadata(&db_path, "7");

        let raw_symbols = vec![invalid_indexed_symbol(&normalized_file)];
        let symbols = vec![invalid_symbol_meta(&normalized_file)];
        let file_states = vec![PersistedFileState {
            file_path: file_path.to_string_lossy().replace('\\', "/"),
            fingerprint: 1,
        }];

        let error = persist_symbol_index(
            &db_path,
            &workspace,
            &raw_symbols,
            &symbols,
            &file_states,
            1,
        )
        .expect_err("invalid rows should abort the full persistence transaction");

        assert!(error.to_string().contains("start 8 is after end 4"));
        assert_eq!(indexed_files_metadata(&db_path), "7");
    }

    #[test]
    fn persist_symbol_refresh_rolls_back_metadata_on_row_failure() {
        let dir = temporary_dir();
        let db_path = dir.join("symbols.db");
        let workspace = dir.join("workspace");
        let file_path = workspace.join("helper.py");
        let normalized_file = file_path.to_string_lossy().replace('\\', "/");
        seed_indexed_files_metadata(&db_path, "7");

        let raw_symbols = vec![invalid_indexed_symbol(&normalized_file)];
        let symbols = vec![invalid_symbol_meta(&normalized_file)];
        let file_states = BTreeMap::from([(normalized_file.clone(), 1)]);
        let changed_file_paths = BTreeSet::from([normalized_file]);
        let impacted_paths = BTreeSet::new();
        let resolved_symbols_by_id = BTreeMap::from([("helper".to_string(), symbols[0].clone())]);

        let error = persist_symbol_refresh(SymbolRefreshPersistence {
            db_path: &db_path,
            workspace_root: &workspace,
            raw_symbols: &raw_symbols,
            symbols: &symbols,
            resolved_symbols_by_id: &resolved_symbols_by_id,
            file_states: &file_states,
            changed_file_paths: &changed_file_paths,
            impacted_paths: &impacted_paths,
            indexed_files: 1,
        })
        .expect_err("invalid rows should abort the full refresh transaction");

        assert!(error.to_string().contains("start 8 is after end 4"));
        assert_eq!(indexed_files_metadata(&db_path), "7");
    }

    #[test]
    fn transitive_c_include_dependents_skips_symlink_header_escape() {
        let root = temporary_dir();
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(
            workspace.join("source.c"),
            "#include \"linked.h\"\n\nint value(void) {\n    return 1;\n}\n",
        )
        .unwrap();
        fs::write(outside.join("linked.h"), "int secret(void);\n").unwrap();

        let linked_header = workspace.join("linked.h");
        if !try_symlink_file(&outside.join("linked.h"), &linked_header) {
            let _ = fs::remove_dir_all(root);
            return;
        }

        let dependents = transitive_c_include_dependents(&workspace, &linked_header).unwrap();

        assert!(dependents.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    fn seed_indexed_files_metadata(db_path: &Path, value: &str) {
        let connection = Connection::open(db_path).unwrap();
        ensure_symbol_tables(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO metadata(key, value) VALUES('indexed_files', ?1)",
                [value],
            )
            .unwrap();
    }

    fn indexed_files_metadata(db_path: &Path) -> String {
        let connection = Connection::open(db_path).unwrap();
        connection
            .query_row(
                "SELECT value FROM metadata WHERE key = 'indexed_files'",
                [],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn invalid_indexed_symbol(file_path: &str) -> IndexedSymbol {
        IndexedSymbol {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            base_name: "helper".to_string(),
            scope_path: None,
            file_path: file_path.to_string(),
            node_kind: "function_definition".to_string(),
            byte_range: (8, 4),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
            references_by_name: BTreeSet::new(),
        }
    }

    fn invalid_symbol_meta(file_path: &str) -> SymbolMeta {
        SymbolMeta {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            file_path: file_path.to_string(),
            node_kind: "function_definition".to_string(),
            byte_range: (8, 4),
            ..Default::default()
        }
    }

    fn temporary_dir() -> std::path::PathBuf {
        let suffix = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(format!("arborist-symbols-{suffix}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    fn try_symlink_file(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn try_symlink_file(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
}
