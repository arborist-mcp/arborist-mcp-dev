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
use crate::symbol_index_workspace::{load_live_workspace_symbols, resolve_workspace_symbols};
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
