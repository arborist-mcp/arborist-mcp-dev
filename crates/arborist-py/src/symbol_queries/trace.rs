use std::path::Path;

use arborist_core::{
    trace_symbol_graph_at_position_from_index_with_source_and_timeout,
    trace_symbol_graph_at_position_from_index_with_timeout,
    trace_symbol_graph_at_position_with_source_and_timeout,
    trace_symbol_graph_from_index_with_source_and_timeout,
    trace_symbol_graph_from_index_with_timeout, trace_symbol_graph_with_source_and_timeout,
    trace_symbol_neighborhood_at_position_from_index_with_source_and_timeout,
    trace_symbol_neighborhood_at_position_from_index_with_timeout,
    trace_symbol_neighborhood_at_position_with_source_and_timeout,
    trace_symbol_neighborhood_from_index_with_source_and_timeout,
    trace_symbol_neighborhood_from_index_with_timeout,
    trace_symbol_neighborhood_with_source_and_timeout,
};
use pyo3::prelude::*;

use crate::{
    ArboristCore, NeighborhoodBounds, parse_direction, require_source_file_path, source_position,
    to_json_result, to_py_error,
};

impl ArboristCore {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn trace_symbol_graph_json_impl(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                trace_symbol_graph_from_index_with_source_and_timeout(
                    Path::new(&index_db_path),
                    require_source_file_path(file_path.as_deref())?,
                    &source,
                    symbol_path,
                    direction,
                    timeout_ms,
                )
            }
            (Some(source), None) => trace_symbol_graph_with_source_and_timeout(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                symbol_path,
                direction,
                timeout_ms,
            ),
            (None, Some(index_db_path)) => trace_symbol_graph_from_index_with_timeout(
                Path::new(&index_db_path),
                symbol_path,
                direction,
                timeout_ms,
            ),
            (None, None) => self.vfs.borrow_mut().trace_symbol_graph_with_timeout(
                Path::new(workspace_root),
                symbol_path,
                direction,
                timeout_ms,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn trace_symbol_neighborhood_json_impl(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        bounds: NeighborhoodBounds,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                trace_symbol_neighborhood_from_index_with_source_and_timeout(
                    Path::new(&index_db_path),
                    require_source_file_path(file_path.as_deref())?,
                    &source,
                    symbol_path,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                )
            }
            (Some(source), None) => trace_symbol_neighborhood_with_source_and_timeout(
                Path::new(workspace_root),
                require_source_file_path(file_path.as_deref())?,
                &source,
                symbol_path,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
                timeout_ms,
            ),
            (None, Some(index_db_path)) => trace_symbol_neighborhood_from_index_with_timeout(
                Path::new(&index_db_path),
                symbol_path,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
                timeout_ms,
            ),
            (None, None) => self
                .vfs
                .borrow_mut()
                .trace_symbol_neighborhood_with_timeout(
                    Path::new(workspace_root),
                    symbol_path,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn trace_symbol_graph_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        direction: &str,
        source: Option<String>,
        index_db_path: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = source_position(row, column);
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                trace_symbol_graph_at_position_from_index_with_source_and_timeout(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    direction,
                    timeout_ms,
                )
            }
            (Some(source), None) => trace_symbol_graph_at_position_with_source_and_timeout(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                direction,
                timeout_ms,
            ),
            (None, Some(index_db_path)) => trace_symbol_graph_at_position_from_index_with_timeout(
                Path::new(&index_db_path),
                Path::new(file_path),
                &position,
                direction,
                timeout_ms,
            ),
            (None, None) => self
                .vfs
                .borrow_mut()
                .trace_symbol_graph_at_position_with_timeout(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    direction,
                    timeout_ms,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn trace_symbol_neighborhood_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        direction: &str,
        bounds: NeighborhoodBounds,
        source: Option<String>,
        index_db_path: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = source_position(row, column);
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                trace_symbol_neighborhood_at_position_from_index_with_source_and_timeout(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                )
            }
            (Some(source), None) => trace_symbol_neighborhood_at_position_with_source_and_timeout(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
                timeout_ms,
            ),
            (None, Some(index_db_path)) => {
                trace_symbol_neighborhood_at_position_from_index_with_timeout(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &position,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .trace_symbol_neighborhood_at_position_with_timeout(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
