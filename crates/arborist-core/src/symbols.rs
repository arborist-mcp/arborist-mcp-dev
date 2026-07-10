use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params, types::Type};
use serde::de::DeserializeOwned;
use tree_sitter::Node;

use crate::language::{
    c_companion_source_path, c_include_targets, c_local_include_targets, contains_kind,
    detect_language, ensure_path_inside_workspace, is_c_header_path, node_text,
    normalize_absolute_path, normalize_path, offset_for_position, parse_document,
    path_is_inside_workspace, point_for_offset, position_from, read_source,
    resolve_local_c_include, visit_tree,
};
use crate::model::{LanguageId, Position, SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION};
use crate::model::{
    SymbolContextResult, SymbolIndexHealth, SymbolIndexStats, SymbolListContextResult,
    SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult, SymbolListResult,
    SymbolMeta, SymbolMetaInit, SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult,
    SymbolReadResult, SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
    SymbolSearchMatchDetail, SymbolSearchNeighborhoodContextResult, SymbolSearchResult,
    SymbolSummary, SymbolSummaryInit, TraceDirection, TraceEvidenceKeys, TraceSymbolGraphResult,
    TraceSymbolNeighborhoodEdge, TraceSymbolNeighborhoodNode, TraceSymbolNeighborhoodResult,
};
use crate::patching::{
    collect_c_references, collect_python_references, resolve_local_python_imported_symbol,
    resolve_local_python_module_path,
};
use crate::semantic::{
    ascend_to_symbol, c_function_header, c_parameters, c_return_type, c_semantic_path,
    c_symbol_id_for_node, python_display_byte_range, python_display_header, python_docstring,
    python_parameters, python_return_type, semantic_parent_path, semantic_path,
};

#[derive(Debug, Clone)]
struct IndexedSymbol {
    symbol_id: String,
    semantic_path: String,
    base_name: String,
    scope_path: Option<String>,
    file_path: String,
    node_kind: String,
    byte_range: (usize, usize),
    signature: Option<String>,
    parameters: Vec<String>,
    return_type: Option<String>,
    docstring: Option<String>,
    references_by_name: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct PersistedFileState {
    file_path: String,
    fingerprint: u64,
}

type IncrementalWorkspaceSymbols = (
    Vec<IndexedSymbol>,
    Vec<SymbolMeta>,
    Vec<PersistedFileState>,
    usize,
    usize,
    usize,
);

struct SymbolRefreshPersistence<'a> {
    db_path: &'a Path,
    workspace_root: &'a Path,
    raw_symbols: &'a [IndexedSymbol],
    symbols: &'a [SymbolMeta],
    file_states: &'a BTreeMap<String, u64>,
    changed_file_paths: &'a BTreeSet<String>,
    impacted_paths: &'a BTreeSet<String>,
    indexed_files: usize,
}

#[derive(Debug, Default)]
struct CIncludeContext {
    include_paths: BTreeSet<String>,
    companion_source_paths: BTreeSet<String>,
}

const SKIPPED_WORKSPACE_DIR_NAMES: &[&str] = &[
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".venv",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "target",
    "venv",
];

const SYMBOL_INDEX_SCHEMA_VERSION: &str = "1";

pub fn trace_symbol_graph(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
    trace_from_symbols(&resolved_symbols, indexed_files, symbol_path, direction)
}

pub fn trace_symbol_neighborhood(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
    read_symbol_from_symbols(&resolved_symbols, indexed_files, symbol_path, None)
}

