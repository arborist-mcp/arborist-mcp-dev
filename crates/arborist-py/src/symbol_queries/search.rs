use arborist_core::{
    search_symbols_context_from_index_filtered,
    search_symbols_context_from_index_with_source_filtered,
    search_symbols_context_with_source_filtered,
    search_symbols_discovery_context_from_index_filtered,
    search_symbols_discovery_context_from_index_with_source_filtered,
    search_symbols_discovery_context_with_source_filtered, search_symbols_from_index_filtered,
    search_symbols_from_index_with_source_filtered,
    search_symbols_neighborhood_context_from_index_filtered,
    search_symbols_neighborhood_context_from_index_with_source_filtered,
    search_symbols_neighborhood_context_with_source_filtered, search_symbols_with_source_filtered,
};
use pyo3::prelude::*;

use super::SymbolQueryContext;
use crate::{ArboristCore, NeighborhoodBounds, parse_direction, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (workspace_root, query, limit=20, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
    #[allow(clippy::too_many_arguments)]
    fn search_symbols_json(
        &self,
        workspace_root: &str,
        query: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        self.search_symbols_json_impl(
            workspace_root,
            query,
            limit,
            index_db_path,
            file_path_contains,
            node_kind,
            file_path,
            source,
        )
    }

    #[pyo3(signature = (workspace_root, query, limit=20, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
    #[allow(clippy::too_many_arguments)]
    fn search_symbols_context_json(
        &self,
        workspace_root: &str,
        query: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        self.search_symbols_context_json_impl(
            workspace_root,
            query,
            limit,
            index_db_path,
            file_path_contains,
            node_kind,
            file_path,
            source,
        )
    }

    #[pyo3(signature = (workspace_root, query, limit=20, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
    #[allow(clippy::too_many_arguments)]
    fn search_symbols_neighborhood_context_json(
        &self,
        workspace_root: &str,
        query: &str,
        limit: usize,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        self.search_symbols_neighborhood_context_json_impl(
            workspace_root,
            query,
            limit,
            direction,
            NeighborhoodBounds::new(max_depth, max_nodes),
            index_db_path,
            file_path_contains,
            node_kind,
            file_path,
            source,
        )
    }

    #[pyo3(signature = (workspace_root, query, limit=20, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
    #[allow(clippy::too_many_arguments)]
    fn search_symbols_discovery_context_json(
        &self,
        workspace_root: &str,
        query: &str,
        limit: usize,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        self.search_symbols_discovery_context_json_impl(
            workspace_root,
            query,
            limit,
            direction,
            NeighborhoodBounds::new(max_depth, max_nodes),
            index_db_path,
            file_path_contains,
            node_kind,
            file_path,
            source,
        )
    }
}

impl ArboristCore {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search_symbols_json_impl(
        &self,
        workspace_root: &str,
        query: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let context = SymbolQueryContext::new(workspace_root, index_db_path, file_path, source);
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => search_symbols_from_index_with_source_filtered(
                index_db_path,
                context.source_file_path()?,
                source,
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (Some(source), None) => search_symbols_with_source_filtered(
                context.workspace_root(),
                context.source_file_path()?,
                source,
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => search_symbols_from_index_filtered(
                index_db_path,
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, None) => self.vfs.borrow_mut().search_symbols_filtered(
                context.workspace_root(),
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search_symbols_context_json_impl(
        &self,
        workspace_root: &str,
        query: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let context = SymbolQueryContext::new(workspace_root, index_db_path, file_path, source);
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => {
                search_symbols_context_from_index_with_source_filtered(
                    index_db_path,
                    context.source_file_path()?,
                    source,
                    query,
                    limit,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                )
            }
            (Some(source), None) => search_symbols_context_with_source_filtered(
                context.workspace_root(),
                context.source_file_path()?,
                source,
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => search_symbols_context_from_index_filtered(
                index_db_path,
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, None) => self.vfs.borrow_mut().search_symbols_context_filtered(
                context.workspace_root(),
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search_symbols_neighborhood_context_json_impl(
        &self,
        workspace_root: &str,
        query: &str,
        limit: usize,
        direction: &str,
        bounds: NeighborhoodBounds,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let context = SymbolQueryContext::new(workspace_root, index_db_path, file_path, source);
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => {
                search_symbols_neighborhood_context_from_index_with_source_filtered(
                    index_db_path,
                    context.source_file_path()?,
                    source,
                    query,
                    limit,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                )
            }
            (Some(source), None) => search_symbols_neighborhood_context_with_source_filtered(
                context.workspace_root(),
                context.source_file_path()?,
                source,
                query,
                limit,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => search_symbols_neighborhood_context_from_index_filtered(
                index_db_path,
                query,
                limit,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, None) => self
                .vfs
                .borrow_mut()
                .search_symbols_neighborhood_context_filtered(
                    context.workspace_root(),
                    query,
                    limit,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search_symbols_discovery_context_json_impl(
        &self,
        workspace_root: &str,
        query: &str,
        limit: usize,
        direction: &str,
        bounds: NeighborhoodBounds,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let context = SymbolQueryContext::new(workspace_root, index_db_path, file_path, source);
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => {
                search_symbols_discovery_context_from_index_with_source_filtered(
                    index_db_path,
                    context.source_file_path()?,
                    source,
                    query,
                    limit,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                )
            }
            (Some(source), None) => search_symbols_discovery_context_with_source_filtered(
                context.workspace_root(),
                context.source_file_path()?,
                source,
                query,
                limit,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => search_symbols_discovery_context_from_index_filtered(
                index_db_path,
                query,
                limit,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, None) => self
                .vfs
                .borrow_mut()
                .search_symbols_discovery_context_filtered(
                    context.workspace_root(),
                    query,
                    limit,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
