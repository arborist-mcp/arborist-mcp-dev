use std::path::Path;

use anyhow::Result;

use crate::model::*;
use crate::symbol_query::SymbolQueryContext;

#[derive(Debug, Clone, Copy)]
enum SourceQueryRoot<'a> {
    Workspace(&'a Path),
    Index(&'a Path),
}

fn with_source_query_context<T>(
    root: SourceQueryRoot<'_>,
    path: &Path,
    source: &str,
    query: impl FnOnce(&SymbolQueryContext) -> Result<T>,
) -> Result<T> {
    let context = match root {
        SourceQueryRoot::Workspace(workspace_root) => SymbolQueryContext::workspace(workspace_root),
        SourceQueryRoot::Index(db_path) => SymbolQueryContext::index(db_path),
    }?
    .with_source_overlay(path, source)?;
    query(&context)
}

pub fn trace_symbol_graph_at_position_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| context.trace_symbol_graph_at_position(path, position, direction),
    )
}

pub fn trace_symbol_graph_with_source(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| context.trace_symbol_graph(symbol_path, direction),
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
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.trace_symbol_neighborhood_at_position(
                path, position, direction, max_depth, max_nodes,
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
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| context.trace_symbol_neighborhood(symbol_path, direction, max_depth, max_nodes),
    )
}

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

pub fn search_symbols_with_source_filtered(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| context.search_symbols(query, limit, file_path_contains, node_kind),
    )
}

pub fn search_symbols_context_with_source_filtered(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| context.search_symbols_context(query, limit, file_path_contains, node_kind),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_neighborhood_context_with_source_filtered(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchNeighborhoodContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.search_symbols_neighborhood_context(
                query,
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains,
                node_kind,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_discovery_context_with_source_filtered(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchDiscoveryContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.search_symbols_discovery_context(
                query,
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains,
                node_kind,
            )
        },
    )
}

pub fn list_symbols_with_source_filtered(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| context.list_symbols(limit, file_path_contains, node_kind),
    )
}

pub fn list_symbols_context_with_source_filtered(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| context.list_symbols_context(limit, file_path_contains, node_kind),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_neighborhood_context_with_source_filtered(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListNeighborhoodContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.list_symbols_neighborhood_context(
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains,
                node_kind,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_discovery_context_with_source_filtered(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListDiscoveryContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.list_symbols_discovery_context(
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains,
                node_kind,
            )
        },
    )
}

pub fn trace_symbol_graph_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.trace_symbol_graph(symbol_path, direction)
    })
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
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.trace_symbol_neighborhood(symbol_path, direction, max_depth, max_nodes)
    })
}

pub fn trace_symbol_graph_at_position_from_index_with_source(
    db_path: &Path,
    path: &Path,
    source: &str,
    position: &Position,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.trace_symbol_graph_at_position(path, position, direction)
    })
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
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context
            .trace_symbol_neighborhood_at_position(path, position, direction, max_depth, max_nodes)
    })
}

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
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.read_symbol_neighborhood_context(symbol_path, direction, max_depth, max_nodes)
    })
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
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.read_symbol_neighborhood_context_at_position(
            path, position, direction, max_depth, max_nodes,
        )
    })
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

pub fn search_symbols_from_index_with_source_filtered(
    db_path: &Path,
    path: &Path,
    source: &str,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.search_symbols(query, limit, file_path_contains, node_kind)
    })
}

pub fn search_symbols_context_from_index_with_source_filtered(
    db_path: &Path,
    path: &Path,
    source: &str,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchContextResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.search_symbols_context(query, limit, file_path_contains, node_kind)
    })
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_neighborhood_context_from_index_with_source_filtered(
    db_path: &Path,
    path: &Path,
    source: &str,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchNeighborhoodContextResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.search_symbols_neighborhood_context(
            query,
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_discovery_context_from_index_with_source_filtered(
    db_path: &Path,
    path: &Path,
    source: &str,
    query: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolSearchDiscoveryContextResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.search_symbols_discovery_context(
            query,
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    })
}

pub fn list_symbols_from_index_with_source_filtered(
    db_path: &Path,
    path: &Path,
    source: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.list_symbols(limit, file_path_contains, node_kind)
    })
}

pub fn list_symbols_context_from_index_with_source_filtered(
    db_path: &Path,
    path: &Path,
    source: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListContextResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.list_symbols_context(limit, file_path_contains, node_kind)
    })
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_neighborhood_context_from_index_with_source_filtered(
    db_path: &Path,
    path: &Path,
    source: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListNeighborhoodContextResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.list_symbols_neighborhood_context(
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_discovery_context_from_index_with_source_filtered(
    db_path: &Path,
    path: &Path,
    source: &str,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListDiscoveryContextResult> {
    with_source_query_context(SourceQueryRoot::Index(db_path), path, source, |context| {
        context.list_symbols_discovery_context(
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    })
}