pub fn read_symbol_context(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let db_path = normalize_absolute_path(db_path)?;
    let (raw_symbols, resolved_symbols, file_states, indexed_files, rebuilt_files, reused_files) =
        resolve_workspace_symbols_incremental(&workspace_root, &db_path)?;
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
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;

    ensure_path_inside_workspace(&workspace_root, &file_path)?;

    if !db_path.exists() {
        return rebuild_symbol_index(&workspace_root, &db_path);
    }

    let connection = Connection::open(&db_path)?;
    require_symbol_index_tables(&connection, &db_path)?;
    validate_symbol_index_workspace(&connection, &workspace_root, &db_path)?;
    load_indexed_files_metadata(&connection)?;
    validate_symbol_index_schema_version(&connection, &db_path)?;
    ensure_symbol_tables(&connection)?;

    let old_resolved_symbols = load_symbols_from_connection(&connection)?.0;
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

pub fn inspect_symbol_index(db_path: &Path) -> Result<SymbolIndexHealth> {
    let db_path = normalize_absolute_path(db_path)?;
    let db_path_display = normalize_path(&db_path);
    let mut health = SymbolIndexHealth {
        response_schema_version: SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION.to_string(),
        db_path: db_path_display,
        exists: db_path.exists(),
        ok: false,
        schema_version: None,
        expected_schema_version: SYMBOL_INDEX_SCHEMA_VERSION.to_string(),
        workspace_root: None,
        indexed_files: None,
        indexed_symbols: None,
        file_state_entries: None,
        fresh_file_count: None,
        stale_files: Vec::new(),
        missing_files: Vec::new(),
        unreadable_files: Vec::new(),
        issues: Vec::new(),
    };

    if !health.exists {
        health
            .issues
            .push(format!("symbol index {} does not exist", db_path.display()));
        health.validate_public_output()?;
        return Ok(health);
    }

    let connection = match Connection::open(&db_path) {
        Ok(connection) => connection,
        Err(error) => {
            health
                .issues
                .push(format!("failed to open symbol index: {error}"));
            health.validate_public_output()?;
            return Ok(health);
        }
    };

    if let Err(error) = require_symbol_index_tables(&connection, &db_path) {
        health.issues.push(error.to_string());
        health.validate_public_output()?;
        return Ok(health);
    }

    health.schema_version =
        load_optional_metadata_value(&connection, "schema_version").map_err(|error| {
            anyhow!(
                "failed to inspect schema_version metadata in {}: {}",
                db_path.display(),
                error
            )
        })?;
    if health.schema_version.is_none() {
        health.issues.push(format!(
            "missing schema_version metadata in symbol index {}",
            db_path.display()
        ));
    } else if health.schema_version.as_deref() != Some(SYMBOL_INDEX_SCHEMA_VERSION) {
        health.issues.push(format!(
            "unsupported symbol index schema_version `{}` in {}; expected `{}`",
            health.schema_version.as_deref().unwrap_or_default(),
            db_path.display(),
            SYMBOL_INDEX_SCHEMA_VERSION
        ));
    }

    match load_symbol_index_workspace_root(&connection, &db_path) {
        Ok(workspace_root) => health.workspace_root = Some(normalize_path(&workspace_root)),
        Err(error) => health.issues.push(error.to_string()),
    }

    match load_indexed_files_metadata(&connection) {
        Ok(indexed_files) => health.indexed_files = Some(indexed_files),
        Err(error) => health.issues.push(error.to_string()),
    }

    match count_table_rows(&connection, "symbols") {
        Ok(count) => health.indexed_symbols = Some(count),
        Err(error) => health
            .issues
            .push(format!("failed to count persisted symbols: {error}")),
    }
    match count_table_rows(&connection, "file_state") {
        Ok(count) => health.file_state_entries = Some(count),
        Err(error) => health
            .issues
            .push(format!("failed to count persisted file states: {error}")),
    }

    match load_file_states(&connection) {
        Ok(file_states) => inspect_symbol_index_freshness(&mut health, &file_states),
        Err(error) => health
            .issues
            .push(format!("failed to inspect persisted file states: {error}")),
    }

    health.ok = health.issues.is_empty();
    health.validate_public_output()?;
    Ok(health)
}

fn inspect_symbol_index_freshness(
    health: &mut SymbolIndexHealth,
    file_states: &BTreeMap<String, u64>,
) {
    let mut fresh_files = 0;
    for (file_path, stored_fingerprint) in file_states {
        let path = Path::new(file_path);
        if !path.exists() {
            health.missing_files.push(file_path.clone());
            health
                .issues
                .push(format!("indexed file is missing: {file_path}"));
            continue;
        }

        match read_source(path) {
            Ok(source) => {
                let current_fingerprint = source_fingerprint(&source);
                if current_fingerprint == *stored_fingerprint {
                    fresh_files += 1;
                } else {
                    health.stale_files.push(file_path.clone());
                    health
                        .issues
                        .push(format!("indexed file is stale: {file_path}"));
                }
            }
            Err(error) => {
                health.unreadable_files.push(file_path.clone());
                health
                    .issues
                    .push(format!("failed to read indexed file {file_path}: {error}"));
            }
        }
    }
    health.fresh_file_count = Some(fresh_files);
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
            if !include_path.starts_with(workspace_root) {
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

fn collect_source_files(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    walk_workspace(workspace_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk_workspace(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if path.is_dir() {
        if should_skip_dir(path) {
            return Ok(());
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            walk_workspace(&entry.path(), files)?;
        }
        return Ok(());
    }

    if detect_language(path).is_ok() {
        files.push(path.to_path_buf());
    }

    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(should_skip_dir_name)
}

fn should_skip_index_path(workspace_root: &Path, path: &Path) -> bool {
    path.strip_prefix(workspace_root)
        .ok()
        .is_some_and(|relative_path| {
            relative_path.components().any(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .is_some_and(should_skip_dir_name)
            })
        })
}

fn should_skip_dir_name(name: &str) -> bool {
    SKIPPED_WORKSPACE_DIR_NAMES
        .iter()
        .any(|skipped| name.eq_ignore_ascii_case(skipped))
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

fn index_symbols_from_document(
    path: &Path,
    source: &str,
    document: &crate::language::ParsedDocument,
) -> Result<Vec<IndexedSymbol>> {
    match document.language_id {
        LanguageId::Python => index_python_symbols(path, source, document.tree.root_node()),
        LanguageId::C => index_c_symbols(path, source, document.tree.root_node()),
    }
}

fn index_python_symbols(path: &Path, source: &str, root: Node<'_>) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();
    let normalized_path = normalize_path(path);

    let mut callback = |node: Node<'_>| {
        if !matches!(node.kind(), "class_definition" | "function_definition") {
            return;
        }

        let mut references = BTreeSet::new();
        let reference_node = python_reference_node(node);
        let _ = collect_python_references(path, reference_node, source, &mut references);
        let signature = python_display_header(node, source).ok();
        let path = match semantic_path(node, source) {
            Ok(path) => path,
            Err(_) => return,
        };
        let scope_path = semantic_parent_path(&path);
        let parameters = python_parameters(node, source).unwrap_or_default();
        let return_type = python_return_type(node, source).ok().flatten();
        let docstring = python_docstring(node, source).ok().flatten();

        symbols.push(IndexedSymbol {
            symbol_id: String::new(),
            base_name: path.rsplit('.').next().unwrap_or(&path).to_string(),
            semantic_path: path,
            scope_path,
            file_path: normalized_path.clone(),
            node_kind: node.kind().to_string(),
            byte_range: python_display_byte_range(node),
            signature,
            parameters,
            return_type,
            docstring,
            references_by_name: references,
        });
    };

    visit_tree(root, &mut callback);
    Ok(symbols)
}

fn python_reference_node(node: Node<'_>) -> Node<'_> {
    node.parent()
        .filter(|parent| parent.kind() == "decorated_definition")
        .unwrap_or(node)
}

fn index_c_symbols(path: &Path, source: &str, root: Node<'_>) -> Result<Vec<IndexedSymbol>> {
    let normalized_path = normalize_path(path);
    let mut symbols = Vec::new();
    let mut cursor = root.walk();

    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "type_definition" => {
                if let Some(name) = c_semantic_path(path, child, source)? {
                    symbols.push(IndexedSymbol {
                        symbol_id: String::new(),
                        base_name: name.rsplit("::").next().unwrap_or(&name).to_string(),
                        semantic_path: name,
                        scope_path: None,
                        file_path: normalized_path.clone(),
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(node_text(child, source)?.trim().to_string()),
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                        references_by_name: BTreeSet::new(),
                    });
                }
            }
            "declaration" if contains_kind(child, "function_declarator") => {
                if let Some(name) = c_semantic_path(path, child, source)? {
                    let scope_path = semantic_parent_path(&name);
                    symbols.push(IndexedSymbol {
                        symbol_id: String::new(),
                        base_name: name.rsplit("::").next().unwrap_or(&name).to_string(),
                        semantic_path: name,
                        scope_path,
                        file_path: normalized_path.clone(),
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(node_text(child, source)?.trim().to_string()),
                        parameters: c_parameters(child, source)?,
                        return_type: c_return_type(child, source)?,
                        docstring: None,
                        references_by_name: BTreeSet::new(),
                    });
                }
            }
            "function_definition" => {
                if let Some(name) = c_semantic_path(path, child, source)? {
                    let mut references = BTreeSet::new();
                    collect_c_references(child, source, &mut references)?;
                    let scope_path = semantic_parent_path(&name);
                    symbols.push(IndexedSymbol {
                        symbol_id: String::new(),
                        base_name: name.rsplit("::").next().unwrap_or(&name).to_string(),
                        semantic_path: name,
                        scope_path,
                        file_path: normalized_path.clone(),
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(c_function_header(child, source)?),
                        parameters: c_parameters(child, source)?,
                        return_type: c_return_type(child, source)?,
                        docstring: None,
                        references_by_name: references,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(symbols)
}

fn resolve_reference_path(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
) -> Option<String> {
    let language_id = detect_language(Path::new(&source_symbol.file_path)).ok();
    let (lookup_name, module_hint) = if language_id == Some(LanguageId::Python) {
        python_reference_lookup(reference_name)
    } else {
        (reference_name, None)
    };
    let candidates = name_index.get(lookup_name)?;
    let visible_candidates: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.file_path == source_symbol.file_path
                || !candidate.semantic_path.contains("::")
        })
        .collect();
    let candidate_slice = if visible_candidates.is_empty() {
        candidates.as_slice()
    } else {
        visible_candidates.as_slice()
    };
    let hinted_candidates = if let Some(module_hint) = module_hint {
        let imported_summary = resolve_local_python_imported_symbol(
            Path::new(&source_symbol.file_path),
            module_hint,
            lookup_name,
        )
        .ok()
        .flatten();
        let filtered = candidate_slice
            .iter()
            .copied()
            .filter(|index| {
                python_symbol_matches_module_hint(
                    source_symbol,
                    &raw_symbols[*index],
                    module_hint,
                    imported_summary.as_ref(),
                )
            })
            .collect::<Vec<_>>();
        if filtered.is_empty() {
            candidate_slice.to_vec()
        } else {
            filtered
        }
    } else {
        candidate_slice.to_vec()
    };
    let include_context = c_include_context_for_file(&source_symbol.file_path).ok();

    hinted_candidates
        .iter()
        .copied()
        .max_by_key(|index| {
            indexed_symbol_candidate_rank(
                &raw_symbols[*index],
                Some(&source_symbol.file_path),
                include_context.as_ref(),
            )
        })
        .map(|index| raw_symbols[index].symbol_id.clone())
}

fn python_reference_lookup(reference_name: &str) -> (&str, Option<&str>) {
    reference_name
        .rsplit_once('.')
        .map(|(module_hint, symbol_name)| (symbol_name, Some(module_hint)))
        .unwrap_or((reference_name, None))
}

fn python_symbol_matches_module_hint(
    source_symbol: &IndexedSymbol,
    symbol: &IndexedSymbol,
    module_hint: &str,
    imported_summary: Option<&SymbolSummary>,
) -> bool {
    if let Some(imported_summary) = imported_summary {
        return imported_summary.file_path == symbol.file_path
            && imported_summary.semantic_path == symbol.semantic_path;
    }

    let Some(resolved_module_path) =
        resolve_local_python_module_path(Path::new(&source_symbol.file_path), module_hint)
    else {
        return false;
    };

    normalize_path(&resolved_module_path) == symbol.file_path
}

fn summarize_symbols(
    symbols: &[SymbolMeta],
    semantic_paths: &[String],
    context_file: Option<&str>,
) -> Vec<SymbolSummary> {
    let include_context = context_file.and_then(|file| c_include_context_for_file(file).ok());
    semantic_paths
        .iter()
        .filter_map(|semantic_path| {
            choose_symbol_summary(
                symbols,
                semantic_path,
                context_file,
                include_context.as_ref(),
            )
        })
        .collect()
}

fn choose_symbol_summary(
    symbols: &[SymbolMeta],
    symbol_id: &str,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> Option<SymbolSummary> {
    symbols
        .iter()
        .filter(|symbol| symbol.symbol_id == symbol_id)
        .max_by_key(|symbol| symbol_candidate_rank(symbol, context_file, include_context))
        .map(|symbol| {
            SymbolSummary::new(SymbolSummaryInit {
                symbol_id: symbol.symbol_id.clone(),
                semantic_path: symbol.semantic_path.clone(),
                scope_path: symbol.scope_path.clone(),
                file_path: symbol.file_path.clone(),
                node_kind: symbol.node_kind.clone(),
                origin_type: symbol_origin_type(symbol, context_file, include_context).to_string(),
                byte_range: symbol.byte_range,
                signature: symbol.signature.clone(),
                parameters: symbol.parameters.clone(),
                return_type: symbol.return_type.clone(),
                docstring: symbol.docstring.clone(),
            })
        })
}

fn symbol_origin_type(
    symbol: &SymbolMeta,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> &'static str {
    if context_file.is_some_and(|context_file| symbol.file_path == context_file) {
        return "local_file";
    }

    if include_context.is_some_and(|include_context| {
        include_context
            .companion_source_paths
            .contains(&symbol.file_path)
    }) {
        return "companion_source";
    }

    if include_context
        .is_some_and(|include_context| include_context.include_paths.contains(&symbol.file_path))
    {
        return "include_header";
    }

    "workspace_symbol"
}

fn symbol_candidate_rank(
    symbol: &SymbolMeta,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> usize {
    let mut rank = resolved_symbol_rank(symbol);

    if let Some(context_file) = context_file {
        if symbol.file_path == context_file {
            rank += 1000;
        } else if symbol.semantic_path.contains("::") {
            rank = rank.saturating_sub(100);
        }
    }

    if let Some(include_context) = include_context {
        if include_context.include_paths.contains(&symbol.file_path) {
            rank += 200;
        }
        if include_context
            .companion_source_paths
            .contains(&symbol.file_path)
        {
            rank += 300;
        }
    }

    rank
}

fn indexed_symbol_candidate_rank(
    symbol: &IndexedSymbol,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> usize {
    let mut rank = indexed_symbol_rank(symbol);

    if let Some(context_file) = context_file {
        if symbol.file_path == context_file {
            rank += 1000;
        } else if symbol.semantic_path.contains("::") {
            rank = rank.saturating_sub(100);
        }
    }

    if let Some(include_context) = include_context {
        if include_context.include_paths.contains(&symbol.file_path) {
            rank += 200;
        }
        if include_context
            .companion_source_paths
            .contains(&symbol.file_path)
        {
            rank += 300;
        }
    }

    rank
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

fn resolve_workspace_symbols_incremental(
    workspace_root: &Path,
    db_path: &Path,
) -> Result<IncrementalWorkspaceSymbols> {
    let indexed_paths = collect_source_files(workspace_root)?;
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

fn build_name_index(raw_symbols: &[IndexedSymbol]) -> BTreeMap<String, Vec<usize>> {
    let mut name_index = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        name_index
            .entry(symbol.base_name.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    name_index
}

fn assign_symbol_ids(raw_symbols: &mut [IndexedSymbol]) -> Result<()> {
    let symbol_ids = (0..raw_symbols.len())
        .map(|index| symbol_id_for_index(index, raw_symbols))
        .collect::<Result<Vec<_>>>()?;

    for (symbol, symbol_id) in raw_symbols.iter_mut().zip(symbol_ids) {
        symbol.symbol_id = symbol_id;
    }

    Ok(())
}

fn symbol_id_for_index(index: usize, raw_symbols: &[IndexedSymbol]) -> Result<String> {
    let symbol = &raw_symbols[index];
    let path = Path::new(&symbol.file_path);
    if detect_language(path).ok() != Some(LanguageId::C) || symbol.semantic_path.contains("::") {
        return Ok(symbol.semantic_path.clone());
    }

    let anchor = if is_c_header_path(path) {
        symbol.file_path.clone()
    } else {
        c_symbol_family_anchor(symbol, raw_symbols)?
    };

    Ok(format!("{anchor}::{}", symbol.base_name))
}

fn c_symbol_family_anchor(symbol: &IndexedSymbol, raw_symbols: &[IndexedSymbol]) -> Result<String> {
    let include_context = c_include_context_for_file(&symbol.file_path)?;
    let source_path = Path::new(&symbol.file_path);

    let best_header = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.semantic_path == symbol.semantic_path
                && !candidate.semantic_path.contains("::")
                && is_c_header_path(Path::new(&candidate.file_path))
        })
        .map(|candidate| {
            let rank = c_family_header_rank(source_path, &candidate.file_path, &include_context);
            (candidate, rank)
        })
        .filter(|(_, rank)| *rank > 0)
        .max_by_key(|(_, rank)| *rank)
        .map(|(candidate, _)| candidate);

    Ok(best_header
        .map(|candidate| candidate.file_path.clone())
        .unwrap_or_else(|| symbol.file_path.clone()))
}

fn c_family_header_rank(
    source_path: &Path,
    header_file_path: &str,
    include_context: &CIncludeContext,
) -> usize {
    let mut rank = 0;
    let header_path = Path::new(header_file_path);
    if same_stem(source_path, header_path) {
        rank += 1000;
    }
    if include_context.include_paths.contains(header_file_path) {
        rank += 500;
    }
    rank
}

fn same_stem(left: &Path, right: &Path) -> bool {
    left.file_stem()
        .and_then(|stem| stem.to_str())
        .zip(right.file_stem().and_then(|stem| stem.to_str()))
        .is_some_and(|(left_stem, right_stem)| left_stem == right_stem)
}

fn symbol_base_name(semantic_path: &str) -> String {
    semantic_path
        .rsplit("::")
        .next()
        .unwrap_or(semantic_path)
        .rsplit('.')
        .next()
        .unwrap_or(semantic_path)
        .to_string()
}

fn symbol_meta_from_indexed(symbol: &IndexedSymbol) -> SymbolMeta {
    SymbolMeta::new(SymbolMetaInit {
        symbol_id: symbol.symbol_id.clone(),
        semantic_path: symbol.semantic_path.clone(),
        scope_path: symbol.scope_path.clone(),
        file_path: symbol.file_path.clone(),
        node_kind: symbol.node_kind.clone(),
        origin_type: "workspace_symbol".to_string(),
        byte_range: symbol.byte_range,
        signature: symbol.signature.clone(),
        parameters: symbol.parameters.clone(),
        return_type: symbol.return_type.clone(),
        docstring: symbol.docstring.clone(),
        dependencies: Vec::new(),
        references: Vec::new(),
    })
}

fn raw_symbol_indexes_by_id(raw_symbols: &[IndexedSymbol]) -> BTreeMap<String, Vec<usize>> {
    let mut indexes = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        indexes
            .entry(symbol.symbol_id.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    indexes
}

fn resolve_dependencies_for_symbol(
    symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
) -> Vec<String> {
    let mut dependencies = BTreeSet::new();
    for reference_name in &symbol.references_by_name {
        if let Some(target_symbol_id) =
            resolve_reference_path(reference_name, symbol, raw_symbols, name_index)
            && target_symbol_id != symbol.symbol_id
        {
            dependencies.insert(target_symbol_id);
        }
    }
    dependencies.into_iter().collect()
}

fn resolve_symbol_dependencies(raw_symbols: &[IndexedSymbol]) -> Vec<SymbolMeta> {
    let name_index = build_name_index(raw_symbols);
    let symbol_indexes = raw_symbol_indexes_by_id(raw_symbols);
    let mut dependency_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for (symbol_id, indexes) in &symbol_indexes {
        let dependencies = dependency_map.entry(symbol_id.clone()).or_default();
        for index in indexes {
            dependencies.extend(resolve_dependencies_for_symbol(
                &raw_symbols[*index],
                raw_symbols,
                &name_index,
            ));
        }
    }

    let mut reference_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (caller, callees) in &dependency_map {
        for callee in callees {
            reference_map
                .entry(callee.clone())
                .or_default()
                .insert(caller.clone());
        }
    }

    raw_symbols
        .iter()
        .map(|symbol| {
            SymbolMeta::new(SymbolMetaInit {
                symbol_id: symbol.symbol_id.clone(),
                semantic_path: symbol.semantic_path.clone(),
                scope_path: symbol.scope_path.clone(),
                file_path: symbol.file_path.clone(),
                node_kind: symbol.node_kind.clone(),
                origin_type: "workspace_symbol".to_string(),
                byte_range: symbol.byte_range,
                signature: symbol.signature.clone(),
                parameters: symbol.parameters.clone(),
                return_type: symbol.return_type.clone(),
                docstring: symbol.docstring.clone(),
                dependencies: dependency_map
                    .get(&symbol.symbol_id)
                    .map(|dependencies| dependencies.iter().cloned().collect())
                    .unwrap_or_default(),
                references: reference_map
                    .get(&symbol.symbol_id)
                    .map(|references| references.iter().cloned().collect())
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn impacted_symbol_ids(
    raw_symbols: &[IndexedSymbol],
    old_changed_symbols: &[IndexedSymbol],
    new_changed_symbols: &[IndexedSymbol],
    old_resolved_map: &BTreeMap<String, SymbolMeta>,
    changed_file_paths: &BTreeSet<String>,
) -> BTreeSet<String> {
    let impacted_names: BTreeSet<_> = old_changed_symbols
        .iter()
        .chain(new_changed_symbols.iter())
        .map(|symbol| symbol.base_name.clone())
        .collect();
    let changed_reference_names: BTreeSet<_> = old_changed_symbols
        .iter()
        .chain(new_changed_symbols.iter())
        .flat_map(|symbol| {
            symbol
                .references_by_name
                .iter()
                .map(|reference| reference_base_name(reference))
                .collect::<Vec<_>>()
        })
        .collect();

    let mut impacted_ids: BTreeSet<_> = old_changed_symbols
        .iter()
        .chain(new_changed_symbols.iter())
        .map(|symbol| symbol.symbol_id.clone())
        .collect();

    for symbol in raw_symbols {
        if changed_file_paths.contains(&symbol.file_path) {
            continue;
        }
        if symbol.base_name.is_empty() {
            continue;
        }
        if symbol
            .references_by_name
            .iter()
            .any(|reference_name| impacted_names.contains(&reference_base_name(reference_name)))
            || changed_reference_names.contains(&symbol.base_name)
        {
            impacted_ids.insert(symbol.symbol_id.clone());
        }
    }

    let seed_ids: Vec<_> = impacted_ids.iter().cloned().collect();
    for symbol_id in seed_ids {
        if let Some(symbol) = old_resolved_map.get(&symbol_id) {
            impacted_ids.extend(symbol.dependencies.iter().cloned());
            impacted_ids.extend(symbol.references.iter().cloned());
        }
    }

    impacted_ids
}

fn refresh_resolved_symbol_subgraph(
    raw_symbols: &[IndexedSymbol],
    old_resolved_map: &BTreeMap<String, SymbolMeta>,
    old_changed_symbols: &[IndexedSymbol],
    new_changed_symbols: &[IndexedSymbol],
    changed_file_paths: &BTreeSet<String>,
) -> (BTreeMap<String, SymbolMeta>, BTreeSet<String>) {
    let name_index = build_name_index(raw_symbols);
    let raw_symbol_indexes = raw_symbol_indexes_by_id(raw_symbols);
    let representative_raw_symbols = raw_symbol_map(raw_symbols);
    let impacted_ids = impacted_symbol_ids(
        raw_symbols,
        old_changed_symbols,
        new_changed_symbols,
        old_resolved_map,
        changed_file_paths,
    );

    let mut resolved_map = old_resolved_map.clone();
    for symbol in old_changed_symbols {
        resolved_map.remove(&symbol.symbol_id);
    }

    for impacted_id in &impacted_ids {
        let Some(raw_symbol) = representative_raw_symbols.get(impacted_id) else {
            resolved_map.remove(impacted_id);
            continue;
        };

        let Some(indexes) = raw_symbol_indexes.get(impacted_id) else {
            continue;
        };

        let mut symbol = symbol_meta_from_indexed(raw_symbol);
        let mut dependencies = BTreeSet::new();
        for index in indexes {
            dependencies.extend(resolve_dependencies_for_symbol(
                &raw_symbols[*index],
                raw_symbols,
                &name_index,
            ));
        }
        symbol.dependencies = dependencies.into_iter().collect();
        resolved_map.insert(impacted_id.clone(), symbol);
    }

    let reference_impacted_paths =
        reference_impacted_paths(old_resolved_map, &resolved_map, &impacted_ids);

    for impacted_path in reference_impacted_paths {
        let callers = resolved_map
            .iter()
            .filter_map(|(caller_path, symbol)| {
                symbol
                    .dependencies
                    .iter()
                    .any(|dependency| dependency == &impacted_path)
                    .then_some(caller_path.clone())
            })
            .collect::<Vec<_>>();

        if let Some(symbol) = resolved_map.get_mut(&impacted_path) {
            symbol.references = callers;
        }
    }

    (resolved_map, impacted_ids)
}

fn reference_impacted_paths(
    old_resolved_map: &BTreeMap<String, SymbolMeta>,
    new_resolved_map: &BTreeMap<String, SymbolMeta>,
    impacted_paths: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut reference_paths = impacted_paths.clone();

    for impacted_path in impacted_paths {
        if let Some(symbol) = old_resolved_map.get(impacted_path) {
            reference_paths.extend(symbol.dependencies.iter().cloned());
            reference_paths.extend(symbol.references.iter().cloned());
        }
        if let Some(symbol) = new_resolved_map.get(impacted_path) {
            reference_paths.extend(symbol.dependencies.iter().cloned());
            reference_paths.extend(symbol.references.iter().cloned());
        }
    }

    reference_paths
}

fn materialize_resolved_symbol_rows(
    raw_symbols: &[IndexedSymbol],
    resolved_map: &BTreeMap<String, SymbolMeta>,
) -> Vec<SymbolMeta> {
    raw_symbols
        .iter()
        .filter_map(|raw_symbol| {
            resolved_map
                .get(&raw_symbol.symbol_id)
                .map(|resolved_symbol| {
                    SymbolMeta::new(SymbolMetaInit {
                        symbol_id: raw_symbol.symbol_id.clone(),
                        semantic_path: raw_symbol.semantic_path.clone(),
                        scope_path: raw_symbol.scope_path.clone(),
                        file_path: raw_symbol.file_path.clone(),
                        node_kind: raw_symbol.node_kind.clone(),
                        origin_type: "workspace_symbol".to_string(),
                        byte_range: raw_symbol.byte_range,
                        signature: raw_symbol.signature.clone(),
                        parameters: raw_symbol.parameters.clone(),
                        return_type: raw_symbol.return_type.clone(),
                        docstring: raw_symbol.docstring.clone(),
                        dependencies: resolved_symbol.dependencies.clone(),
                        references: resolved_symbol.references.clone(),
                    })
                })
        })
        .collect()
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

fn trace_from_symbol(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let symbol = symbol.clone().with_origin_type("trace_root");

    let callers = if matches!(direction, TraceDirection::Callers | TraceDirection::Both) {
        summarize_symbols(resolved_symbols, &symbol.references, None)
    } else {
        Vec::new()
    };

    let callees = if matches!(direction, TraceDirection::Callees | TraceDirection::Both) {
        summarize_symbols(
            resolved_symbols,
            &symbol.dependencies,
            Some(&symbol.file_path),
        )
    } else {
        Vec::new()
    };

    let result = TraceSymbolGraphResult {
        evidence_keys: trace_evidence_keys(&symbol, &callers, &callees),
        symbol,
        callers,
        callees,
        indexed_files,
    };
    result.validate_public_output()?;
    Ok(result)
}

fn trace_neighborhood_from_symbol(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    if max_nodes == 0 {
        return Err(anyhow!("max_nodes must be greater than zero"));
    }

    let root = symbol.clone().with_origin_type("trace_root");
    let resolved_map = resolved_symbol_map(resolved_symbols);

    let mut nodes = vec![TraceSymbolNeighborhoodNode {
        symbol: symbol_summary_from_meta(&root),
        depth: 0,
    }];
    let mut edges = Vec::new();
    let mut queued = BTreeSet::from([root.symbol_id.clone()]);
    let mut edge_keys = BTreeSet::new();
    let mut queue = VecDeque::from([(root.symbol_id.clone(), 0usize)]);
    let mut truncated = false;

    while let Some((symbol_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let Some(current) = resolved_map.get(&symbol_id) else {
            continue;
        };

        for (from_symbol_id, to_symbol_id) in neighborhood_edges_for_symbol(current, &direction) {
            let next_symbol_id = if from_symbol_id == current.symbol_id {
                &to_symbol_id
            } else {
                &from_symbol_id
            };

            let Some(next_symbol) = resolved_map.get(next_symbol_id) else {
                continue;
            };

            if !queued.contains(next_symbol_id) {
                if nodes.len() >= max_nodes {
                    truncated = true;
                    continue;
                }

                queued.insert(next_symbol_id.clone());
                queue.push_back((next_symbol_id.clone(), depth + 1));
                nodes.push(TraceSymbolNeighborhoodNode {
                    symbol: symbol_summary_from_meta(next_symbol),
                    depth: depth + 1,
                });
            }

            let edge_key = (from_symbol_id.clone(), to_symbol_id.clone());
            if edge_keys.insert(edge_key.clone()) {
                edges.push(TraceSymbolNeighborhoodEdge {
                    from_symbol_id: edge_key.0,
                    to_symbol_id: edge_key.1,
                });
            }
        }
    }

    let result = TraceSymbolNeighborhoodResult {
        symbol: root,
        direction,
        max_depth,
        max_nodes,
        truncated,
        indexed_files,
        nodes,
        edges,
    };
    result.validate_public_output()?;
    Ok(result)
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
    let trace = trace_from_symbol(resolved_symbols, indexed_files, symbol, direction.clone())?;
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

fn node_at_byte_offset<'tree>(
    root: Node<'tree>,
    source: &str,
    byte_offset: usize,
) -> Option<Node<'tree>> {
    let (start, end) = if source.is_empty() {
        (0, 0)
    } else if byte_offset < source.len() {
        (byte_offset, byte_offset + 1)
    } else {
        (byte_offset.saturating_sub(1), byte_offset)
    };

    root.named_descendant_for_byte_range(start, end)
        .or_else(|| root.descendant_for_byte_range(start, end))
        .or_else(|| root.named_descendant_for_byte_range(start, start))
        .or_else(|| root.descendant_for_byte_range(start, start))
}

fn choose_symbol_at_location<'a>(
    resolved_symbols: &'a [SymbolMeta],
    file_path: &str,
    symbol_id: &str,
    semantic_path: &str,
    byte_range: (usize, usize),
) -> Option<&'a SymbolMeta> {
    resolved_symbols
        .iter()
        .filter(|symbol| {
            symbol.file_path == file_path
                && symbol.byte_range == byte_range
                && (symbol.symbol_id == symbol_id || symbol.semantic_path == semantic_path)
        })
        .max_by_key(|symbol| resolved_symbol_rank(symbol))
        .or_else(|| {
            resolved_symbols
                .iter()
                .filter(|symbol| {
                    symbol.file_path == file_path
                        && (symbol.symbol_id == symbol_id || symbol.semantic_path == semantic_path)
                })
                .max_by_key(|symbol| resolved_symbol_rank(symbol))
        })
}

fn resolve_symbol_at_position<'a>(
    resolved_symbols: &'a [SymbolMeta],
    file_path: &Path,
    position: &Position,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<&'a SymbolMeta> {
    let normalized_file_path = normalize_path(file_path);
    let source = match file_overrides.and_then(|overrides| overrides.get(&normalized_file_path)) {
        Some(source) => source.clone(),
        None => read_source(file_path)?,
    };
    let document = parse_document(file_path, &source)?;
    let byte_offset = offset_for_position(&source, position)?;
    let node =
        node_at_byte_offset(document.tree.root_node(), &source, byte_offset).ok_or_else(|| {
            anyhow!(
                "position {}:{} does not resolve to a syntax node in {}",
                position.row,
                position.column,
                file_path.display()
            )
        })?;
    let symbol_node = ascend_to_symbol(document.language_id, node).ok_or_else(|| {
        anyhow!(
            "position {}:{} does not resolve to a semantic symbol in {}",
            position.row,
            position.column,
            file_path.display()
        )
    })?;

    let (symbol_id, semantic_path, byte_range) = match document.language_id {
        LanguageId::Python => {
            let semantic_path = semantic_path(symbol_node, &source)?;
            let byte_range = python_display_byte_range(symbol_node);
            (semantic_path.clone(), semantic_path, byte_range)
        }
        LanguageId::C => {
            let semantic_path = c_semantic_path(file_path, symbol_node, &source)?
                .ok_or_else(|| anyhow!("position does not resolve to a C semantic symbol"))?;
            let symbol_id = c_symbol_id_for_node(file_path, symbol_node, &source)?
                .ok_or_else(|| anyhow!("position does not resolve to a C symbol id"))?;
            (
                symbol_id,
                semantic_path,
                (symbol_node.start_byte(), symbol_node.end_byte()),
            )
        }
    };

    choose_symbol_at_location(
        resolved_symbols,
        &normalized_file_path,
        &symbol_id,
        &semantic_path,
        byte_range,
    )
    .ok_or_else(|| {
        anyhow!(
            "symbol at {}:{} not found in workspace index: {}",
            position.row,
            position.column,
            normalized_file_path
        )
    })
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
            direction.clone(),
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
            direction.clone(),
            max_depth,
            max_nodes,
            file_overrides,
        )?);
    }

    let result = SymbolSearchNeighborhoodContextResult { search, contexts };
    result.validate_public_output()?;
    Ok(result)
}

fn symbol_source_text(
    symbol: &SymbolMeta,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<String> {
    if let Some(file_overrides) = file_overrides
        && let Some(source) = file_overrides.get(&symbol.file_path)
    {
        return Ok(source.clone());
    }

    read_source(Path::new(&symbol.file_path))
}

fn read_symbol_result_from_meta(
    symbol: &SymbolMeta,
    indexed_files: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadResult> {
    let source = symbol_source_text(symbol, file_overrides)?;
    let snippet = symbol_source_slice(symbol, &source)?.to_string();
    let start_point = position_from(point_for_offset(&source, symbol.byte_range.0)?);
    let end_point = position_from(point_for_offset(&source, symbol.byte_range.1)?);

    let result = SymbolReadResult {
        indexed_files,
        symbol: symbol_summary_from_meta(symbol),
        source: snippet,
        start_point,
        end_point,
    };
    result.validate_public_output()?;
    Ok(result)
}

fn symbol_source_slice<'a>(symbol: &SymbolMeta, source: &'a str) -> Result<&'a str> {
    if symbol.byte_range.0 > symbol.byte_range.1 {
        return Err(anyhow!(
            "invalid symbol byte range for {}: start byte is after end byte",
            symbol.symbol_id
        ));
    }

    source
        .get(symbol.byte_range.0..symbol.byte_range.1)
        .ok_or_else(|| anyhow!("symbol source range is invalid for {}", symbol.symbol_id))
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
            direction.clone(),
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
            direction.clone(),
            max_depth,
            max_nodes,
            file_overrides,
        )?);
    }

    let result = SymbolListNeighborhoodContextResult { list, contexts };
    result.validate_public_output()?;
    Ok(result)
}

fn normalize_optional_search_filter(value: Option<&str>, field: &str) -> Result<Option<String>> {
    match value {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(anyhow!("{field} must not be blank"));
            }
            Ok(Some(trimmed.to_ascii_lowercase()))
        }
        None => Ok(None),
    }
}

fn symbol_matches_search_filters(
    symbol: &SymbolMeta,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> bool {
    if let Some(file_path_contains) = file_path_contains
        && !symbol
            .file_path
            .to_ascii_lowercase()
            .contains(file_path_contains)
    {
        return false;
    }
    if let Some(node_kind) = node_kind
        && symbol.node_kind.to_ascii_lowercase() != node_kind
    {
        return false;
    }
    true
}

fn trace_evidence_keys(
    symbol: &SymbolMeta,
    callers: &[SymbolSummary],
    callees: &[SymbolSummary],
) -> TraceEvidenceKeys {
    TraceEvidenceKeys {
        symbol: symbol.evidence_key.clone(),
        callers: callers
            .iter()
            .map(|summary| summary.evidence_key.clone())
            .collect(),
        callees: callees
            .iter()
            .map(|summary| summary.evidence_key.clone())
            .collect(),
    }
}

fn neighborhood_edges_for_symbol(
    symbol: &SymbolMeta,
    direction: &TraceDirection,
) -> Vec<(String, String)> {
    let mut edges = Vec::new();

    if matches!(direction, TraceDirection::Callers | TraceDirection::Both) {
        edges.extend(
            symbol
                .references
                .iter()
                .cloned()
                .map(|caller_id| (caller_id, symbol.symbol_id.clone())),
        );
    }
    if matches!(direction, TraceDirection::Callees | TraceDirection::Both) {
        edges.extend(
            symbol
                .dependencies
                .iter()
                .cloned()
                .map(|callee_id| (symbol.symbol_id.clone(), callee_id)),
        );
    }

    edges
}

fn symbol_summary_from_meta(symbol: &SymbolMeta) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: symbol.symbol_id.clone(),
        semantic_path: symbol.semantic_path.clone(),
        scope_path: symbol.scope_path.clone(),
        file_path: symbol.file_path.clone(),
        node_kind: symbol.node_kind.clone(),
        origin_type: symbol.origin_type.clone(),
        byte_range: symbol.byte_range,
        signature: symbol.signature.clone(),
        parameters: symbol.parameters.clone(),
        return_type: symbol.return_type.clone(),
        docstring: symbol.docstring.clone(),
    })
}

fn search_match_detail(
    symbol: &SymbolMeta,
    query: &str,
    normalized_query: &str,
) -> Option<SymbolSearchMatchDetail> {
    let base_name = symbol_base_name(&symbol.semantic_path);
    let normalized_base_name = base_name.to_ascii_lowercase();
    let normalized_symbol_id = symbol.symbol_id.to_ascii_lowercase();
    let normalized_semantic_path = symbol.semantic_path.to_ascii_lowercase();
    let normalized_scope_path = symbol
        .scope_path
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let normalized_file_path = symbol.file_path.to_ascii_lowercase();
    let normalized_node_kind = symbol.node_kind.to_ascii_lowercase();
    let normalized_signature = symbol
        .signature
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let normalized_parameters = symbol.parameters.join(" ").to_ascii_lowercase();
    let normalized_return_type = symbol
        .return_type
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let normalized_docstring = symbol
        .docstring
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();

    let mut matched_fields = Vec::new();
    if normalized_base_name.contains(normalized_query) {
        matched_fields.push("base_name".to_string());
    }
    if normalized_symbol_id.contains(normalized_query) {
        matched_fields.push("symbol_id".to_string());
    }
    if normalized_semantic_path.contains(normalized_query) {
        matched_fields.push("semantic_path".to_string());
    }
    if normalized_scope_path.contains(normalized_query) {
        matched_fields.push("scope_path".to_string());
    }
    if normalized_file_path.contains(normalized_query) {
        matched_fields.push("file_path".to_string());
    }
    if normalized_node_kind.contains(normalized_query) {
        matched_fields.push("node_kind".to_string());
    }
    if normalized_signature.contains(normalized_query) {
        matched_fields.push("signature".to_string());
    }
    if normalized_parameters.contains(normalized_query) {
        matched_fields.push("parameters".to_string());
    }
    if normalized_return_type.contains(normalized_query) {
        matched_fields.push("return_type".to_string());
    }
    if normalized_docstring.contains(normalized_query) {
        matched_fields.push("docstring".to_string());
    }

    if matched_fields.is_empty() {
        return None;
    }

    let exact_query =
        query == symbol.semantic_path || query == symbol.symbol_id || query == base_name;
    let score = if exact_query {
        1000
    } else if normalized_base_name == normalized_query {
        950
    } else if normalized_symbol_id == normalized_query {
        925
    } else if normalized_semantic_path == normalized_query {
        900
    } else if normalized_base_name.starts_with(normalized_query) {
        850
    } else if normalized_semantic_path.starts_with(normalized_query) {
        825
    } else if normalized_symbol_id.starts_with(normalized_query) {
        800
    } else if normalized_file_path.contains(normalized_query) {
        300
    } else if normalized_signature.contains(normalized_query)
        || normalized_parameters.contains(normalized_query)
        || normalized_return_type.contains(normalized_query)
    {
        200
    } else if normalized_docstring.contains(normalized_query) {
        100
    } else {
        400
    };

    Some(SymbolSearchMatchDetail {
        symbol_id: symbol.symbol_id.clone(),
        score,
        matched_fields,
    })
}

fn persist_symbol_index(
    db_path: &Path,
    workspace_root: &Path,
    raw_symbols: &[IndexedSymbol],
    symbols: &[SymbolMeta],
    file_states: &[PersistedFileState],
    indexed_files: usize,
) -> Result<()> {
    let connection = Connection::open(db_path)?;
    ensure_symbol_tables(&connection)?;

    let tx = connection.unchecked_transaction()?;
    persist_symbol_index_metadata(&tx, workspace_root, indexed_files)?;
    tx.execute("DELETE FROM symbols", [])?;
    tx.execute("DELETE FROM file_state", [])?;
    let raw_symbol_rows = raw_symbol_row_map(raw_symbols);
    {
        let mut statement = tx.prepare(
            "INSERT INTO symbols (
                symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, dependencies_json,
                references_json, reference_names_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?;

        for symbol in symbols {
            let raw_symbol = raw_symbol_rows
                .get(&symbol_row_key(symbol))
                .ok_or_else(|| anyhow!("missing raw symbol for {}", symbol.semantic_path))?;
            let (start_byte, end_byte) = persisted_byte_range(symbol)?;
            statement.execute(params![
                symbol.symbol_id,
                symbol.semantic_path,
                symbol.scope_path,
                symbol.file_path,
                symbol.node_kind,
                start_byte,
                end_byte,
                symbol.signature,
                serde_json::to_string(&symbol.parameters)?,
                symbol.return_type,
                symbol.docstring,
                serde_json::to_string(&symbol.dependencies)?,
                serde_json::to_string(&symbol.references)?,
                serde_json::to_string(&reference_names(raw_symbol))?,
            ])?;
        }
    }
    {
        let mut statement =
            tx.prepare("INSERT INTO file_state (file_path, fingerprint) VALUES (?1, ?2)")?;

        for file_state in file_states {
            statement.execute(params![file_state.file_path, file_state.fingerprint as i64])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn persisted_byte_range(symbol: &SymbolMeta) -> Result<(i64, i64)> {
    if symbol.byte_range.0 > symbol.byte_range.1 {
        return Err(anyhow!(
            "invalid byte range for {}: start {} is after end {}",
            symbol.semantic_path,
            symbol.byte_range.0,
            symbol.byte_range.1
        ));
    }

    Ok((
        i64::try_from(symbol.byte_range.0).map_err(|error| {
            anyhow!("invalid start byte for {}: {}", symbol.semantic_path, error)
        })?,
        i64::try_from(symbol.byte_range.1)
            .map_err(|error| anyhow!("invalid end byte for {}: {}", symbol.semantic_path, error))?,
    ))
}

fn persist_symbol_refresh(context: SymbolRefreshPersistence<'_>) -> Result<()> {
    let connection = Connection::open(context.db_path)?;
    ensure_symbol_tables(&connection)?;

    let raw_symbol_rows = raw_symbol_row_map(context.raw_symbols);
    let resolved_symbol_map = resolved_symbol_map(context.symbols);
    let changed_symbols: Vec<_> = context
        .symbols
        .iter()
        .filter(|symbol| context.changed_file_paths.contains(&symbol.file_path))
        .cloned()
        .collect();

    let tx = connection.unchecked_transaction()?;
    persist_symbol_index_metadata(&tx, context.workspace_root, context.indexed_files)?;
    {
        let mut delete_statement = tx.prepare("DELETE FROM symbols WHERE file_path = ?1")?;
        for changed_file_path in context.changed_file_paths {
            delete_statement.execute([changed_file_path])?;
        }
    }

    {
        let mut insert_statement = tx.prepare(
            "INSERT INTO symbols (
                symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, dependencies_json,
                references_json, reference_names_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?;

        for symbol in &changed_symbols {
            let raw_symbol = raw_symbol_rows
                .get(&symbol_row_key(symbol))
                .ok_or_else(|| anyhow!("missing raw symbol for {}", symbol.semantic_path))?;
            let (start_byte, end_byte) = persisted_byte_range(symbol)?;
            insert_statement.execute(params![
                symbol.symbol_id,
                symbol.semantic_path,
                symbol.scope_path,
                symbol.file_path,
                symbol.node_kind,
                start_byte,
                end_byte,
                symbol.signature,
                serde_json::to_string(&symbol.parameters)?,
                symbol.return_type,
                symbol.docstring,
                serde_json::to_string(&symbol.dependencies)?,
                serde_json::to_string(&symbol.references)?,
                serde_json::to_string(&reference_names(raw_symbol))?,
            ])?;
        }
    }

    {
        let mut update_statement = tx.prepare(
            "UPDATE symbols
             SET dependencies_json = ?1, references_json = ?2
             WHERE symbol_id = ?3",
        )?;

        for impacted_path in context.impacted_paths {
            let Some(symbol) = resolved_symbol_map.get(impacted_path) else {
                continue;
            };
            if context.changed_file_paths.contains(&symbol.file_path) {
                continue;
            }
            update_statement.execute(params![
                serde_json::to_string(&symbol.dependencies)?,
                serde_json::to_string(&symbol.references)?,
                symbol.symbol_id,
            ])?;
        }
    }

    for changed_file_path in context.changed_file_paths {
        tx.execute(
            "DELETE FROM file_state WHERE file_path = ?1",
            [changed_file_path],
        )?;
        if let Some(fingerprint) = context.file_states.get(changed_file_path) {
            tx.execute(
                "INSERT INTO file_state (file_path, fingerprint) VALUES (?1, ?2)",
                params![changed_file_path, *fingerprint as i64],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

fn load_symbol_index(db_path: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    if !db_path.exists() {
        return Err(anyhow!("symbol index {} does not exist", db_path.display()));
    }

    let connection = Connection::open(db_path)?;
    require_symbol_index_tables(&connection, db_path)?;
    load_indexed_files_metadata(&connection)?;
    validate_symbol_index_schema_version(&connection, db_path)?;
    ensure_symbol_tables(&connection)?;
    load_symbols_from_connection(&connection)
}

fn load_symbol_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    if !db_path.exists() {
        return Err(anyhow!("symbol index {} does not exist", db_path.display()));
    }

    let connection = Connection::open(db_path)?;
    require_symbol_index_tables(&connection, db_path)?;
    let workspace_root = load_symbol_index_workspace_root(&connection, db_path)?;
    validate_symbol_index_schema_version(&connection, db_path)?;
    ensure_symbol_tables(&connection)?;

    let mut grouped_symbols = load_indexed_symbols_grouped_by_file(&connection)?;
    let original_grouped_symbols = grouped_symbols.clone();
    let mut changed_file_paths = BTreeSet::new();

    for (override_path, override_source) in file_overrides {
        let override_path = normalize_absolute_path(Path::new(override_path))?;
        if !path_is_inside_workspace(&workspace_root, &override_path)?
            || should_skip_index_path(&workspace_root, &override_path)
            || detect_language(&override_path).is_err()
        {
            continue;
        }

        let document = parse_document(&override_path, override_source)?;
        let symbols = index_symbols_from_document(&override_path, override_source, &document)?;
        let normalized_path = normalize_path(&override_path);
        grouped_symbols.insert(normalized_path.clone(), symbols);
        changed_file_paths.insert(normalized_path);
    }

    let mut raw_symbols = grouped_symbols
        .into_values()
        .flat_map(|symbols| symbols.into_iter())
        .collect::<Vec<_>>();
    assign_symbol_ids(&mut raw_symbols)?;

    let (resolved_symbols, indexed_files) = load_symbols_from_connection(&connection)?;
    let old_resolved_map = resolved_symbol_map(&resolved_symbols);
    let old_changed_symbols = original_grouped_symbols
        .iter()
        .filter(|(file_path, _)| changed_file_paths.contains(*file_path))
        .flat_map(|(_, symbols)| symbols.iter().cloned())
        .collect::<Vec<_>>();
    let new_changed_symbols = raw_symbols
        .iter()
        .filter(|symbol| changed_file_paths.contains(&symbol.file_path))
        .cloned()
        .collect::<Vec<_>>();
    let (resolved_map, _) = refresh_resolved_symbol_subgraph(
        &raw_symbols,
        &old_resolved_map,
        &old_changed_symbols,
        &new_changed_symbols,
        &changed_file_paths,
    );

    Ok((
        materialize_resolved_symbol_rows(&raw_symbols, &resolved_map),
        indexed_files,
    ))
}

fn persist_symbol_index_metadata(
    tx: &Transaction<'_>,
    workspace_root: &Path,
    indexed_files: usize,
) -> Result<()> {
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [SYMBOL_INDEX_SCHEMA_VERSION],
    )?;
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [normalize_path(workspace_root)],
    )?;
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('indexed_files', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [indexed_files.to_string()],
    )?;
    Ok(())
}

fn load_symbol_index_workspace_root(connection: &Connection, db_path: &Path) -> Result<PathBuf> {
    let Some(stored_workspace) = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'workspace_root'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Err(anyhow!(
            "missing workspace_root metadata in symbol index {}",
            db_path.display()
        ));
    };

    normalize_absolute_path(Path::new(&stored_workspace))
}

fn validate_symbol_index_schema_version(connection: &Connection, db_path: &Path) -> Result<()> {
    let Some(value) = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Err(anyhow!(
            "missing schema_version metadata in symbol index {}",
            db_path.display()
        ));
    };

    if value != SYMBOL_INDEX_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported symbol index schema_version `{}` in {}; expected `{}`",
            value,
            db_path.display(),
            SYMBOL_INDEX_SCHEMA_VERSION
        ));
    }

    Ok(())
}

fn load_optional_metadata_value(connection: &Connection, key: &str) -> Result<Option<String>> {
    connection
        .query_row("SELECT value FROM metadata WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(Into::into)
}

fn require_symbol_index_tables(connection: &Connection, db_path: &Path) -> Result<()> {
    for table_name in ["metadata", "symbols", "file_state"] {
        if !table_exists(connection, table_name)? {
            return Err(anyhow!(
                "missing symbol index table `{}` in {}",
                table_name,
                db_path.display()
            ));
        }
    }
    require_table_columns(connection, db_path, "metadata", &["key", "value"])?;
    require_table_column_types(
        connection,
        db_path,
        "metadata",
        &[("key", "TEXT"), ("value", "TEXT")],
    )?;
    require_table_columns(
        connection,
        db_path,
        "symbols",
        &[
            "semantic_path",
            "file_path",
            "node_kind",
            "start_byte",
            "end_byte",
            "signature",
            "dependencies_json",
            "references_json",
        ],
    )?;
    require_table_column_types(
        connection,
        db_path,
        "symbols",
        &[
            ("semantic_path", "TEXT"),
            ("file_path", "TEXT"),
            ("node_kind", "TEXT"),
            ("start_byte", "INTEGER"),
            ("end_byte", "INTEGER"),
            ("signature", "TEXT"),
            ("dependencies_json", "TEXT"),
            ("references_json", "TEXT"),
        ],
    )?;
    require_table_columns(
        connection,
        db_path,
        "file_state",
        &["file_path", "fingerprint"],
    )?;
    require_table_column_types(
        connection,
        db_path,
        "file_state",
        &[("file_path", "TEXT"), ("fingerprint", "INTEGER")],
    )?;
    Ok(())
}

fn table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table_name],
            |_| Ok(()),
        )
        .optional()
        .map(|hit| hit.is_some())
        .map_err(Into::into)
}

fn require_table_columns(
    connection: &Connection,
    db_path: &Path,
    table_name: &str,
    required_columns: &[&str],
) -> Result<()> {
    let columns = table_columns(connection, table_name)?;
    for required_column in required_columns {
        if !columns.contains(*required_column) {
            return Err(anyhow!(
                "symbol index table `{}` in {} is missing required column `{}`",
                table_name,
                db_path.display(),
                required_column
            ));
        }
    }
    Ok(())
}

fn require_table_column_types(
    connection: &Connection,
    db_path: &Path,
    table_name: &str,
    required_columns: &[(&str, &str)],
) -> Result<()> {
    let column_types = table_column_types(connection, table_name)?;
    for (column_name, expected_type) in required_columns {
        let actual_type = column_types
            .get(*column_name)
            .map(|value| value.to_ascii_uppercase())
            .unwrap_or_default();
        if actual_type != *expected_type {
            return Err(anyhow!(
                "symbol index table `{}` in {} has incompatible type `{}` for column `{}`; expected `{}`",
                table_name,
                db_path.display(),
                actual_type,
                column_name,
                expected_type
            ));
        }
    }
    Ok(())
}

fn table_columns(connection: &Connection, table_name: &str) -> Result<BTreeSet<String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    let mut names = BTreeSet::new();
    for column in columns {
        names.insert(column?);
    }
    Ok(names)
}

fn table_column_types(
    connection: &Connection,
    table_name: &str,
) -> Result<BTreeMap<String, String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let columns = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })?;
    let mut types = BTreeMap::new();
    for column in columns {
        let (name, column_type) = column?;
        types.insert(name, column_type);
    }
    Ok(types)
}

