use std::path::Path;

use arborist_core::{
    Position, list_symbols_context_from_index_filtered,
    list_symbols_context_from_index_with_source_filtered,
    list_symbols_context_with_source_filtered, list_symbols_discovery_context_from_index_filtered,
    list_symbols_discovery_context_from_index_with_source_filtered,
    list_symbols_discovery_context_with_source_filtered, list_symbols_from_index_filtered,
    list_symbols_from_index_with_source_filtered,
    list_symbols_neighborhood_context_from_index_filtered,
    list_symbols_neighborhood_context_from_index_with_source_filtered,
    list_symbols_neighborhood_context_with_source_filtered, list_symbols_with_source_filtered,
    read_symbol_at_position_from_index, read_symbol_at_position_from_index_with_source,
    read_symbol_at_position_with_source, read_symbol_context_at_position_from_index,
    read_symbol_context_at_position_from_index_with_source,
    read_symbol_context_at_position_with_source, read_symbol_context_from_index,
    read_symbol_context_from_index_with_source, read_symbol_context_with_source,
    read_symbol_discovery_context_at_position_from_index,
    read_symbol_discovery_context_at_position_from_index_with_source,
    read_symbol_discovery_context_at_position_with_source,
    read_symbol_discovery_context_from_index, read_symbol_discovery_context_from_index_with_source,
    read_symbol_discovery_context_with_source, read_symbol_from_index,
    read_symbol_from_index_with_source, read_symbol_neighborhood_context_at_position_from_index,
    read_symbol_neighborhood_context_at_position_from_index_with_source,
    read_symbol_neighborhood_context_at_position_with_source,
    read_symbol_neighborhood_context_from_index,
    read_symbol_neighborhood_context_from_index_with_source,
    read_symbol_neighborhood_context_with_source, read_symbol_with_source,
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
    trace_symbol_graph_at_position_from_index,
    trace_symbol_graph_at_position_from_index_with_source,
    trace_symbol_graph_at_position_with_source, trace_symbol_graph_from_index,
    trace_symbol_graph_from_index_with_source, trace_symbol_graph_with_source,
    trace_symbol_neighborhood_at_position_from_index,
    trace_symbol_neighborhood_at_position_from_index_with_source,
    trace_symbol_neighborhood_at_position_with_source, trace_symbol_neighborhood_from_index,
    trace_symbol_neighborhood_from_index_with_source, trace_symbol_neighborhood_with_source,
};
use pyo3::prelude::*;

use crate::{ArboristCore, parse_direction, require_source_file_path, to_json_result, to_py_error};

