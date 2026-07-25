use std::path::Path;

use anyhow::Result;

use super::super::{SourceQueryRoot, with_source_query_context};
use crate::model::*;

pub fn read_symbol_at_position_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
) -> Result<SymbolReadResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| context.read_symbol_at_position(path, position),
    )
}

pub fn read_symbol_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
) -> Result<SymbolReadResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| context.read_symbol(symbol_path),
    )
}

pub fn read_symbol_context_at_position_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| context.read_symbol_context_at_position(path, position, direction),
    )
}

pub fn read_symbol_context_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<SymbolContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| context.read_symbol_context(symbol_path, direction),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_neighborhood_context_at_position_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.read_symbol_neighborhood_context_at_position(
                path, position, direction, max_depth, max_nodes,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_neighborhood_context_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolNeighborhoodContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.read_symbol_neighborhood_context(symbol_path, direction, max_depth, max_nodes)
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_discovery_context_at_position_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.read_symbol_discovery_context_at_position(
                path, position, direction, max_depth, max_nodes,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_symbol_discovery_context_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolReadDiscoveryContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.read_symbol_discovery_context(symbol_path, direction, max_depth, max_nodes)
        },
    )
}