fn count_table_rows(connection: &Connection, table_name: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");
    let count = connection.query_row(&sql, [], |row| row.get::<_, i64>(0))?;
    usize::try_from(count).map_err(|error| anyhow!("invalid row count in `{table_name}`: {error}"))
}

fn ensure_symbol_tables(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS symbols (
            symbol_id TEXT NOT NULL,
            semantic_path TEXT NOT NULL,
            scope_path TEXT,
            file_path TEXT NOT NULL,
            node_kind TEXT NOT NULL,
            start_byte INTEGER NOT NULL,
            end_byte INTEGER NOT NULL,
            signature TEXT,
            parameters_json TEXT NOT NULL DEFAULT '[]',
            return_type TEXT,
            docstring TEXT,
            dependencies_json TEXT NOT NULL,
            references_json TEXT NOT NULL,
            reference_names_json TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (semantic_path, file_path)
        );
        CREATE TABLE IF NOT EXISTS file_state (
            file_path TEXT PRIMARY KEY,
            fingerprint INTEGER NOT NULL
        );
        ",
    )?;
    ensure_reference_names_column(connection)?;
    ensure_symbol_id_column(connection)?;
    ensure_scope_path_column(connection)?;
    ensure_parameters_json_column(connection)?;
    ensure_return_type_column(connection)?;
    ensure_docstring_column(connection)?;
    ensure_symbols_primary_key_layout(connection)?;
    Ok(())
}

