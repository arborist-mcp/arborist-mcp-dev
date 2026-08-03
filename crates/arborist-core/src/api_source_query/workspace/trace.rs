use std::path::Path;

use anyhow::Result;

use super::super::{SourceQueryRoot, with_source_query_context_with_trace_deadline};
use crate::model::*;

pub fn trace_symbol_graph_at_position_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_symbol_graph_at_position_with_source_and_timeout(
        workspace_root,
        path,
        source,
        position,
        direction,
        None,
    )
}

pub fn trace_symbol_graph_at_position_with_source_and_timeout(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context
                .trace_symbol_graph_at_position_with_deadline(path, position, direction, deadline)
        },
    )
}

pub fn trace_symbol_graph_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_symbol_graph_with_source_and_timeout(
        workspace_root,
        path,
        source,
        symbol_path,
        direction,
        None,
    )
}

pub fn trace_symbol_graph_with_source_and_timeout(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        timeout_ms,
        |context, deadline| {
            context.trace_symbol_graph_with_deadline(symbol_path, direction, deadline)
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn trace_symbol_neighborhood_at_position_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    trace_symbol_neighborhood_at_position_with_source_and_timeout(
        workspace_root,
        path,
        source,
        position,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn trace_symbol_neighborhood_at_position_with_source_and_timeout(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Workspace(workspace_root),
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

#[allow(clippy::too_many_arguments)]
pub fn trace_symbol_neighborhood_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    trace_symbol_neighborhood_with_source_and_timeout(
        workspace_root,
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
pub fn trace_symbol_neighborhood_with_source_and_timeout(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    with_source_query_context_with_trace_deadline(
        SourceQueryRoot::Workspace(workspace_root),
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
