use std::path::Path;

use anyhow::Result;

use super::super::{SourceQueryRoot, with_source_query_context_with_trace_deadline};
use crate::model::*;

pub fn trace_symbol_graph_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_symbol_graph_from_index_with_source_and_timeout(
        db_path,
        path,
        source,
        symbol_path,
        direction,
        None,
    )
}

pub fn trace_symbol_graph_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context.trace_symbol_graph_with_deadline(symbol_path, direction, deadline)
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn trace_symbol_neighborhood_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    trace_symbol_neighborhood_from_index_with_source_and_timeout(
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
pub fn trace_symbol_neighborhood_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context.trace_symbol_neighborhood_with_deadline(
                symbol_path,
                direction,
                max_depth,
                max_nodes,
                deadline,
            )
        },
    )
}

pub fn trace_symbol_graph_at_position_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_symbol_graph_at_position_from_index_with_source_and_timeout(
        db_path, path, source, position, direction, None,
    )
}

pub fn trace_symbol_graph_at_position_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context
                .trace_symbol_graph_at_position_with_deadline(path, position, direction, deadline)
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn trace_symbol_neighborhood_at_position_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    trace_symbol_neighborhood_at_position_from_index_with_source_and_timeout(
        db_path, path, source, position, direction, max_depth, max_nodes, None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn trace_symbol_neighborhood_at_position_from_index_with_source_and_timeout(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Index(db_path),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context.trace_symbol_neighborhood_at_position_with_deadline(
                path, position, direction, max_depth, max_nodes, deadline,
            )
        },
    )
}