fn validate_symbol_index_workspace(
    connection: &Connection,
    workspace_root: &Path,
    db_path: &Path,
) -> Result<()> {
    let expected_workspace = normalize_path(workspace_root);
    let stored_workspace = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'workspace_root'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    let Some(stored_workspace) = stored_workspace else {
        return Err(anyhow!(
            "missing workspace_root metadata in symbol index {}",
            db_path.display()
        ));
    };

    if stored_workspace != expected_workspace {
        return Err(anyhow!(
            "symbol index {} belongs to workspace {}, not {}",
            db_path.display(),
            stored_workspace,
            expected_workspace
        ));
    }

    Ok(())
}

fn load_file_states(connection: &Connection) -> Result<BTreeMap<String, u64>> {
    let mut statement =
        connection.prepare("SELECT file_path, fingerprint FROM file_state ORDER BY file_path")?;
    let rows = statement.query_map([], |row| {
        Ok((
            nonempty_string_from_row(row, 0, "file_state.file_path")?,
            row.get::<_, i64>(1)? as u64,
        ))
    })?;

    let mut states = BTreeMap::new();
    for row in rows {
        let (file_path, fingerprint) = row?;
        states.insert(file_path, fingerprint);
    }
    Ok(states)
}

fn load_indexed_symbols_grouped_by_file(
    connection: &Connection,
) -> Result<BTreeMap<String, Vec<IndexedSymbol>>> {
    let mut statement = connection.prepare(
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, reference_names_json
         FROM symbols
         ORDER BY file_path, semantic_path",
    )?;
    let rows = statement.query_map([], |row| {
        let parameters_json: String = row.get(8)?;
        let reference_names_json: String = row.get(11)?;
        let parameters: Vec<String> = json_from_column(&parameters_json, 8)?;
        let reference_names =
            string_list_from_json_column(&reference_names_json, 11, "reference_names_json")?;
        let symbol_id = nonempty_string_from_row(row, 0, "symbol_id")?;
        let semantic_path = nonempty_string_from_row(row, 1, "semantic_path")?;
        Ok(IndexedSymbol {
            symbol_id,
            base_name: symbol_base_name(&semantic_path),
            semantic_path,
            scope_path: row.get(2)?,
            file_path: nonempty_string_from_row(row, 3, "file_path")?,
            node_kind: nonempty_string_from_row(row, 4, "node_kind")?,
            byte_range: byte_range_from_row(row, 5, 6)?,
            signature: row.get(7)?,
            parameters,
            return_type: row.get(9)?,
            docstring: row.get(10)?,
            references_by_name: reference_names.into_iter().collect(),
        })
    })?;

    let mut grouped = BTreeMap::new();
    for row in rows {
        let symbol = row?;
        grouped
            .entry(symbol.file_path.clone())
            .or_insert_with(Vec::new)
            .push(symbol);
    }
    Ok(grouped)
}

