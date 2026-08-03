use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::language::normalize_absolute_path;
use crate::model::{
    Position, SymbolContextResult, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, TraceDirection,
};
use crate::symbol_index_workspace::resolve_workspace_symbols_with_overrides_with_timeout;
use crate::symbol_query_execution::{
    read_symbol_at_position_from_symbols_with_deadline,
    read_symbol_context_at_position_from_symbols_with_deadline,
    read_symbol_context_from_symbols_with_deadline,
    read_symbol_discovery_context_at_position_from_symbols_with_deadline,
    read_symbol_discovery_context_from_symbols_with_deadline,
    read_symbol_from_symbols_with_deadline,
    read_symbol_neighborhood_context_at_position_from_symbols_with_deadline,
    read_symbol_neighborhood_context_from_symbols_with_deadline,
};
use crate::symbol_trace::TraceQueryDeadline;

use super::{
    load_normalized_symbol_index_with_overrides_with_timeout,
    load_workspace_symbols_with_overrides_at_path_with_timeout,
};

pub fn read_symbol_with_overrides_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    read_symbol_with_overrides_with_deadline(workspace_root, file_overrides, symbol_path, &deadline)
}

pub(crate) fn read_symbol_with_overrides_with_deadline(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolReadResult> {
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols_with_overrides_with_timeout(
        workspace_root,
        file_overrides,
        timeout_ms,
    )?;
    read_symbol_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        Some(file_overrides),
        deadline,
    )
}

pub fn read_symbol_context_with_overrides_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<SymbolContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    read_symbol_context_with_overrides_with_deadline(
        workspace_root,
        file_overrides,
        symbol_path,
        direction,
        &deadline,
    )
}

pub(crate) fn read_symbol_context_with_overrides_with_deadline(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolContextResult> {
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols_with_overrides_with_timeout(
        workspace_root,
        file_overrides,
        timeout_ms,
    )?;
    read_symbol_context_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        Some(file_overrides),
        deadline,
    )
}

pub fn read_symbol_neighborhood_context_with_overrides_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    read_symbol_neighborhood_context_with_overrides_with_deadline(
        workspace_root,
        file_overrides,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        &deadline,
    )
}

