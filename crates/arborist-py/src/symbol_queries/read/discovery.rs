use arborist_core::{
    read_symbol_discovery_context_at_position_from_index_with_source_and_timeout,
    read_symbol_discovery_context_at_position_from_index_with_timeout,
    read_symbol_discovery_context_at_position_with_source_and_timeout,
    read_symbol_discovery_context_from_index_with_source_and_timeout,
    read_symbol_discovery_context_from_index_with_timeout,
    read_symbol_discovery_context_with_source_and_timeout,
};
use pyo3::prelude::*;

use super::super::SymbolQueryContext;
use crate::{
    ArboristCore, NeighborhoodBounds, parse_direction, source_position, to_json_result, to_py_error,
};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (workspace_root, symbol_path, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path=None, source=None, timeout_ms=None))]
    #[allow(clippy::too_many_arguments)]
    fn read_symbol_discovery_context_json(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.read_symbol_discovery_context_json_impl(
            workspace_root,
            symbol_path,
            direction,
            NeighborhoodBounds::new(max_depth, max_nodes),
            index_db_path,
            file_path,
            source,
            timeout_ms,
        )
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, direction="both", max_depth=2, max_nodes=64, source=None, index_db_path=None, timeout_ms=None))]
    #[allow(clippy::too_many_arguments)]
    fn read_symbol_discovery_context_at_position_json(
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
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.read_symbol_discovery_context_at_position_json_impl(
            workspace_root,
            file_path,
            row,
            column,
            direction,
            NeighborhoodBounds::new(max_depth, max_nodes),
            source,
            index_db_path,
            timeout_ms,
        )
    }
}

impl ArboristCore {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn read_symbol_discovery_context_json_impl(
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
        let context = SymbolQueryContext::new(workspace_root, index_db_path, file_path, source);
        let direction = parse_direction(direction)?;
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => {
                read_symbol_discovery_context_from_index_with_source_and_timeout(
                    index_db_path,
                    context.source_file_path()?,
                    source,
                    symbol_path,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                )
            }
            (Some(source), None) => read_symbol_discovery_context_with_source_and_timeout(
                context.workspace_root(),
                context.source_file_path()?,
                source,
                symbol_path,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
                timeout_ms,
            ),
            (None, Some(index_db_path)) => read_symbol_discovery_context_from_index_with_timeout(
                index_db_path,
                symbol_path,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
                timeout_ms,
            ),
            (None, None) => self
                .vfs
                .borrow_mut()
                .read_symbol_discovery_context_with_timeout(
                    context.workspace_root(),
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
    pub(crate) fn read_symbol_discovery_context_at_position_json_impl(
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
        let context = SymbolQueryContext::new(
            workspace_root,
            index_db_path,
            Some(file_path.to_string()),
            source,
        );
        let direction = parse_direction(direction)?;
        let position = source_position(row, column);
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => {
                read_symbol_discovery_context_at_position_from_index_with_source_and_timeout(
                    index_db_path,
                    context.position_file_path()?,
                    source,
                    &position,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                )
            }
            (Some(source), None) => {
                read_symbol_discovery_context_at_position_with_source_and_timeout(
                    context.workspace_root(),
                    context.position_file_path()?,
                    source,
                    &position,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                )
            }
            (None, Some(index_db_path)) => {
                read_symbol_discovery_context_at_position_from_index_with_timeout(
                    index_db_path,
                    context.position_file_path()?,
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
                .read_symbol_discovery_context_at_position_with_timeout(
                    context.workspace_root(),
                    context.position_file_path()?,
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