fn load_symbols_from_connection(connection: &Connection) -> Result<(Vec<SymbolMeta>, usize)> {
    let indexed_files = load_indexed_files_metadata(connection)?;

    let mut statement = connection.prepare(
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, dependencies_json,
                references_json
         FROM symbols",
    )?;
    let rows = statement.query_map([], |row| {
        let parameters_json: String = row.get(8)?;
        let dependencies_json: String = row.get(11)?;
        let references_json: String = row.get(12)?;
        Ok(SymbolMeta::new(SymbolMetaInit {
            symbol_id: nonempty_string_from_row(row, 0, "symbol_id")?,
            semantic_path: nonempty_string_from_row(row, 1, "semantic_path")?,
            scope_path: row.get(2)?,
            file_path: nonempty_string_from_row(row, 3, "file_path")?,
            node_kind: nonempty_string_from_row(row, 4, "node_kind")?,
            origin_type: "workspace_symbol".to_string(),
            byte_range: byte_range_from_row(row, 5, 6)?,
            signature: row.get(7)?,
            parameters: json_from_column(&parameters_json, 8)?,
            return_type: row.get(9)?,
            docstring: row.get(10)?,
            dependencies: string_list_from_json_column(
                &dependencies_json,
                11,
                "dependencies_json",
            )?,
            references: string_list_from_json_column(&references_json, 12, "references_json")?,
        }))
    })?;

    let mut symbols = Vec::new();
    for row in rows {
        symbols.push(row?);
    }

    Ok((symbols, indexed_files))
}

