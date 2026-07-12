use std::path::Path;

use arborist_core::{
    Position, read_symbol_at_position_from_index, read_symbol_at_position_from_index_with_source,
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
};
use pyo3::prelude::*;

use crate::{ArboristCore, parse_direction, require_source_file_path, to_json_result, to_py_error};

impl ArboristCore {
    pub(crate) fn read_symbol_json_impl(
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

    pub(crate) fn read_symbol_at_position_json_impl(
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

    pub(crate) fn read_symbol_context_json_impl(
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
    pub(crate) fn read_symbol_context_at_position_json_impl(
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
    pub(crate) fn read_symbol_neighborhood_context_json_impl(
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
    pub(crate) fn read_symbol_neighborhood_context_at_position_json_impl(
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
    pub(crate) fn read_symbol_discovery_context_json_impl(
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
    pub(crate) fn read_symbol_discovery_context_at_position_json_impl(
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
}
