use arborist_core::{
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
};
use pyo3::prelude::*;

use super::SymbolQueryContext;
use crate::{
    ArboristCore, NeighborhoodBounds, parse_direction, source_position, to_json_result, to_py_error,
};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (workspace_root, symbol_path, index_db_path=None, file_path=None, source=None))]
    fn read_symbol_json(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        self.read_symbol_json_impl(
            workspace_root,
            symbol_path,
            index_db_path,
            file_path,
            source,
        )
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, source=None, index_db_path=None))]
    fn read_symbol_at_position_json(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        source: Option<String>,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        self.read_symbol_at_position_json_impl(
            workspace_root,
            file_path,
            row,
            column,
            source,
            index_db_path,
        )
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", index_db_path=None, file_path=None, source=None))]
    fn read_symbol_context_json(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        self.read_symbol_context_json_impl(
            workspace_root,
            symbol_path,
            direction,
            index_db_path,
            file_path,
            source,
        )
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, direction="both", source=None, index_db_path=None))]
    #[allow(clippy::too_many_arguments)]
    fn read_symbol_context_at_position_json(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        direction: &str,
        source: Option<String>,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        self.read_symbol_context_at_position_json_impl(
            workspace_root,
            file_path,
            row,
            column,
            direction,
            source,
            index_db_path,
        )
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path=None, source=None))]
    #[allow(clippy::too_many_arguments)]
    fn read_symbol_neighborhood_context_json(
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
        self.read_symbol_neighborhood_context_json_impl(
            workspace_root,
            symbol_path,
            direction,
            NeighborhoodBounds::new(max_depth, max_nodes),
            index_db_path,
            file_path,
            source,
        )
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, direction="both", max_depth=2, max_nodes=64, source=None, index_db_path=None))]
    #[allow(clippy::too_many_arguments)]
    fn read_symbol_neighborhood_context_at_position_json(
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
        self.read_symbol_neighborhood_context_at_position_json_impl(
            workspace_root,
            file_path,
            row,
            column,
            direction,
            NeighborhoodBounds::new(max_depth, max_nodes),
            source,
            index_db_path,
        )
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path=None, source=None))]
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
    ) -> PyResult<String> {
        self.read_symbol_discovery_context_json_impl(
            workspace_root,
            symbol_path,
            direction,
            NeighborhoodBounds::new(max_depth, max_nodes),
            index_db_path,
            file_path,
            source,
        )
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, direction="both", max_depth=2, max_nodes=64, source=None, index_db_path=None))]
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
        )
    }
}

