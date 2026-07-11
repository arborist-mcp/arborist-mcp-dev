use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::language::{ensure_path_inside_workspace, normalize_absolute_path};
use crate::model::{
    Position, TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult,
};
use crate::symbol_index_state::load_symbol_index_with_overrides;
use crate::symbol_index_workspace::resolve_workspace_symbols_with_overrides;
use crate::symbol_query_execution::{
    trace_from_symbols, trace_neighborhood_from_symbols,
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
