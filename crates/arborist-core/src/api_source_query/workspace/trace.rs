use std::path::Path;

use anyhow::Result;

use super::super::{SourceQueryRoot, with_source_query_context_with_timeout};
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
    with_source_query_context_with_timeout(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        timeout_ms,
        |context, timeout_ms| {
            context
                .trace_symbol_graph_at_position_with_timeout(path, position, direction, timeout_ms)
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
    with_source_query_context_with_timeout(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        timeout_ms,
        |context, timeout_ms| {
            context.trace_symbol_graph_with_timeout(symbol_path, direction, timeout_ms)
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
    with_source_query_context_with_timeout(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        timeout_ms,
        |context, timeout_ms| {
            context.trace_symbol_neighborhood_at_position_with_timeout(
                path, position, direction, max_depth, max_nodes, timeout_ms,
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
    with_source_query_context_with_timeout(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        timeout_ms,
        |context, timeout_ms| {
            context.trace_symbol_neighborhood_with_timeout(
                symbol_path,
                direction,
                max_depth,
                max_nodes,
                timeout_ms,
            )
        },
    )
}