impl ArboristCore {
    pub(super) fn trace_symbol_graph_json_impl(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => trace_symbol_graph_from_index_with_source(
                Path::new(&index_db_path),
                require_source_file_path(file_path.as_deref())?,
                &source,
                symbol_path,
                direction,
            ),
            (Some(source), None) => trace_symbol_graph_with_source(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                symbol_path,
                direction,
            ),
            (None, Some(index_db_path)) => {
                trace_symbol_graph_from_index(Path::new(&index_db_path), symbol_path, direction)
            }
            (None, None) => self.vfs.borrow_mut().trace_symbol_graph(
                Path::new(workspace_root),
                symbol_path,
                direction,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn trace_symbol_neighborhood_json_impl(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                trace_symbol_neighborhood_from_index_with_source(
                    Path::new(&index_db_path),
                    require_source_file_path(file_path.as_deref())?,
                    &source,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => trace_symbol_neighborhood_with_source(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => trace_symbol_neighborhood_from_index(
                Path::new(&index_db_path),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, None) => self.vfs.borrow_mut().trace_symbol_neighborhood(
                Path::new(workspace_root),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    pub(super) fn read_symbol_json_impl(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => read_symbol_from_index_with_source(
                Path::new(&index_db_path),
                require_source_file_path(file_path.as_deref())?,
                &source,
                symbol_path,
            ),
            (Some(source), None) => read_symbol_with_source(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                symbol_path,
            ),
            (None, Some(index_db_path)) => {
                read_symbol_from_index(Path::new(&index_db_path), symbol_path)
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .read_symbol(Path::new(workspace_root), symbol_path),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    pub(super) fn read_symbol_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        source: Option<String>,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => read_symbol_at_position_from_index_with_source(
                Path::new(&index_db_path),
                Path::new(file_path),
                &source,
                &position,
            ),
            (Some(source), None) => read_symbol_at_position_with_source(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
            ),
            (None, Some(index_db_path)) => read_symbol_at_position_from_index(
                Path::new(&index_db_path),
                Path::new(file_path),
                &position,
            ),
            (None, None) => self.vfs.borrow_mut().read_symbol_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &position,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    pub(super) fn read_symbol_context_json_impl(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => read_symbol_context_from_index_with_source(
                Path::new(&index_db_path),
                require_source_file_path(file_path.as_deref())?,
                &source,
                symbol_path,
                direction,
            ),
            (Some(source), None) => read_symbol_context_with_source(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                symbol_path,
                direction,
            ),
            (None, Some(index_db_path)) => {
                read_symbol_context_from_index(Path::new(&index_db_path), symbol_path, direction)
            }
            (None, None) => self.vfs.borrow_mut().read_symbol_context(
                Path::new(workspace_root),
                symbol_path,
                direction,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn read_symbol_context_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        direction: &str,
        source: Option<String>,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                read_symbol_context_at_position_from_index_with_source(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    direction,
                )
            }
            (Some(source), None) => read_symbol_context_at_position_with_source(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                direction,
            ),
            (None, Some(index_db_path)) => read_symbol_context_at_position_from_index(
                Path::new(&index_db_path),
                Path::new(file_path),
                &position,
                direction,
            ),
            (None, None) => self.vfs.borrow_mut().read_symbol_context_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &position,
                direction,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn trace_symbol_graph_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        direction: &str,
        source: Option<String>,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                trace_symbol_graph_at_position_from_index_with_source(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    direction,
                )
            }
            (Some(source), None) => trace_symbol_graph_at_position_with_source(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                direction,
            ),
            (None, Some(index_db_path)) => trace_symbol_graph_at_position_from_index(
                Path::new(&index_db_path),
                Path::new(file_path),
                &position,
                direction,
            ),
            (None, None) => self.vfs.borrow_mut().trace_symbol_graph_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &position,
                direction,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn trace_symbol_neighborhood_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        source: Option<String>,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                trace_symbol_neighborhood_at_position_from_index_with_source(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => trace_symbol_neighborhood_at_position_with_source(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => trace_symbol_neighborhood_at_position_from_index(
                Path::new(&index_db_path),
                Path::new(file_path),
                &position,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, None) => self.vfs.borrow_mut().trace_symbol_neighborhood_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &position,
                direction,
                max_depth,
                max_nodes,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn read_symbol_neighborhood_context_json_impl(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                read_symbol_neighborhood_context_from_index_with_source(
                    Path::new(&index_db_path),
                    require_source_file_path(file_path.as_deref())?,
                    &source,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => read_symbol_neighborhood_context_with_source(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => read_symbol_neighborhood_context_from_index(
                Path::new(&index_db_path),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, None) => self.vfs.borrow_mut().read_symbol_neighborhood_context(
                Path::new(workspace_root),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn read_symbol_neighborhood_context_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        source: Option<String>,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                read_symbol_neighborhood_context_at_position_from_index_with_source(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => read_symbol_neighborhood_context_at_position_with_source(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => read_symbol_neighborhood_context_at_position_from_index(
                Path::new(&index_db_path),
                Path::new(file_path),
                &position,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, None) => self
                .vfs
                .borrow_mut()
                .read_symbol_neighborhood_context_at_position(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    direction,
                    max_depth,
                    max_nodes,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn read_symbol_discovery_context_json_impl(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                read_symbol_discovery_context_from_index_with_source(
                    Path::new(&index_db_path),
                    require_source_file_path(file_path.as_deref())?,
                    &source,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => read_symbol_discovery_context_with_source(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => read_symbol_discovery_context_from_index(
                Path::new(&index_db_path),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, None) => self.vfs.borrow_mut().read_symbol_discovery_context(
                Path::new(workspace_root),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn read_symbol_discovery_context_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        source: Option<String>,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                read_symbol_discovery_context_at_position_from_index_with_source(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => read_symbol_discovery_context_at_position_with_source(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => read_symbol_discovery_context_at_position_from_index(
                Path::new(&index_db_path),
                Path::new(file_path),
                &position,
                direction,
                max_depth,
                max_nodes,
            ),
            (None, None) => self
                .vfs
                .borrow_mut()
                .read_symbol_discovery_context_at_position(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    direction,
                    max_depth,
                    max_nodes,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn search_symbols_json_impl(
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
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => search_symbols_from_index_with_source_filtered(
                Path::new(&index_db_path),
                require_source_file_path(file_path.as_deref())?,
                &source,
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (Some(source), None) => search_symbols_with_source_filtered(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => search_symbols_from_index_filtered(
                Path::new(&index_db_path),
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, None) => self.vfs.borrow_mut().search_symbols_filtered(
                Path::new(workspace_root),
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
    pub(super) fn search_symbols_context_json_impl(
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
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                search_symbols_context_from_index_with_source_filtered(
                    Path::new(&index_db_path),
                    require_source_file_path(file_path.as_deref())?,
                    &source,
                    query,
                    limit,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                )
            }
            (Some(source), None) => search_symbols_context_with_source_filtered(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => search_symbols_context_from_index_filtered(
                Path::new(&index_db_path),
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, None) => self.vfs.borrow_mut().search_symbols_context_filtered(
                Path::new(workspace_root),
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
    pub(super) fn search_symbols_neighborhood_context_json_impl(
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
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                search_symbols_neighborhood_context_from_index_with_source_filtered(
                    Path::new(&index_db_path),
                    require_source_file_path(file_path.as_deref())?,
                    &source,
                    query,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                )
            }
            (Some(source), None) => search_symbols_neighborhood_context_with_source_filtered(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                query,
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => search_symbols_neighborhood_context_from_index_filtered(
                Path::new(&index_db_path),
                query,
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
                .search_symbols_neighborhood_context_filtered(
                    Path::new(workspace_root),
                    query,
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
    pub(super) fn search_symbols_discovery_context_json_impl(
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
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                search_symbols_discovery_context_from_index_with_source_filtered(
                    Path::new(&index_db_path),
                    require_source_file_path(file_path.as_deref())?,
                    &source,
                    query,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                )
            }
            (Some(source), None) => search_symbols_discovery_context_with_source_filtered(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                query,
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            (None, Some(index_db_path)) => search_symbols_discovery_context_from_index_filtered(
                Path::new(&index_db_path),
                query,
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
                .search_symbols_discovery_context_filtered(
                    Path::new(workspace_root),
                    query,
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
    pub(super) fn list_symbols_json_impl(
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
    pub(super) fn list_symbols_context_json_impl(
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
    pub(super) fn list_symbols_neighborhood_context_json_impl(
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
    pub(super) fn list_symbols_discovery_context_json_impl(
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
