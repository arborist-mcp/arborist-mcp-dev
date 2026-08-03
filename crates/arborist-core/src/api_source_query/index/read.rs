use std::path::Path;

use anyhow::Result;

use super::super::{SourceQueryRoot, with_source_query_context_with_trace_deadline};
use crate::model::*;

pub fn read_symbol_at_position_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
) -> Result<SymbolReadResult> {
    read_symbol_at_position_from_index_with_source_and_timeout(
        db_path, path, source, position, None,
    )
}

pub fn read_symbol_at_position_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| context.read_symbol_at_position_with_deadline(path, position, deadline),
    )
}

pub fn read_symbol_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
) -> Result<SymbolReadResult> {
    read_symbol_from_index_with_source_and_timeout(db_path, path, source, symbol_path, None)
}

pub fn read_symbol_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| context.read_symbol_with_deadline(symbol_path, deadline),
    )
}

pub fn read_symbol_context_at_position_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    read_symbol_context_at_position_from_index_with_source_and_timeout(
        db_path, path, source, position, direction, None,
    )
}

pub fn read_symbol_context_at_position_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<SymbolContextResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context
                .read_symbol_context_at_position_with_deadline(path, position, direction, deadline)
        },
    )
}

pub fn read_symbol_context_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    read_symbol_context_from_index_with_source_and_timeout(
        db_path,
        path,
        source,
        symbol_path,
        direction,
        None,
    )
}

pub fn read_symbol_context_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<SymbolContextResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context.read_symbol_context_with_deadline(symbol_path, direction, deadline)
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_neighborhood_context_at_position_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    read_symbol_neighborhood_context_at_position_from_index_with_source_and_timeout(
        db_path, path, source, position, direction, max_depth, max_nodes, None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_neighborhood_context_at_position_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context.read_symbol_neighborhood_context_at_position_with_deadline(
                path, position, direction, max_depth, max_nodes, deadline,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_neighborhood_context_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    read_symbol_neighborhood_context_from_index_with_source_and_timeout(
        db_path,
        path,
        source,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_neighborhood_context_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolNeighborhoodContextResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context.read_symbol_neighborhood_context_with_deadline(
                symbol_path,
                direction,
                max_depth,
                max_nodes,
                deadline,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_discovery_context_at_position_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    read_symbol_discovery_context_at_position_from_index_with_source_and_timeout(
        db_path, path, source, position, direction, max_depth, max_nodes, None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_discovery_context_at_position_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadDiscoveryContextResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context.read_symbol_discovery_context_at_position_with_deadline(
                path, position, direction, max_depth, max_nodes, deadline,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_discovery_context_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    read_symbol_discovery_context_from_index_with_source_and_timeout(
        db_path,
        path,
        source,
        symbol_path,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_discovery_context_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<SymbolReadDiscoveryContextResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context.read_symbol_discovery_context_with_deadline(
                symbol_path,
                direction,
                max_depth,
                max_nodes,
                deadline,
            )
        },
    )
}
