use std::path::Path;

use anyhow::Result;

use crate::model::{
    SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
    SymbolSearchNeighborhoodContextResult, SymbolSearchResult, TraceDirection,
};
use crate::symbol_index_workspace::load_live_workspace_symbols;
use crate::symbol_query_execution::{
    search_context_from_symbols, search_discovery_context_from_symbols, search_from_symbols,
    search_neighborhood_context_from_symbols,
};

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