pub(crate) fn read_symbol_neighborhood_context_with_overrides_with_deadline(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolNeighborhoodContextResult> {
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols_with_overrides_with_timeout(
        workspace_root,
        file_overrides,
        timeout_ms,
    )?;
    read_symbol_neighborhood_context_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
        deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_discovery_context_with_overrides_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadDiscoveryContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    read_symbol_discovery_context_with_overrides_with_deadline(
        workspace_root,
        file_overrides,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        &deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_discovery_context_with_overrides_with_deadline(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolReadDiscoveryContextResult> {
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols_with_overrides_with_timeout(
        workspace_root,
        file_overrides,
        timeout_ms,
    )?;
    read_symbol_discovery_context_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
        deadline,
    )
}

pub fn read_symbol_at_position_with_overrides_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    read_symbol_at_position_with_overrides_with_deadline(
        workspace_root,
        file_overrides,
        file_path,
        position,
        &deadline,
    )
}

pub(crate) fn read_symbol_at_position_with_overrides_with_deadline(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolReadResult> {
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (file_path, resolved_symbols, indexed_files) =
        load_workspace_symbols_with_overrides_at_path_with_timeout(
            workspace_root,
            file_overrides,
            file_path,
            timeout_ms,
        )?;
    read_symbol_at_position_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        Some(file_overrides),
        deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_context_at_position_with_overrides_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<SymbolContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    read_symbol_context_at_position_with_overrides_with_deadline(
        workspace_root,
        file_overrides,
        file_path,
        position,
        direction,
        &deadline,
    )
}

pub(crate) fn read_symbol_context_at_position_with_overrides_with_deadline(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolContextResult> {
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (file_path, resolved_symbols, indexed_files) =
        load_workspace_symbols_with_overrides_at_path_with_timeout(
            workspace_root,
            file_overrides,
            file_path,
            timeout_ms,
        )?;
    read_symbol_context_at_position_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        Some(file_overrides),
        deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_neighborhood_context_at_position_with_overrides_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    read_symbol_neighborhood_context_at_position_with_overrides_with_deadline(
        workspace_root,
        file_overrides,
        file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        &deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_neighborhood_context_at_position_with_overrides_with_deadline(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolNeighborhoodContextResult> {
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (file_path, resolved_symbols, indexed_files) =
        load_workspace_symbols_with_overrides_at_path_with_timeout(
            workspace_root,
            file_overrides,
            file_path,
            timeout_ms,
        )?;
    read_symbol_neighborhood_context_at_position_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
        deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_discovery_context_at_position_with_overrides_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadDiscoveryContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    read_symbol_discovery_context_at_position_with_overrides_with_deadline(
        workspace_root,
        file_overrides,
        file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        &deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_discovery_context_at_position_with_overrides_with_deadline(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolReadDiscoveryContextResult> {
    let timeout_ms = deadline.remaining_timeout_ms("workspace symbol loading")?;
    let (file_path, resolved_symbols, indexed_files) =
        load_workspace_symbols_with_overrides_at_path_with_timeout(
            workspace_root,
            file_overrides,
            file_path,
            timeout_ms,
        )?;
    read_symbol_discovery_context_at_position_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
        deadline,
    )
}

pub(crate) fn read_symbol_from_index_with_overrides_with_deadline(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolReadResult> {
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    read_symbol_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        Some(file_overrides),
        deadline,
    )
}

pub(crate) fn read_symbol_context_from_index_with_overrides_with_deadline(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolContextResult> {
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    read_symbol_context_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        Some(file_overrides),
        deadline,
    )
}

pub fn read_symbol_neighborhood_context_from_index_with_overrides_with_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    read_symbol_neighborhood_context_from_index_with_overrides_with_deadline(
        db_path,
        file_overrides,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        &deadline,
    )
}

pub(crate) fn read_symbol_neighborhood_context_from_index_with_overrides_with_deadline(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolNeighborhoodContextResult> {
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    read_symbol_neighborhood_context_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
        deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_discovery_context_from_index_with_overrides_with_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadDiscoveryContextResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    read_symbol_discovery_context_from_index_with_overrides_with_deadline(
        db_path,
        file_overrides,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        &deadline,
    )
}

pub(crate) fn read_symbol_discovery_context_from_index_with_overrides_with_deadline(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolReadDiscoveryContextResult> {
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    read_symbol_discovery_context_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
        deadline,
    )
}

pub(crate) fn read_symbol_at_position_from_index_with_overrides_with_deadline(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolReadResult> {
    let file_path = normalize_absolute_path(file_path)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    read_symbol_at_position_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        Some(file_overrides),
        deadline,
    )
}

pub(crate) fn read_symbol_context_at_position_from_index_with_overrides_with_deadline(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolContextResult> {
    let file_path = normalize_absolute_path(file_path)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    read_symbol_context_at_position_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        Some(file_overrides),
        deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_neighborhood_context_at_position_from_index_with_overrides_with_deadline(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolNeighborhoodContextResult> {
    let file_path = normalize_absolute_path(file_path)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    read_symbol_neighborhood_context_at_position_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
        deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_symbol_discovery_context_at_position_from_index_with_overrides_with_deadline(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    deadline: &TraceQueryDeadline,
) -> Result<SymbolReadDiscoveryContextResult> {
    let file_path = normalize_absolute_path(file_path)?;
    let timeout_ms = deadline.remaining_timeout_ms("index symbol loading")?;
    let (resolved_symbols, indexed_files) =
        load_normalized_symbol_index_with_overrides_with_timeout(
            db_path,
            file_overrides,
            timeout_ms,
        )?;
    read_symbol_discovery_context_at_position_from_symbols_with_deadline(
        &resolved_symbols,
        indexed_files,
        &file_path,
        position,
        direction,
        max_depth,
        max_nodes,
        Some(file_overrides),
        deadline,
    )
}
