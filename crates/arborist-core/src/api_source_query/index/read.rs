use std::path::Path;

use anyhow::Result;

use super::super::{
    SourceQueryRoot, with_source_query_context, with_source_query_context_with_timeout,
};
use crate::model::*;

pub fn read_symbol_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
) -> Result<SymbolReadResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.read_symbol(symbol_path)
    })
}
pub fn read_symbol_context_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.read_symbol_context(symbol_path, direction)
    })
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
    with_source_query_context_with_timeout(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, timeout_ms| {
            context.read_symbol_neighborhood_context_with_timeout(
                symbol_path,
                direction,
                max_depth,
                max_nodes,
                timeout_ms,
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
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.read_symbol_discovery_context(symbol_path, direction, max_depth, max_nodes)
    })
}
pub fn read_symbol_at_position_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
) -> Result<SymbolReadResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.read_symbol_at_position(path, position)
    })
}
pub fn read_symbol_context_at_position_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.read_symbol_context_at_position(path, position, direction)
    })
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
    with_source_query_context_with_timeout(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, timeout_ms| {
            context.read_symbol_neighborhood_context_at_position_with_timeout(
                path, position, direction, max_depth, max_nodes, timeout_ms,
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
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.read_symbol_discovery_context_at_position(
            path, position, direction, max_depth, max_nodes,
        )
    })
}
