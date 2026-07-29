use arborist_core::{
    validate_patch_with_graph_context_at_position_from_index_path_with_timeout,
    validate_patch_with_graph_context_at_position_from_index_with_timeout,
    validate_patch_with_graph_context_at_position_with_timeout,
    validate_patch_with_graph_context_from_index_path_with_timeout,
    validate_patch_with_graph_context_from_index_with_timeout,
    validate_patch_with_graph_context_with_timeout,
};
use pyo3::prelude::*;

use crate::symbol_queries::SymbolQueryContext;
use crate::{
    ArboristCore, NeighborhoodBounds, parse_direction, source_position, to_json_result, to_py_error,
};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (workspace_root, file_path, semantic_path, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64, index_db_path=None, timeout_ms=None))]
    #[allow(clippy::too_many_arguments)]
    fn validate_patch_with_graph_context_json(
        &self,
        workspace_root: &str,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.validate_patch_with_graph_context_json_impl(
            workspace_root,
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            direction,
            NeighborhoodBounds::new(max_depth, max_nodes),
            index_db_path,
            timeout_ms,
        )
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64, index_db_path=None, timeout_ms=None))]
    #[allow(clippy::too_many_arguments)]
    fn validate_patch_with_graph_context_at_position_json(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.validate_patch_with_graph_context_at_position_json_impl(
            workspace_root,
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
            direction,
            NeighborhoodBounds::new(max_depth, max_nodes),
            index_db_path,
            timeout_ms,
        )
    }
}

impl ArboristCore {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate_patch_with_graph_context_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        bounds: NeighborhoodBounds,
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
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_graph_context_from_index_with_timeout(
                    index_db_path,
                    context.required_file_path()?,
                    source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                )
            }
            (Some(source), None) => validate_patch_with_graph_context_with_timeout(
                context.workspace_root(),
                context.required_file_path()?,
                source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                bounds.max_depth,
                bounds.max_nodes,
                timeout_ms,
            ),
            (None, Some(index_db_path)) => {
                validate_patch_with_graph_context_from_index_path_with_timeout(
                    index_db_path,
                    context.required_file_path()?,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_graph_context_with_timeout(
                    context.workspace_root(),
                    context.required_file_path()?,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
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
    pub(crate) fn validate_patch_with_graph_context_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        bounds: NeighborhoodBounds,
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
                validate_patch_with_graph_context_at_position_from_index_with_timeout(
                    index_db_path,
                    context.position_file_path()?,
                    source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                )
            }
            (Some(source), None) => validate_patch_with_graph_context_at_position_with_timeout(
                context.workspace_root(),
                context.position_file_path()?,
                source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                direction,
                bounds.max_depth,
                bounds.max_nodes,
                timeout_ms,
            ),
            (None, Some(index_db_path)) => {
                validate_patch_with_graph_context_at_position_from_index_path_with_timeout(
                    index_db_path,
                    context.position_file_path()?,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    bounds.max_depth,
                    bounds.max_nodes,
                    timeout_ms,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_graph_context_at_position_with_timeout(
                    context.workspace_root(),
                    context.position_file_path()?,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
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