fn nonempty_string_from_row(
    row: &Row<'_>,
    column: usize,
    column_name: &str,
) -> rusqlite::Result<String> {
    let value: String = row.get(column)?;
    if value.trim().is_empty() {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("empty {column_name}"),
            )),
        ));
    }
    Ok(value)
}

fn load_indexed_files_metadata(connection: &Connection) -> Result<usize> {
    let Some(value) = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'indexed_files'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Err(anyhow!("missing indexed_files metadata"));
    };

    value
        .parse::<usize>()
        .map_err(|error| anyhow!("invalid indexed_files metadata `{value}`: {error}"))
}

fn json_from_column<T: DeserializeOwned>(json: &str, column: usize) -> rusqlite::Result<T> {
    serde_json::from_str(json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error))
    })
}

fn string_list_from_json_column(
    json: &str,
    column: usize,
    column_name: &str,
) -> rusqlite::Result<Vec<String>> {
    let values: Vec<String> = json_from_column(json, column)?;
    if values.iter().any(|value| value.trim().is_empty()) {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("empty {column_name} entry"),
            )),
        ));
    }
    Ok(values)
}

fn byte_range_from_row(
    row: &Row<'_>,
    start_column: usize,
    end_column: usize,
) -> rusqlite::Result<(usize, usize)> {
    let start = nonnegative_i64_as_usize(row.get(start_column)?, start_column)?;
    let end = nonnegative_i64_as_usize(row.get(end_column)?, end_column)?;
    if start > end {
        return Err(integer_conversion_error(
            end_column,
            format!("end_byte {end} is before start_byte {start}"),
        ));
    }
    Ok((start, end))
}

