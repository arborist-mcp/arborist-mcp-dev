use std::path::Path;

use arborist_core::{
    list_symbols_context_from_index_filtered, list_symbols_context_from_index_with_source_filtered,
    list_symbols_context_with_source_filtered, list_symbols_discovery_context_from_index_filtered,
    list_symbols_discovery_context_from_index_with_source_filtered,
    list_symbols_discovery_context_with_source_filtered, list_symbols_from_index_filtered,
    list_symbols_from_index_with_source_filtered,
    list_symbols_neighborhood_context_from_index_filtered,
    list_symbols_neighborhood_context_from_index_with_source_filtered,
    list_symbols_neighborhood_context_with_source_filtered, list_symbols_with_source_filtered,
};
use pyo3::prelude::*;

use crate::{ArboristCore, parse_direction, require_source_file_path, to_json_result, to_py_error};

impl ArboristCore {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn list_symbols_json_impl(
        &self,
        workspace_root: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => list_symbols_from_index_with_source_filtered(
                Path::new(&index_db_path),
                require_source_file_path(file_path.as_deref())?,
                &source,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (Some(source), None) => list_symbols_with_source_filtered(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => list_symbols_from_index_filtered(
                Path::new(&index_db_path),
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, None) => self.vfs.borrow_mut().list_symbols_filtered(
                Path::new(workspace_root),
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn list_symbols_context_json_impl(
        &self,
        workspace_root: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                list_symbols_context_from_index_with_source_filtered(
                    Path::new(&index_db_path),
                    require_source_file_path(file_path.as_deref())?,
                    &source,
                    limit,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                )
            }
            (Some(source), None) => list_symbols_context_with_source_filtered(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => list_symbols_context_from_index_filtered(
                Path::new(&index_db_path),
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, None) => self.vfs.borrow_mut().list_symbols_context_filtered(
                Path::new(workspace_root),
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn list_symbols_neighborhood_context_json_impl(
        &self,
        workspace_root: &str,
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
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                list_symbols_neighborhood_context_from_index_with_source_filtered(
                    Path::new(&index_db_path),
                    require_source_file_path(file_path.as_deref())?,
                    &source,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                )
            }
            (Some(source), None) => list_symbols_neighborhood_context_with_source_filtered(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => list_symbols_neighborhood_context_from_index_filtered(
                Path::new(&index_db_path),
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, None) => self
                .vfs
                .borrow_mut()
                .list_symbols_neighborhood_context_filtered(
                    Path::new(workspace_root),
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn list_symbols_discovery_context_json_impl(
        &self,
        workspace_root: &str,
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
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                list_symbols_discovery_context_from_index_with_source_filtered(
                    Path::new(&index_db_path),
                    require_source_file_path(file_path.as_deref())?,
                    &source,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                )
            }
            (Some(source), None) => list_symbols_discovery_context_with_source_filtered(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => list_symbols_discovery_context_from_index_filtered(
                Path::new(&index_db_path),
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, None) => self
                .vfs
                .borrow_mut()
                .list_symbols_discovery_context_filtered(
                    Path::new(workspace_root),
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
