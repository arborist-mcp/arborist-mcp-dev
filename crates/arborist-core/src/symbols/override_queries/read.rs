use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::language::normalize_absolute_path;
use crate::model::{
    Position, SymbolContextResult, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, TraceDirection,
};
use crate::symbol_index_workspace::resolve_workspace_symbols_with_overrides;
use crate::symbol_query_execution::{
    read_symbol_at_position_from_symbols, read_symbol_context_at_position_from_symbols,
    read_symbol_context_from_symbols, read_symbol_discovery_context_at_position_from_symbols,
    read_symbol_discovery_context_from_symbols, read_symbol_from_symbols,
    read_symbol_neighborhood_context_at_position_from_symbols,
    read_symbol_neighborhood_context_from_symbols,
};

use super::{
    load_normalized_symbol_index_with_overrides, load_workspace_symbols_with_overrides_at_path,
};

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
    let (file_path, resolved_symbols, indexed_files) =
        load_workspace_symbols_with_overrides_at_path(workspace_root, file_overrides, file_path)?;
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
    let (file_path, resolved_symbols, indexed_files) =
        load_workspace_symbols_with_overrides_at_path(workspace_root, file_overrides, file_path)?;
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
    let (file_path, resolved_symbols, indexed_files) =
        load_workspace_symbols_with_overrides_at_path(workspace_root, file_overrides, file_path)?;
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
    let (file_path, resolved_symbols, indexed_files) =
        load_workspace_symbols_with_overrides_at_path(workspace_root, file_overrides, file_path)?;
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

pub fn read_symbol_from_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
) -> Result<SymbolReadResult> {
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
    let file_path = normalize_absolute_path(file_path)?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
