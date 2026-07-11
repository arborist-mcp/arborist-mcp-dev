mod json_args;

use std::cell::RefCell;
use std::path::Path;

use arborist_core::{
    PatchAstNodeResult, Position, PositionEdit, TraceDirection, TraceSymbolGraphResult,
    VirtualFileSystem, WorkspaceScanLimits, execute_tree_query_from_path_with_limit,
    execute_tree_query_with_limit, get_semantic_skeleton, get_semantic_skeleton_from_path,
    inspect_symbol_index, list_symbols_context_from_index_filtered,
    list_symbols_context_from_index_with_source_filtered,
    list_symbols_context_with_source_filtered, list_symbols_discovery_context_from_index_filtered,
    list_symbols_discovery_context_from_index_with_source_filtered,
    list_symbols_discovery_context_with_source_filtered, list_symbols_from_index_filtered,
    list_symbols_from_index_with_source_filtered,
    list_symbols_neighborhood_context_from_index_filtered,
    list_symbols_neighborhood_context_from_index_with_source_filtered,
    list_symbols_neighborhood_context_with_source_filtered, list_symbols_with_source_filtered,
    patch_ast_node, patch_ast_node_at_position, preview_patch_ast_node,
    preview_patch_ast_node_at_position, preview_patch_ast_node_at_position_from_path,
    preview_patch_ast_node_from_path, read_symbol_at_position_from_index,
    read_symbol_at_position_from_index_with_source, read_symbol_at_position_with_source,
    read_symbol_context_at_position_from_index,
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
    rebuild_symbol_index_with_limits, refresh_symbol_index_for_file_with_limits,
    replay_patch_evidence_against_trace, search_symbols_context_from_index_filtered,
    search_symbols_context_from_index_with_source_filtered,
    search_symbols_context_with_source_filtered,
    search_symbols_discovery_context_from_index_filtered,
    search_symbols_discovery_context_from_index_with_source_filtered,
    search_symbols_discovery_context_with_source_filtered, search_symbols_from_index_filtered,
    search_symbols_from_index_with_source_filtered,
    search_symbols_neighborhood_context_from_index_filtered,
    search_symbols_neighborhood_context_from_index_with_source_filtered,
    search_symbols_neighborhood_context_with_source_filtered, search_symbols_with_source_filtered,
    supported_languages, trace_symbol_graph_at_position_from_index,
    trace_symbol_graph_at_position_from_index_with_source,
    trace_symbol_graph_at_position_with_source, trace_symbol_graph_from_index,
    trace_symbol_graph_from_index_with_source, trace_symbol_graph_with_source,
    trace_symbol_neighborhood_at_position_from_index,
    trace_symbol_neighborhood_at_position_from_index_with_source,
    trace_symbol_neighborhood_at_position_with_source, trace_symbol_neighborhood_from_index,
    trace_symbol_neighborhood_from_index_with_source, trace_symbol_neighborhood_with_source,
    validate_patch_commit_with_trace, validate_patch_with_discovery_context,
    validate_patch_with_discovery_context_at_position,
    validate_patch_with_discovery_context_at_position_from_index,
    validate_patch_with_discovery_context_from_index, validate_patch_with_graph_context,
    validate_patch_with_graph_context_at_position,
    validate_patch_with_graph_context_at_position_from_index,
    validate_patch_with_graph_context_from_index, validate_patch_with_neighborhood_context,
    validate_patch_with_neighborhood_context_at_position,
    validate_patch_with_neighborhood_context_at_position_from_index,
    validate_patch_with_neighborhood_context_from_index, validate_patch_with_trace_context,
    validate_patch_with_trace_context_at_position,
    validate_patch_with_trace_context_at_position_from_index,
    validate_patch_with_trace_context_from_index,
};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use crate::json_args::parse_json_arg;
use serde::Serialize;

#[pyclass(unsendable)]
struct ArboristCore {
    vfs: RefCell<VirtualFileSystem>,
}

