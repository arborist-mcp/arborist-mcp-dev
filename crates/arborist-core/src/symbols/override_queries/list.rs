use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::model::{
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, TraceDirection,
};
use crate::symbol_index_workspace::resolve_workspace_symbols_with_overrides;
use crate::symbol_query_execution::{
    list_context_from_symbols, list_discovery_context_from_symbols, list_from_symbols,
    list_neighborhood_context_from_symbols,
};

use super::load_normalized_symbol_index_with_overrides;

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

pub fn list_symbols_from_index_with_overrides_filtered(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides(db_path, file_overrides)?;
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