fn nonnegative_i64_as_usize(value: i64, column: usize) -> rusqlite::Result<usize> {
    if value < 0 {
        return Err(integer_conversion_error(
            column,
            format!("expected non-negative integer, got {value}"),
        ));
    }
    usize::try_from(value).map_err(|error| integer_conversion_error(column, error.to_string()))
}

fn integer_conversion_error(column: usize, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        Type::Integer,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}

fn ensure_reference_names_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "reference_names_json" {
            return Ok(());
        }
    }

    connection.execute(
        "ALTER TABLE symbols ADD COLUMN reference_names_json TEXT NOT NULL DEFAULT '[]'",
        [],
    )?;
    Ok(())
}

fn ensure_symbol_id_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "symbol_id" {
            return Ok(());
        }
    }

    connection.execute(
        "ALTER TABLE symbols ADD COLUMN symbol_id TEXT NOT NULL DEFAULT ''",
        [],
    )?;
    connection.execute(
        "UPDATE symbols SET symbol_id = semantic_path WHERE symbol_id = ''",
        [],
    )?;
    Ok(())
}

fn ensure_scope_path_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "scope_path" {
            return Ok(());
        }
    }

    connection.execute("ALTER TABLE symbols ADD COLUMN scope_path TEXT", [])?;
    Ok(())
}

