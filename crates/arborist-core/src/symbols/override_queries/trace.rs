use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::language::normalize_absolute_path;
use crate::model::{
    Position, TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult,
};
use crate::symbol_index_workspace::resolve_workspace_symbols_with_overrides;
use crate::symbol_query_execution::{
    trace_from_symbols_with_timeout, trace_neighborhood_from_symbols_with_timeout,
    trace_symbol_graph_at_position_from_symbols_with_timeout,
    trace_symbol_neighborhood_at_position_from_symbols_with_timeout,
};

use super::{
    load_normalized_symbol_index_with_overrides, load_workspace_symbols_with_overrides_at_path,
};

pub fn trace_symbol_graph_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_symbol_graph_with_overrides_and_timeout(
        workspace_root,
        file_overrides,
        symbol_path,
        direction,
        None,
    )
}

pub fn trace_symbol_graph_with_overrides_and_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    trace_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        timeout_ms,
    )
}

pub fn trace_symbol_neighborhood_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    trace_symbol_neighborhood_with_overrides_and_timeout(
        workspace_root,
        file_overrides,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn trace_symbol_neighborhood_with_overrides_and_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    trace_neighborhood_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        timeout_ms,
    )
}

#[allow(dead_code)]
pub fn trace_symbol_graph_at_position_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_symbol_graph_at_position_with_overrides_and_timeout(
        workspace_root,
        file_overrides,
        file_path,
        position,
        direction,
        None,
    )
}

pub fn trace_symbol_graph_at_position_with_overrides_and_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    let (file_path, resolved_symbols, indexed_files) =
        load_workspace_symbols_with_overrides_at_path(workspace_root, file_overrides, file_path)?;
    trace_symbol_graph_at_position_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        Some(file_overrides),
        timeout_ms,
    )
}

#[allow(dead_code)]
pub fn trace_symbol_neighborhood_at_position_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    trace_symbol_neighborhood_at_position_with_overrides_and_timeout(
        workspace_root,
        file_overrides,
        file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn trace_symbol_neighborhood_at_position_with_overrides_and_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    let (file_path, resolved_symbols, indexed_files) =
        load_workspace_symbols_with_overrides_at_path(workspace_root, file_overrides, file_path)?;
    trace_symbol_neighborhood_at_position_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
        timeout_ms,
    )
}

pub fn trace_symbol_graph_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_symbol_graph_from_index_with_overrides_and_timeout(
        db_path,
        file_overrides,
        symbol_path,
        direction,
        None,
    )
}

pub fn trace_symbol_graph_from_index_with_overrides_and_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
    trace_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        timeout_ms,
    )
}

pub fn trace_symbol_neighborhood_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    trace_symbol_neighborhood_from_index_with_overrides_and_timeout(
        db_path,
        file_overrides,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub fn trace_symbol_neighborhood_from_index_with_overrides_and_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
    trace_neighborhood_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        timeout_ms,
    )
}

#[allow(dead_code)]
pub fn trace_symbol_graph_at_position_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_symbol_graph_at_position_from_index_with_overrides_and_timeout(
        db_path,
        file_overrides,
        file_path,
        position,
        direction,
        None,
    )
}

pub fn trace_symbol_graph_at_position_from_index_with_overrides_and_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
    trace_symbol_graph_at_position_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        Some(file_overrides),
        timeout_ms,
    )
}

#[allow(dead_code)]
pub fn trace_symbol_neighborhood_at_position_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    trace_symbol_neighborhood_at_position_from_index_with_overrides_and_timeout(
        db_path,
        file_overrides,
        file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn trace_symbol_neighborhood_at_position_from_index_with_overrides_and_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
    trace_symbol_neighborhood_at_position_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
        timeout_ms,
    )
}
