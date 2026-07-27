use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::model::{
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, TraceDirection,
};
use crate::symbol_index_workspace::resolve_workspace_symbols_with_overrides_with_timeout;
use crate::symbol_query_execution::{
    list_context_from_symbols, list_context_from_symbols_with_timeout,
    list_discovery_context_from_symbols_with_timeout, list_from_symbols,
    list_from_symbols_with_timeout, list_neighborhood_context_from_symbols_with_timeout,
};
use crate::symbol_trace::TraceQueryDeadline;

use super::load_normalized_symbol_index_with_overrides_with_timeout;

pub fn list_symbols_with_overrides_filtered(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    list_symbols_with_overrides_filtered_with_timeout(
        workspace_root,
        file_overrides,
        limit,
        file_path_contains,
        node_kind,
        None,
    )
}

pub fn list_symbols_with_overrides_filtered_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols_with_overrides_with_timeout(
        workspace_root,
        file_overrides,
        timeout_ms,
    )?;
    deadline.check("workspace symbol listing")?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol listing")?;
    list_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        timeout_ms,
    )
}

pub fn list_symbols_context_with_overrides_filtered(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListContextResult> {
    list_symbols_context_with_overrides_filtered_with_timeout(
        workspace_root,
        file_overrides,
        limit,
        file_path_contains,
        node_kind,
        None,
    )
}

pub fn list_symbols_context_with_overrides_filtered_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols_with_overrides_with_timeout(
        workspace_root,
        file_overrides,
        timeout_ms,
    )?;
    deadline.check("workspace symbol listing")?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol listing")?;
    list_context_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        Some(file_overrides),
        timeout_ms,
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
    list_symbols_discovery_context_with_overrides_filtered_with_timeout(
        workspace_root,
        file_overrides,
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
pub fn list_symbols_discovery_context_with_overrides_filtered_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListDiscoveryContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols_with_overrides_with_timeout(
        workspace_root,
        file_overrides,
        timeout_ms,
    )?;
    deadline.check("workspace symbol listing")?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol listing")?;
    list_discovery_context_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        Some(file_overrides),
        timeout_ms,
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
    list_symbols_neighborhood_context_with_overrides_filtered_with_timeout(
        workspace_root,
        file_overrides,
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
pub fn list_symbols_neighborhood_context_with_overrides_filtered_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols_with_overrides_with_timeout(
        workspace_root,
        file_overrides,
        timeout_ms,
    )?;
    deadline.check("workspace symbol listing")?;
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol listing")?;
    list_neighborhood_context_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        Some(file_overrides),
        timeout_ms,
    )
}

pub fn list_symbols_from_index_with_overrides_filtered(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    list_symbols_from_index_with_overrides_filtered_with_timeout(
        db_path,
        file_overrides,
        limit,
        file_path_contains,
        node_kind,
        None,
    )
}

pub fn list_symbols_from_index_with_overrides_filtered_with_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    deadline.check("index symbol listing")?;
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
    list_symbols_context_from_index_with_overrides_filtered_with_timeout(
        db_path,
        file_overrides,
        limit,
        file_path_contains,
        node_kind,
        None,
    )
}

pub fn list_symbols_context_from_index_with_overrides_filtered_with_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    deadline.check("index symbol listing")?;
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
    list_symbols_neighborhood_context_from_index_with_overrides_filtered_with_timeout(
        db_path,
        file_overrides,
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
pub fn list_symbols_neighborhood_context_from_index_with_overrides_filtered_with_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    deadline.check("index symbol listing")?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol listing")?;
    list_neighborhood_context_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        Some(file_overrides),
        timeout_ms,
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
    list_symbols_discovery_context_from_index_with_overrides_filtered_with_timeout(
        db_path,
        file_overrides,
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
pub fn list_symbols_discovery_context_from_index_with_overrides_filtered_with_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListDiscoveryContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    deadline.check("index symbol listing")?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol listing")?;
    list_discovery_context_from_symbols_with_timeout(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        Some(file_overrides),
        timeout_ms,
    )
}