fn ensure_parameters_json_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "parameters_json" {
            return Ok(());
        }
    }

    connection.execute(
        "ALTER TABLE symbols ADD COLUMN parameters_json TEXT NOT NULL DEFAULT '[]'",
        [],
    )?;
    Ok(())
}

fn ensure_return_type_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "return_type" {
            return Ok(());
        }
    }

    connection.execute("ALTER TABLE symbols ADD COLUMN return_type TEXT", [])?;
    Ok(())
}

fn ensure_docstring_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "docstring" {
            return Ok(());
        }
    }

    connection.execute("ALTER TABLE symbols ADD COLUMN docstring TEXT", [])?;
    Ok(())
}

fn ensure_symbols_primary_key_layout(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
    })?;

    let mut semantic_path_pk = 0;
    let mut file_path_pk = 0;
    for column in columns {
        let (name, pk_order) = column?;
        match name.as_str() {
            "semantic_path" => semantic_path_pk = pk_order,
            "file_path" => file_path_pk = pk_order,
            _ => {}
        }
    }

    if semantic_path_pk == 1 && file_path_pk == 2 {
        return Ok(());
    }

    if semantic_path_pk == 0 && file_path_pk == 0 {
        return Ok(());
    }

    connection.execute_batch(
        "
        ALTER TABLE symbols RENAME TO symbols_legacy;
        CREATE TABLE symbols (
            symbol_id TEXT NOT NULL,
            semantic_path TEXT NOT NULL,
            scope_path TEXT,
            file_path TEXT NOT NULL,
            node_kind TEXT NOT NULL,
            start_byte INTEGER NOT NULL,
            end_byte INTEGER NOT NULL,
            signature TEXT,
            parameters_json TEXT NOT NULL DEFAULT '[]',
            return_type TEXT,
            docstring TEXT,
            dependencies_json TEXT NOT NULL,
            references_json TEXT NOT NULL,
            reference_names_json TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (semantic_path, file_path)
        );
        INSERT INTO symbols (
            symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
            signature, parameters_json, return_type, docstring, dependencies_json,
            references_json, reference_names_json
        )
        SELECT
            COALESCE(NULLIF(symbol_id, ''), semantic_path),
            semantic_path, scope_path, file_path, node_kind, start_byte, end_byte, signature,
            COALESCE(parameters_json, '[]'), return_type, docstring,
            dependencies_json, references_json,
            COALESCE(reference_names_json, '[]')
        FROM symbols_legacy;
        DROP TABLE symbols_legacy;
        ",
    )?;
    Ok(())
}

fn raw_symbol_map(symbols: &[IndexedSymbol]) -> BTreeMap<String, IndexedSymbol> {
    let mut map = BTreeMap::new();
    for symbol in symbols {
        map.entry(symbol.symbol_id.clone())
            .and_modify(|existing| {
                if indexed_symbol_rank(symbol) > indexed_symbol_rank(existing) {
                    *existing = symbol.clone();
                }
            })
            .or_insert_with(|| symbol.clone());
    }
    map
}

fn raw_symbol_row_map(
    symbols: &[IndexedSymbol],
) -> BTreeMap<(String, String, usize, usize), IndexedSymbol> {
    symbols
        .iter()
        .cloned()
        .map(|symbol| {
            (
                (
                    symbol.semantic_path.clone(),
                    symbol.file_path.clone(),
                    symbol.byte_range.0,
                    symbol.byte_range.1,
                ),
                symbol,
            )
        })
        .collect()
}

fn resolved_symbol_map(symbols: &[SymbolMeta]) -> BTreeMap<String, SymbolMeta> {
    let mut map = BTreeMap::new();
    for symbol in symbols {
        map.entry(symbol.symbol_id.clone())
            .and_modify(|existing| {
                if resolved_symbol_rank(symbol) > resolved_symbol_rank(existing) {
                    *existing = symbol.clone();
                }
            })
            .or_insert_with(|| symbol.clone());
    }
    map
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
        .max_by_key(|symbol| resolved_symbol_rank(symbol))
}

fn reference_names(symbol: &IndexedSymbol) -> Vec<String> {
    symbol.references_by_name.iter().cloned().collect()
}

fn reference_base_name(reference_name: &str) -> String {
    reference_name
        .rsplit('.')
        .next()
        .unwrap_or(reference_name)
        .to_string()
}

fn symbol_row_key(symbol: &SymbolMeta) -> (String, String, usize, usize) {
    (
        symbol.semantic_path.clone(),
        symbol.file_path.clone(),
        symbol.byte_range.0,
        symbol.byte_range.1,
    )
}

fn indexed_symbol_rank(symbol: &IndexedSymbol) -> usize {
    symbol_kind_rank(&symbol.node_kind)
}

fn resolved_symbol_rank(symbol: &SymbolMeta) -> usize {
    symbol_kind_rank(&symbol.node_kind)
}

fn symbol_kind_rank(node_kind: &str) -> usize {
    match node_kind {
        "function_definition" => 3,
        "class_definition" => 3,
        "type_definition" => 2,
        "declaration" => 1,
        _ => 0,
    }
}

fn c_include_context_for_file(file_path: &str) -> Result<CIncludeContext> {
    let path = Path::new(file_path);
    if detect_language(path).ok() != Some(LanguageId::C) {
        return Ok(CIncludeContext::default());
    }

    let mut include_paths = BTreeSet::new();
    let mut visited = BTreeSet::new();
    collect_c_include_closure(path, &mut include_paths, &mut visited)?;

    let companion_source_paths = include_paths
        .iter()
        .filter_map(|include_path| {
            c_companion_source_path(Path::new(include_path))
                .map(|candidate| normalize_path(&candidate))
        })
        .collect();

    Ok(CIncludeContext {
        include_paths,
        companion_source_paths,
    })
}

fn collect_c_include_closure(
    path: &Path,
    include_paths: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    let normalized_path = normalize_path(path);
    if !visited.insert(normalized_path) {
        return Ok(());
    }

    let source = read_source(path)?;
    let document = parse_document(path, &source)?;
    for include_target in c_include_targets(document.tree.root_node(), &source)? {
        let Some(include_path) = resolve_local_c_include(path, &include_target) else {
            continue;
        };
        let normalized_include = normalize_path(&include_path);
        if include_paths.insert(normalized_include) {
            collect_c_include_closure(&include_path, include_paths, visited)?;
        }
    }

    Ok(())
}

fn source_fingerprint(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::{
        IndexedSymbol, PersistedFileState, SKIPPED_WORKSPACE_DIR_NAMES, SymbolMeta,
        SymbolRefreshPersistence, ensure_symbol_tables, persist_symbol_index,
        persist_symbol_refresh, persisted_byte_range, should_skip_dir_name, should_skip_index_path,
    };

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn recognizes_skipped_workspace_directory_names() {
        for name in SKIPPED_WORKSPACE_DIR_NAMES {
            assert!(
                should_skip_dir_name(name),
                "{name} should be skipped during workspace indexing"
            );
            assert!(
                should_skip_dir_name(&name.to_ascii_uppercase()),
                "{name} should be skipped case-insensitively during workspace indexing"
            );
        }

        for name in ["src", "venv-tools", "node_modules_backup", "targeted"] {
            assert!(
                !should_skip_dir_name(name),
                "{name} should not be skipped by partial name matching"
            );
        }
    }

    #[test]
    fn recognizes_skipped_workspace_path_segments() {
        let workspace = temporary_dir();
        let source_path = workspace.join("src").join("helper.py");
        let venv_path = workspace.join(".venv").join("installed.py");
        let similarly_named_path = workspace.join("venv-tools").join("helper.py");
        let sibling_workspace_path = workspace
            .parent()
            .unwrap()
            .join("other-workspace")
            .join(".venv")
            .join("installed.py");

        assert!(!should_skip_index_path(&workspace, &source_path));
        assert!(should_skip_index_path(&workspace, &venv_path));
        assert!(!should_skip_index_path(&workspace, &similarly_named_path));
        assert!(!should_skip_index_path(&workspace, &sibling_workspace_path));
    }

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

        let error = persist_symbol_refresh(SymbolRefreshPersistence {
            db_path: &db_path,
            workspace_root: &workspace,
            raw_symbols: &raw_symbols,
            symbols: &symbols,
            file_states: &file_states,
            changed_file_paths: &changed_file_paths,
            impacted_paths: &impacted_paths,
            indexed_files: 1,
        })
        .expect_err("invalid rows should abort the full refresh transaction");

        assert!(error.to_string().contains("start 8 is after end 4"));
        assert_eq!(indexed_files_metadata(&db_path), "7");
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
}