#[pymethods]
impl ArboristCore {
    #[new]
    fn new() -> Self {
        Self {
            vfs: RefCell::new(VirtualFileSystem::new()),
        }
    }

    fn supported_languages(&self) -> Vec<String> {
        supported_languages()
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    #[pyo3(signature = (file_path, source=None, depth_limit=2, expand_nodes=None))]
    fn get_semantic_skeleton_json(
        &self,
        file_path: &str,
        source: Option<String>,
        depth_limit: usize,
        expand_nodes: Option<Vec<String>>,
    ) -> PyResult<String> {
        let expand_nodes = expand_nodes.unwrap_or_default();
        let result = match source {
            Some(source) => {
                get_semantic_skeleton(Path::new(file_path), &source, depth_limit, &expand_nodes)
            }
            None => {
                get_semantic_skeleton_from_path(Path::new(file_path), depth_limit, &expand_nodes)
            }
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (file_path, query, source=None, max_captures=10_000))]
    fn execute_tree_query_json(
        &self,
        file_path: &str,
        query: &str,
        source: Option<String>,
        max_captures: usize,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => {
                execute_tree_query_with_limit(Path::new(file_path), &source, query, max_captures)
            }
            None => {
                execute_tree_query_from_path_with_limit(Path::new(file_path), query, max_captures)
            }
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (file_path, semantic_path, new_code, source=None, bypass_reason=None))]
    fn patch_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => patch_ast_node(
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
            ),
            None => {
                let mut vfs = self.vfs.borrow_mut();
                let result = vfs
                    .patch_node(
                        Path::new(file_path),
                        semantic_path,
                        new_code,
                        bypass_reason.as_deref(),
                    )
                    .map_err(to_py_error)?;
                if result.applied {
                    vfs.commit_file(Path::new(file_path)).map_err(to_py_error)?;
                }
                Ok(result)
            }
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (file_path, row, column, new_code, source=None, bypass_reason=None))]
    fn patch_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let position = Position { row, column };
        let result = match source {
            Some(source) => patch_ast_node_at_position(
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
            ),
            None => {
                let mut vfs = self.vfs.borrow_mut();
                let result = vfs
                    .patch_node_at_position(
                        Path::new(file_path),
                        &position,
                        new_code,
                        bypass_reason.as_deref(),
                    )
                    .map_err(to_py_error)?;
                if result.applied {
                    vfs.commit_file(Path::new(file_path)).map_err(to_py_error)?;
                }
                Ok(result)
            }
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (file_path, semantic_path, new_code, source=None, bypass_reason=None))]
    fn preview_patch_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => preview_patch_ast_node(
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
            ),
            None => preview_patch_ast_node_from_path(
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (file_path, row, column, new_code, source=None, bypass_reason=None))]
    fn preview_patch_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let position = Position { row, column };
        let result = match source {
            Some(source) => preview_patch_ast_node_at_position(
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
            ),
            None => preview_patch_ast_node_at_position_from_path(
                Path::new(file_path),
                &position,
                new_code,
                bypass_reason.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    fn patch_virtual_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .patch_node(
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
            )
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (file_path, row, column, new_code, bypass_reason=None))]
    fn patch_virtual_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let position = Position { row, column };
        let result = self
            .vfs
            .borrow_mut()
            .patch_node_at_position(
                Path::new(file_path),
                &position,
                new_code,
                bypass_reason.as_deref(),
            )
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", index_db_path=None, file_path=None, source=None))]
    fn trace_symbol_graph_json(
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

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path=None, source=None))]
    #[allow(clippy::too_many_arguments)]
    fn trace_symbol_neighborhood_json(
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

    #[pyo3(signature = (workspace_root, symbol_path, index_db_path=None, file_path=None, source=None))]
    fn read_symbol_json(
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

    #[pyo3(signature = (workspace_root, file_path, row, column, direction="both", source=None, index_db_path=None))]
    #[allow(clippy::too_many_arguments)]
    fn trace_symbol_graph_at_position_json(
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

    #[pyo3(signature = (workspace_root, file_path, row, column, direction="both", max_depth=2, max_nodes=64, source=None, index_db_path=None))]
    #[allow(clippy::too_many_arguments)]
    fn trace_symbol_neighborhood_at_position_json(
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
    #[pyo3(signature = (workspace_root, query, limit=20, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
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
    #[pyo3(signature = (workspace_root, query, limit=20, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
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

    #[pyo3(signature = (workspace_root, limit=100, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
    #[allow(clippy::too_many_arguments)]
    fn list_symbols_json(
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

    #[pyo3(signature = (workspace_root, limit=100, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
    #[allow(clippy::too_many_arguments)]
    fn list_symbols_context_json(
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
    #[pyo3(signature = (workspace_root, limit=100, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
    fn list_symbols_neighborhood_context_json(
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
    #[pyo3(signature = (workspace_root, limit=100, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
    fn list_symbols_discovery_context_json(
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

    fn replay_patch_evidence_against_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let trace: TraceSymbolGraphResult = parse_json_arg(trace_json)?;
        let result = replay_patch_evidence_against_trace(&patch, &trace).map_err(to_py_error)?;
        to_json_result(&result)
    }

    fn validate_patch_commit_with_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let trace: TraceSymbolGraphResult = parse_json_arg(trace_json)?;
        let result = validate_patch_commit_with_trace(&patch, &trace).map_err(to_py_error)?;
        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, file_path, semantic_path, new_code, source=None, bypass_reason=None, direction="both", index_db_path=None))]
    // Keep the Python binding signature aligned with the JSON-RPC parameter surface.
    #[allow(clippy::too_many_arguments)]
    fn validate_patch_with_trace_context_json(
        &self,
        workspace_root: &str,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => validate_patch_with_trace_context_from_index(
                Path::new(&index_db_path),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
            (Some(source), None) => validate_patch_with_trace_context(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_trace_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                )
            }
            (None, None) => self.vfs.borrow_mut().validate_patch_with_trace_context(
                Path::new(workspace_root),
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, new_code, source=None, bypass_reason=None, direction="both", index_db_path=None))]
    #[allow(clippy::too_many_arguments)]
    fn validate_patch_with_trace_context_at_position_json(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_trace_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                )
            }
            (Some(source), None) => validate_patch_with_trace_context_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_trace_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_trace_context_at_position(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, file_path, semantic_path, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64, index_db_path=None))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => validate_patch_with_graph_context_from_index(
                Path::new(&index_db_path),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (Some(source), None) => validate_patch_with_graph_context(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_graph_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self.vfs.borrow_mut().validate_patch_with_graph_context(
                Path::new(workspace_root),
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64, index_db_path=None))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_graph_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => validate_patch_with_graph_context_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_graph_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_graph_context_at_position(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, file_path, semantic_path, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64, index_db_path=None))]
    #[allow(clippy::too_many_arguments)]
    fn validate_patch_with_neighborhood_context_json(
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_neighborhood_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => validate_patch_with_neighborhood_context(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_neighborhood_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_neighborhood_context(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64, index_db_path=None))]
    #[allow(clippy::too_many_arguments)]
    fn validate_patch_with_neighborhood_context_at_position_json(
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_neighborhood_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => validate_patch_with_neighborhood_context_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_neighborhood_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_neighborhood_context_at_position(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, file_path, semantic_path, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64, index_db_path=None))]
    #[allow(clippy::too_many_arguments)]
    fn validate_patch_with_discovery_context_json(
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_discovery_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => validate_patch_with_discovery_context(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_discovery_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self.vfs.borrow_mut().validate_patch_with_discovery_context(
                Path::new(workspace_root),
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64, index_db_path=None))]
    #[allow(clippy::too_many_arguments)]
    fn validate_patch_with_discovery_context_at_position_json(
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_discovery_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => validate_patch_with_discovery_context_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_discovery_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_discovery_context_at_position(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, db_path, max_files=20_000))]
    fn rebuild_symbol_index_json(
        &self,
        workspace_root: &str,
        db_path: &str,
        max_files: usize,
    ) -> PyResult<String> {
        let result = rebuild_symbol_index_with_limits(
            Path::new(workspace_root),
            Path::new(db_path),
            WorkspaceScanLimits { max_files },
        )
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    fn inspect_symbol_index_json(&self, db_path: &str) -> PyResult<String> {
        let result = inspect_symbol_index(Path::new(db_path)).map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, db_path, file_path, max_files=20_000))]
    fn refresh_symbol_index_for_file_json(
        &self,
        workspace_root: &str,
        db_path: &str,
        file_path: &str,
        max_files: usize,
    ) -> PyResult<String> {
        let result = refresh_symbol_index_for_file_with_limits(
            Path::new(workspace_root),
            Path::new(db_path),
            Path::new(file_path),
            WorkspaceScanLimits { max_files },
        )
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    fn register_symbol_index_json(&self, workspace_root: &str, db_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .register_symbol_index(Path::new(workspace_root), Path::new(db_path))
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    fn unregister_symbol_index_json(&self, workspace_root: &str) -> PyResult<bool> {
        self.vfs
            .borrow_mut()
            .unregister_symbol_index(Path::new(workspace_root))
            .map_err(to_py_error)
    }

    fn list_symbol_indexes_json(&self) -> PyResult<String> {
        let result = self
            .vfs
            .borrow()
            .registered_symbol_indexes_checked()
            .map_err(to_py_error)?;
        to_json_result(&result)
    }

    fn open_virtual_file_json(&self, file_path: &str, source: Option<String>) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .open_file(Path::new(file_path), source.as_deref())
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    fn read_virtual_file_json(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .read_file(Path::new(file_path))
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    fn list_virtual_files_json(&self, dirty_only: bool) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .virtual_file_statuses(dirty_only)
            .map_err(to_py_error)?;
        to_json_result(&result)
    }

    fn apply_buffer_edit_json(
        &self,
        file_path: &str,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .apply_edit(Path::new(file_path), start_byte, old_end_byte, new_text)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    fn apply_position_edits_json(&self, file_path: &str, edits_json: &str) -> PyResult<String> {
        let edits: Vec<PositionEdit> = parse_json_arg(edits_json)?;
        let result = self
            .vfs
            .borrow_mut()
            .apply_position_edits(Path::new(file_path), &edits)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    fn commit_virtual_file_json(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .commit_file(Path::new(file_path))
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    fn discard_virtual_file_json(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .discard_file(Path::new(file_path))
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (file_path, persist=false))]
    fn close_virtual_file_json(&self, file_path: &str, persist: bool) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .close_file(Path::new(file_path), persist)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }
}

fn to_py_error(error: anyhow::Error) -> PyErr {
    PyValueError::new_err(error.to_string())
}

fn to_runtime_error(error: serde_json::Error) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

fn to_json_result<T: Serialize>(result: &T) -> PyResult<String> {
    serde_json::to_string(result).map_err(to_runtime_error)
}

fn require_source_file_path(file_path: Option<&str>) -> PyResult<&Path> {
    file_path
        .map(Path::new)
        .ok_or_else(|| PyValueError::new_err("file_path is required when source is provided"))
}

fn parse_direction(direction: &str) -> PyResult<TraceDirection> {
    match direction {
        "callers" => Ok(TraceDirection::Callers),
        "callees" => Ok(TraceDirection::Callees),
        "both" => Ok(TraceDirection::Both),
        other => Err(PyValueError::new_err(format!(
            "invalid direction `{other}`, expected callers|callees|both"
        ))),
    }
}

#[pymodule]
fn _arborist_core(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<ArboristCore>()?;
    Ok(())
}

#[cfg(test)]
mod tests;
