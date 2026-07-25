use std::path::Path;

use anyhow::Result;

use super::super::{SourceQueryRoot, with_source_query_context};
use crate::model::*;

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
