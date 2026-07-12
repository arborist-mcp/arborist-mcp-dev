use std::path::Path;

use anyhow::Result;

use crate::language::normalize_absolute_path;
use crate::model::{
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, TraceDirection,
};
use crate::symbol_index_state::load_symbol_index;
use crate::symbol_query_execution::{
    list_context_from_symbols, list_discovery_context_from_symbols, list_from_symbols,
    list_neighborhood_context_from_symbols,
};

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
