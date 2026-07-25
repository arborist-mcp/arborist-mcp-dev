use std::path::Path;

use anyhow::Result;

use super::super::{SourceQueryRoot, with_source_query_context};
use crate::model::*;

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