impl ArboristCore {
    pub(crate) fn read_symbol_json_impl(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let context = SymbolQueryContext::new(workspace_root, index_db_path, file_path, source);
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => read_symbol_from_index_with_source(
                index_db_path,
                context.source_file_path()?,
                source,
                symbol_path,
            ),
            (Some(source), None) => read_symbol_with_source(
                context.workspace_root(),
                context.source_file_path()?,
                source,
                symbol_path,
            ),
            (None, Some(index_db_path)) => read_symbol_from_index(index_db_path, symbol_path),
            (None, None) => self
                .vfs
                .borrow_mut()
                .read_symbol(context.workspace_root(), symbol_path),
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
        let context = SymbolQueryContext::new(
            workspace_root,
            index_db_path,
            Some(file_path.to_string()),
            source,
        );
        let position = source_position(row, column);
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => read_symbol_at_position_from_index_with_source(
                index_db_path,
                context.position_file_path()?,
                source,
                &position,
            ),
            (Some(source), None) => read_symbol_at_position_with_source(
                context.workspace_root(),
                context.position_file_path()?,
                source,
                &position,
            ),
            (None, Some(index_db_path)) => read_symbol_at_position_from_index(
                index_db_path,
                context.position_file_path()?,
                &position,
            ),
            (None, None) => self.vfs.borrow_mut().read_symbol_at_position(
                context.workspace_root(),
                context.position_file_path()?,
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
        let context = SymbolQueryContext::new(workspace_root, index_db_path, file_path, source);
        let direction = parse_direction(direction)?;
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => read_symbol_context_from_index_with_source(
                index_db_path,
                context.source_file_path()?,
                source,
                symbol_path,
                direction,
            ),
            (Some(source), None) => read_symbol_context_with_source(
                context.workspace_root(),
                context.source_file_path()?,
                source,
                symbol_path,
                direction,
            ),
            (None, Some(index_db_path)) => {
                read_symbol_context_from_index(index_db_path, symbol_path, direction)
            }
            (None, None) => self.vfs.borrow_mut().read_symbol_context(
                context.workspace_root(),
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
                read_symbol_context_at_position_from_index_with_source(
                    index_db_path,
                    context.position_file_path()?,
                    source,
                    &position,
                    direction,
                )
            }
            (Some(source), None) => read_symbol_context_at_position_with_source(
                context.workspace_root(),
                context.position_file_path()?,
                source,
                &position,
                direction,
            ),
            (None, Some(index_db_path)) => read_symbol_context_at_position_from_index(
                index_db_path,
                context.position_file_path()?,
                &position,
                direction,
            ),
            (None, None) => self.vfs.borrow_mut().read_symbol_context_at_position(
                context.workspace_root(),
                context.position_file_path()?,
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
        bounds: NeighborhoodBounds,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let context = SymbolQueryContext::new(workspace_root, index_db_path, file_path, source);
        let direction = parse_direction(direction)?;
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => {
                read_symbol_neighborhood_context_from_index_with_source(
                    index_db_path,
                    context.source_file_path()?,
                    source,
                    symbol_path,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                )
            }
            (Some(source), None) => read_symbol_neighborhood_context_with_source(
                context.workspace_root(),
                context.source_file_path()?,
                source,
                symbol_path,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
            ),
            (None, Some(index_db_path)) => read_symbol_neighborhood_context_from_index(
                index_db_path,
                symbol_path,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
            ),
            (None, None) => self.vfs.borrow_mut().read_symbol_neighborhood_context(
                context.workspace_root(),
                symbol_path,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
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
        bounds: NeighborhoodBounds,
        source: Option<String>,
        index_db_path: Option<String>,
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
                read_symbol_neighborhood_context_at_position_from_index_with_source(
                    index_db_path,
                    context.position_file_path()?,
                    source,
                    &position,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                )
            }
            (Some(source), None) => read_symbol_neighborhood_context_at_position_with_source(
                context.workspace_root(),
                context.position_file_path()?,
                source,
                &position,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
            ),
            (None, Some(index_db_path)) => read_symbol_neighborhood_context_at_position_from_index(
                index_db_path,
                context.position_file_path()?,
                &position,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
            ),
            (None, None) => self
                .vfs
                .borrow_mut()
                .read_symbol_neighborhood_context_at_position(
                    context.workspace_root(),
                    context.position_file_path()?,
                    &position,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
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
        bounds: NeighborhoodBounds,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> PyResult<String> {
        let context = SymbolQueryContext::new(workspace_root, index_db_path, file_path, source);
        let direction = parse_direction(direction)?;
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => {
                read_symbol_discovery_context_from_index_with_source(
                    index_db_path,
                    context.source_file_path()?,
                    source,
                    symbol_path,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                )
            }
            (Some(source), None) => read_symbol_discovery_context_with_source(
                context.workspace_root(),
                context.source_file_path()?,
                source,
                symbol_path,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
            ),
            (None, Some(index_db_path)) => read_symbol_discovery_context_from_index(
                index_db_path,
                symbol_path,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
            ),
            (None, None) => self.vfs.borrow_mut().read_symbol_discovery_context(
                context.workspace_root(),
                symbol_path,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
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
                read_symbol_discovery_context_at_position_from_index_with_source(
                    index_db_path,
                    context.position_file_path()?,
                    source,
                    &position,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                )
            }
            (Some(source), None) => read_symbol_discovery_context_at_position_with_source(
                context.workspace_root(),
                context.position_file_path()?,
                source,
                &position,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
            ),
            (None, Some(index_db_path)) => read_symbol_discovery_context_at_position_from_index(
                index_db_path,
                context.position_file_path()?,
                &position,
                direction,
                bounds.max_depth,
                bounds.max_nodes,
            ),
            (None, None) => self
                .vfs
                .borrow_mut()
                .read_symbol_discovery_context_at_position(
                    context.workspace_root(),
                    context.position_file_path()?,
                    &position,
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
