use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::language::{ensure_path_inside_workspace, normalize_absolute_path};
use crate::model::Position;
use crate::model::{
    SymbolContextResult, SymbolListContextResult, SymbolListDiscoveryContextResult,
    SymbolListNeighborhoodContextResult, SymbolListResult, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, SymbolSearchContextResult,
    SymbolSearchDiscoveryContextResult, SymbolSearchNeighborhoodContextResult, SymbolSearchResult,
    TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult,
};
use crate::symbol_index_state::load_symbol_index_with_overrides;
use crate::symbol_index_workspace::resolve_workspace_symbols_with_overrides;
use crate::symbol_query_execution::{
    list_context_from_symbols, list_discovery_context_from_symbols, list_from_symbols,
    list_neighborhood_context_from_symbols, read_symbol_at_position_from_symbols,
    read_symbol_context_at_position_from_symbols, read_symbol_context_from_symbols,
    read_symbol_discovery_context_at_position_from_symbols,
    read_symbol_discovery_context_from_symbols, read_symbol_from_symbols,
    read_symbol_neighborhood_context_at_position_from_symbols,
    read_symbol_neighborhood_context_from_symbols, search_context_from_symbols,
    search_discovery_context_from_symbols, search_from_symbols,
    search_neighborhood_context_from_symbols, trace_from_symbols, trace_neighborhood_from_symbols,
    trace_symbol_graph_at_position_from_symbols,
    trace_symbol_neighborhood_at_position_from_symbols,
};

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
