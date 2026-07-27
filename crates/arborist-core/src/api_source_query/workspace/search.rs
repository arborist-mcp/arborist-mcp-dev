use std::path::Path;

use anyhow::Result;

use super::super::{SourceQueryRoot, with_source_query_context};
use crate::model::*;

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

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_with_source_filtered_with_timeout(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolSearchResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.search_symbols_with_timeout(
                query,
                limit,
                file_path_contains,
                node_kind,
                timeout_ms,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_context_with_source_filtered_with_timeout(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    query: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolSearchContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.search_symbols_context_with_timeout(
                query,
                limit,
                file_path_contains,
                node_kind,
                timeout_ms,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_symbols_neighborhood_context_with_source_filtered_with_timeout(
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
    timeout_ms: Option<u64>,
) -> Result<SymbolSearchNeighborhoodContextResult> {
    with_source_query_context(
        SourceQueryRoot::Workspace(workspace_root),
        path,
        source,
        |context| {
            context.search_symbols_neighborhood_context_with_timeout(
                query,
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains,
                node_kind,
                timeout_ms,
            )
        },
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
