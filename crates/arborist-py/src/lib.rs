use std::cell::RefCell;
use std::path::Path;

use arborist_core::{
    PatchAstNodeResult, Position, PositionEdit, TraceDirection, TraceSymbolGraphResult,
    VirtualFileSystem, execute_tree_query, execute_tree_query_from_path, get_semantic_skeleton,
    get_semantic_skeleton_from_path, list_symbols_context_from_index_filtered,
    list_symbols_discovery_context_from_index_filtered, list_symbols_from_index_filtered,
    list_symbols_neighborhood_context_from_index_filtered, patch_ast_node,
    patch_ast_node_at_position, read_symbol_at_position_from_index,
    read_symbol_at_position_with_source, read_symbol_context_at_position_from_index,
    read_symbol_context_at_position_with_source, read_symbol_context_from_index,
    read_symbol_discovery_context_at_position_from_index,
    read_symbol_discovery_context_at_position_with_source,
    read_symbol_discovery_context_from_index, read_symbol_from_index,
    read_symbol_neighborhood_context_at_position_from_index,
    read_symbol_neighborhood_context_at_position_with_source,
    read_symbol_neighborhood_context_from_index, rebuild_symbol_index,
    refresh_symbol_index_for_file, replay_patch_evidence_against_trace,
    search_symbols_context_from_index_filtered,
    search_symbols_discovery_context_from_index_filtered, search_symbols_from_index_filtered,
    search_symbols_neighborhood_context_from_index_filtered, supported_languages,
    trace_symbol_graph_at_position_from_index, trace_symbol_graph_at_position_with_source,
    trace_symbol_graph_from_index, trace_symbol_neighborhood_at_position_from_index,
    trace_symbol_neighborhood_at_position_with_source, trace_symbol_neighborhood_from_index,
    validate_patch_commit_with_trace, validate_patch_with_discovery_context,
    validate_patch_with_discovery_context_at_position,
    validate_patch_with_discovery_context_at_position_from_path,
    validate_patch_with_discovery_context_from_path, validate_patch_with_graph_context,
    validate_patch_with_graph_context_at_position,
    validate_patch_with_graph_context_at_position_from_path,
    validate_patch_with_graph_context_from_path, validate_patch_with_neighborhood_context,
    validate_patch_with_neighborhood_context_at_position,
    validate_patch_with_neighborhood_context_at_position_from_path,
    validate_patch_with_neighborhood_context_from_path, validate_patch_with_trace_context,
    validate_patch_with_trace_context_at_position,
    validate_patch_with_trace_context_at_position_from_path,
    validate_patch_with_trace_context_from_path,
};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use serde::de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (file_path, query, source=None))]
    fn execute_tree_query_json(
        &self,
        file_path: &str,
        query: &str,
        source: Option<String>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => execute_tree_query(Path::new(file_path), &source, query),
            None => execute_tree_query_from_path(Path::new(file_path), query),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
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

        serde_json::to_string(&result).map_err(to_runtime_error)
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

        serde_json::to_string(&result).map_err(to_runtime_error)
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

        serde_json::to_string(&result).map_err(to_runtime_error)
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", index_db_path=None))]
    fn trace_symbol_graph_json(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match index_db_path {
            Some(index_db_path) => {
                trace_symbol_graph_from_index(Path::new(&index_db_path), symbol_path, direction)
            }
            None => self.vfs.borrow_mut().trace_symbol_graph(
                Path::new(workspace_root),
                symbol_path,
                direction,
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", max_depth=2, max_nodes=64, index_db_path=None))]
    fn trace_symbol_neighborhood_json(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match index_db_path {
            Some(index_db_path) => trace_symbol_neighborhood_from_index(
                Path::new(&index_db_path),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
            None => self.vfs.borrow_mut().trace_symbol_neighborhood(
                Path::new(workspace_root),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, symbol_path, index_db_path=None))]
    fn read_symbol_json(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let result = match index_db_path {
            Some(index_db_path) => read_symbol_from_index(Path::new(&index_db_path), symbol_path),
            None => self
                .vfs
                .borrow_mut()
                .read_symbol(Path::new(workspace_root), symbol_path),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
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
        if source.is_some() && index_db_path.is_some() {
            return Err(PyValueError::new_err(
                "index_db_path is not supported when source is provided",
            ));
        }
        let result = match (source, index_db_path) {
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
            (Some(_), Some(_)) => unreachable!("checked above"),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", index_db_path=None))]
    fn read_symbol_context_json(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match index_db_path {
            Some(index_db_path) => {
                read_symbol_context_from_index(Path::new(&index_db_path), symbol_path, direction)
            }
            None => self.vfs.borrow_mut().read_symbol_context(
                Path::new(workspace_root),
                symbol_path,
                direction,
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
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
        if source.is_some() && index_db_path.is_some() {
            return Err(PyValueError::new_err(
                "index_db_path is not supported when source is provided",
            ));
        }
        let result = match (source, index_db_path) {
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
            (Some(_), Some(_)) => unreachable!("checked above"),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
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
        if source.is_some() && index_db_path.is_some() {
            return Err(PyValueError::new_err(
                "index_db_path is not supported when source is provided",
            ));
        }
        let result = match (source, index_db_path) {
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
            (Some(_), Some(_)) => unreachable!("checked above"),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
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
        if source.is_some() && index_db_path.is_some() {
            return Err(PyValueError::new_err(
                "index_db_path is not supported when source is provided",
            ));
        }
        let result = match (source, index_db_path) {
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
            (Some(_), Some(_)) => unreachable!("checked above"),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", max_depth=2, max_nodes=64, index_db_path=None))]
    fn read_symbol_neighborhood_context_json(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match index_db_path {
            Some(index_db_path) => read_symbol_neighborhood_context_from_index(
                Path::new(&index_db_path),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
            None => self.vfs.borrow_mut().read_symbol_neighborhood_context(
                Path::new(workspace_root),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
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
        if source.is_some() && index_db_path.is_some() {
            return Err(PyValueError::new_err(
                "index_db_path is not supported when source is provided",
            ));
        }
        let result = match (source, index_db_path) {
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
            (Some(_), Some(_)) => unreachable!("checked above"),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", max_depth=2, max_nodes=64, index_db_path=None))]
    fn read_symbol_discovery_context_json(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match index_db_path {
            Some(index_db_path) => read_symbol_discovery_context_from_index(
                Path::new(&index_db_path),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
            None => self.vfs.borrow_mut().read_symbol_discovery_context(
                Path::new(workspace_root),
                symbol_path,
                direction,
                max_depth,
                max_nodes,
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
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
        if source.is_some() && index_db_path.is_some() {
            return Err(PyValueError::new_err(
                "index_db_path is not supported when source is provided",
            ));
        }
        let result = match (source, index_db_path) {
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
            (Some(_), Some(_)) => unreachable!("checked above"),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, query, limit=20, index_db_path=None, file_path_contains=None, node_kind=None))]
    fn search_symbols_json(
        &self,
        workspace_root: &str,
        query: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
    ) -> PyResult<String> {
        let result = match index_db_path {
            Some(index_db_path) => search_symbols_from_index_filtered(
                Path::new(&index_db_path),
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            None => self.vfs.borrow_mut().search_symbols_filtered(
                Path::new(workspace_root),
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, query, limit=20, index_db_path=None, file_path_contains=None, node_kind=None))]
    fn search_symbols_context_json(
        &self,
        workspace_root: &str,
        query: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
    ) -> PyResult<String> {
        let result = match index_db_path {
            Some(index_db_path) => search_symbols_context_from_index_filtered(
                Path::new(&index_db_path),
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            None => self.vfs.borrow_mut().search_symbols_context_filtered(
                Path::new(workspace_root),
                query,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (workspace_root, query, limit=20, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match index_db_path {
            Some(index_db_path) => search_symbols_neighborhood_context_from_index_filtered(
                Path::new(&index_db_path),
                query,
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            None => self
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (workspace_root, query, limit=20, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match index_db_path {
            Some(index_db_path) => search_symbols_discovery_context_from_index_filtered(
                Path::new(&index_db_path),
                query,
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            None => self
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, limit=100, index_db_path=None, file_path_contains=None, node_kind=None))]
    fn list_symbols_json(
        &self,
        workspace_root: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
    ) -> PyResult<String> {
        let result = match index_db_path {
            Some(index_db_path) => list_symbols_from_index_filtered(
                Path::new(&index_db_path),
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            None => self.vfs.borrow_mut().list_symbols_filtered(
                Path::new(workspace_root),
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, limit=100, index_db_path=None, file_path_contains=None, node_kind=None))]
    fn list_symbols_context_json(
        &self,
        workspace_root: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
    ) -> PyResult<String> {
        let result = match index_db_path {
            Some(index_db_path) => list_symbols_context_from_index_filtered(
                Path::new(&index_db_path),
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            None => self.vfs.borrow_mut().list_symbols_context_filtered(
                Path::new(workspace_root),
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (workspace_root, limit=100, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match index_db_path {
            Some(index_db_path) => list_symbols_neighborhood_context_from_index_filtered(
                Path::new(&index_db_path),
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            None => self
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (workspace_root, limit=100, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match index_db_path {
            Some(index_db_path) => list_symbols_discovery_context_from_index_filtered(
                Path::new(&index_db_path),
                limit,
                direction,
                max_depth,
                max_nodes,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
            ),
            None => self
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn replay_patch_evidence_against_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let trace: TraceSymbolGraphResult = parse_json_arg(trace_json)?;
        let result = replay_patch_evidence_against_trace(&patch, &trace).map_err(to_py_error)?;
        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn validate_patch_commit_with_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let trace: TraceSymbolGraphResult = parse_json_arg(trace_json)?;
        let result = validate_patch_commit_with_trace(&patch, &trace).map_err(to_py_error)?;
        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, file_path, semantic_path, new_code, source=None, bypass_reason=None, direction="both"))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match source {
            Some(source) => validate_patch_with_trace_context(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
            None => validate_patch_with_trace_context_from_path(
                Path::new(workspace_root),
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, new_code, source=None, bypass_reason=None, direction="both"))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match source {
            Some(source) => validate_patch_with_trace_context_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
            None => validate_patch_with_trace_context_at_position_from_path(
                Path::new(workspace_root),
                Path::new(file_path),
                &position,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, file_path, semantic_path, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match source {
            Some(source) => validate_patch_with_graph_context(
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
            None => validate_patch_with_graph_context_from_path(
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match source {
            Some(source) => validate_patch_with_graph_context_at_position(
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
            None => validate_patch_with_graph_context_at_position_from_path(
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, file_path, semantic_path, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match source {
            Some(source) => validate_patch_with_neighborhood_context(
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
            None => validate_patch_with_neighborhood_context_from_path(
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match source {
            Some(source) => validate_patch_with_neighborhood_context_at_position(
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
            None => validate_patch_with_neighborhood_context_at_position_from_path(
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, file_path, semantic_path, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match source {
            Some(source) => validate_patch_with_discovery_context(
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
            None => validate_patch_with_discovery_context_from_path(
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, new_code, source=None, bypass_reason=None, direction="both", max_depth=2, max_nodes=64))]
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
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match source {
            Some(source) => validate_patch_with_discovery_context_at_position(
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
            None => validate_patch_with_discovery_context_at_position_from_path(
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn rebuild_symbol_index_json(&self, workspace_root: &str, db_path: &str) -> PyResult<String> {
        let result = rebuild_symbol_index(Path::new(workspace_root), Path::new(db_path))
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn refresh_symbol_index_for_file_json(
        &self,
        workspace_root: &str,
        db_path: &str,
        file_path: &str,
    ) -> PyResult<String> {
        let result = refresh_symbol_index_for_file(
            Path::new(workspace_root),
            Path::new(db_path),
            Path::new(file_path),
        )
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn register_symbol_index_json(&self, workspace_root: &str, db_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .register_symbol_index(Path::new(workspace_root), Path::new(db_path))
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
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
        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn open_virtual_file_json(&self, file_path: &str, source: Option<String>) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .open_file(Path::new(file_path), source.as_deref())
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn read_virtual_file_json(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .read_file(Path::new(file_path))
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn list_virtual_files_json(&self, dirty_only: bool) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .virtual_file_statuses(dirty_only)
            .map_err(to_py_error)?;
        serde_json::to_string(&result).map_err(to_runtime_error)
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

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn apply_position_edits_json(&self, file_path: &str, edits_json: &str) -> PyResult<String> {
        let edits: Vec<PositionEdit> = parse_json_arg(edits_json)?;
        let result = self
            .vfs
            .borrow_mut()
            .apply_position_edits(Path::new(file_path), &edits)
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn commit_virtual_file_json(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .commit_file(Path::new(file_path))
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn discard_virtual_file_json(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .discard_file(Path::new(file_path))
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (file_path, persist=false))]
    fn close_virtual_file_json(&self, file_path: &str, persist: bool) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .close_file(Path::new(file_path), persist)
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }
}

fn to_py_error(error: anyhow::Error) -> PyErr {
    PyValueError::new_err(error.to_string())
}

fn to_runtime_error(error: serde_json::Error) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

fn parse_json_arg<T: DeserializeOwned>(json: &str) -> PyResult<T> {
    let checked = serde_json::from_str::<DuplicateCheckedJson>(json)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    serde_json::from_value(checked.0).map_err(|error| PyValueError::new_err(error.to_string()))
}

struct DuplicateCheckedJson(serde_json::Value);

impl<'de> Deserialize<'de> for DuplicateCheckedJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateCheckedJsonVisitor)
    }
}

struct DuplicateCheckedJsonVisitor;

impl<'de> Visitor<'de> for DuplicateCheckedJsonVisitor {
    type Value = DuplicateCheckedJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Number(
            serde_json::Number::from(value),
        )))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Number(
            serde_json::Number::from(value),
        )))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let number =
            serde_json::Number::from_f64(value).ok_or_else(|| E::custom("invalid JSON number"))?;
        Ok(DuplicateCheckedJson(serde_json::Value::Number(number)))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::String(
            value.to_string(),
        )))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        DuplicateCheckedJson::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(access.size_hint().unwrap_or(0));
        while let Some(value) = access.next_element::<DuplicateCheckedJson>()? {
            values.push(value.0);
        }
        Ok(DuplicateCheckedJson(serde_json::Value::Array(values)))
    }

    fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = serde_json::Map::new();
        while let Some(key) = access.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!(
                    "duplicate JSON object key `{key}`"
                )));
            }
            let value = access.next_value::<DuplicateCheckedJson>()?;
            values.insert(key, value.0);
        }
        Ok(DuplicateCheckedJson(serde_json::Value::Object(values)))
    }
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
mod tests {
    use super::{
        ArboristCore, PatchAstNodeResult, PositionEdit, TraceSymbolGraphResult, parse_json_arg,
    };
    use std::sync::Once;

    fn prepare_python() {
        static PREPARE: Once = Once::new();
        PREPARE.call_once(pyo3::prepare_freethreaded_python);
    }

    #[test]
    fn parse_json_arg_rejects_duplicate_top_level_keys() {
        prepare_python();

        let error = parse_json_arg::<PositionEdit>(
            r#"{"start":{"row":0,"column":0},"end":{"row":0,"column":1},"new_text":"x","new_text":"y"}"#,
        )
        .expect_err("duplicate top-level keys should be rejected");

        assert!(
            error
                .to_string()
                .contains("duplicate JSON object key `new_text`")
        );
    }

    #[test]
    fn parse_json_arg_rejects_duplicate_nested_keys() {
        prepare_python();

        let error = parse_json_arg::<Vec<PositionEdit>>(
            r#"[{"start":{"row":0,"column":0,"row":1},"end":{"row":0,"column":1},"new_text":"x"}]"#,
        )
        .expect_err("duplicate nested keys should be rejected");

        assert!(
            error
                .to_string()
                .contains("duplicate JSON object key `row`")
        );
    }

    #[test]
    fn parse_json_arg_accepts_valid_payloads() {
        prepare_python();

        let edits = parse_json_arg::<Vec<PositionEdit>>(
            r#"[{"start":{"row":0,"column":0},"end":{"row":0,"column":1},"new_text":"x"}]"#,
        )
        .expect("valid edit payload should parse");

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "x");
    }

    #[test]
    fn parse_json_arg_rejects_missing_nested_trace_fields() {
        prepare_python();

        let error = parse_json_arg::<TraceSymbolGraphResult>(
            r#"{
                "symbol":{"symbol_id":"top_level"},
                "callers":[],
                "callees":[],
                "evidence_keys":{
                    "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers":[],
                    "callees":[]
                },
                "indexed_files":1
            }"#,
        )
        .expect_err("trace payloads should reject missing nested symbol fields");

        assert!(error.to_string().contains("missing field"));
    }

    #[test]
    fn parse_json_arg_rejects_missing_nested_patch_fields() {
        prepare_python();

        let error = parse_json_arg::<PatchAstNodeResult>(
            r#"{
                "file":"sample.py",
                "target_path":"top_level",
                "resolved_path":"top_level",
                "resolved_symbol_id":"top_level",
                "applied":true,
                "bypass_applied":false,
                "updated_source":"def top_level() -> int:\n    return 1\n",
                "validation":{
                    "syntax_errors":[],
                    "resolved_identifiers":[],
                    "ambiguous_identifiers":[],
                    "binding_decisions":[],
                    "commit_gate":{
                        "status":"allowed",
                        "allowed":true,
                        "reason":"ok",
                        "bypass_reason":null,
                        "blocking_decisions":[],
                        "evidence_invariants":[],
                        "syntax_error_count":0
                    }
                }
            }"#,
        )
        .expect_err("patch payloads should reject missing nested validation fields");

        assert!(error.to_string().contains("missing field"));
    }

    #[test]
    fn replay_patch_evidence_against_trace_json_rejects_blank_selected_evidence_keys() {
        prepare_python();

        let core = ArboristCore::new();
        let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return 1\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"ok",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"ok",
                                "selected_evidence_key":"   ",
                                "candidate_evidence_keys":["top_level|sample.py|function_definition|trace_root|0..10|"]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":[]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("blank selected evidence keys should be rejected");

        assert!(error.to_string().contains("selected_evidence_key"));
    }

    #[test]
    fn replay_patch_evidence_against_trace_json_rejects_tampered_syntax_error_details() {
        prepare_python();

        let core = ArboristCore::new();
        let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":false,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return (\n",
                    "validation":{
                        "syntax_errors":[{
                            "kind":"error",
                            "message":"manually tampered",
                            "start_byte":0,
                            "end_byte":1,
                            "start_point":{"row":0,"column":0},
                            "end_point":{"row":0,"column":1}
                        }],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[],
                        "commit_gate":{
                            "status":"rejected",
                            "allowed":false,
                            "reason":"syntax validation failed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[],
                            "syntax_error_count":1
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":[]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("tampered syntax error details should be rejected");

        assert!(error.to_string().contains("patch.validation.syntax_errors"));
    }

    #[test]
    fn replay_patch_evidence_against_trace_json_rejects_blank_updated_source() {
        prepare_python();

        let core = ArboristCore::new();
        let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"   ",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"ok",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":[]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("blank updated_source values should be rejected");

        assert!(error.to_string().contains("patch.updated_source"));
    }

    #[test]
    fn replay_patch_evidence_against_trace_json_rejects_duplicate_candidate_evidence_keys() {
        prepare_python();

        let core = ArboristCore::new();
        let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[{
                            "name":"helper",
                            "symbol":{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }
                        }],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"helper",
                            "status":"resolved",
                            "reason":"resolved uniquely",
                            "selected_symbol_id":"helper",
                            "candidates":[{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }]
                        }],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"resolved binding has one selected candidate evidence key",
                                "selected_evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys":[
                                    "helper|sample.py|function_definition|callee|12..34|",
                                    "helper|sample.py|function_definition|callee|12..34|"
                                ]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[{
                        "symbol_id":"helper",
                        "semantic_path":"helper",
                        "scope_path":null,
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"callee",
                        "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                        "byte_range":[12,34],
                        "signature":null,
                        "parameters":[],
                        "return_type":null,
                        "docstring":null
                    }],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":["helper|sample.py|function_definition|callee|12..34|"]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("duplicate candidate evidence keys should be rejected");

        assert!(error.to_string().contains("candidate_evidence_keys[1]"));
    }

    #[test]
    fn replay_patch_evidence_against_trace_json_rejects_non_root_trace_symbol_origin_type() {
        prepare_python();

        let core = ArboristCore::new();
        let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return 2\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"callee",
                        "evidence_key":"top_level|sample.py|function_definition|callee|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|callee|0..10|",
                        "callers":[],
                        "callees":[]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("non-root trace symbol origin types should be rejected");

        assert!(error.to_string().contains("trace.symbol.origin_type"));
    }

    #[test]
    fn replay_patch_evidence_against_trace_json_rejects_tampered_resolved_identifier_summaries() {
        prepare_python();

        let core = ArboristCore::new();
        let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"helper",
                            "status":"resolved",
                            "reason":"resolved uniquely",
                            "selected_symbol_id":"helper",
                            "candidates":[{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }]
                        }],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"resolved binding has one selected candidate evidence key",
                                "selected_evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys":["helper|sample.py|function_definition|callee|12..34|"]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[{
                        "symbol_id":"helper",
                        "semantic_path":"helper",
                        "scope_path":null,
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"callee",
                        "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                        "byte_range":[12,34],
                        "signature":null,
                        "parameters":[],
                        "return_type":null,
                        "docstring":null
                    }],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":["helper|sample.py|function_definition|callee|12..34|"]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("tampered resolved identifier summaries should be rejected");

        assert!(error.to_string().contains("resolved_identifiers"));
    }

    #[test]
    fn replay_patch_evidence_against_trace_json_rejects_unsupported_binding_decision_statuses() {
        prepare_python();

        let core = ArboristCore::new();
        let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[{
                            "name":"helper",
                            "symbol":{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }
                        }],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"helper",
                            "status":"mystery",
                            "reason":"manually tampered",
                            "selected_symbol_id":"helper",
                            "candidates":[{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }]
                        }],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"resolved binding has one selected candidate evidence key",
                                "selected_evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys":["helper|sample.py|function_definition|callee|12..34|"]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[{
                        "symbol_id":"helper",
                        "semantic_path":"helper",
                        "scope_path":null,
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"callee",
                        "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                        "byte_range":[12,34],
                        "signature":null,
                        "parameters":[],
                        "return_type":null,
                        "docstring":null
                    }],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":["helper|sample.py|function_definition|callee|12..34|"]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("unsupported binding decision statuses should be rejected");

        assert!(error.to_string().contains("binding_decisions[0].status"));
    }

    #[test]
    fn validate_patch_commit_with_trace_json_rejects_inconsistent_patch_gate_flags() {
        prepare_python();

        let core = ArboristCore::new();
        let error = core
            .validate_patch_commit_with_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":false,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return 1\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"ok",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":[]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("inconsistent patch gate flags should be rejected");

        assert!(error.to_string().contains("patch.applied"));
    }

    #[test]
    fn replay_patch_evidence_against_trace_json_rejects_tampered_patch_gate_reason() {
        prepare_python();

        let core = ArboristCore::new();
        let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[{
                            "name":"helper",
                            "symbol":{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }
                        }],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"helper",
                            "status":"resolved",
                            "reason":"resolved uniquely",
                            "selected_symbol_id":"helper",
                            "candidates":[{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }]
                        }],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"manually overridden",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"resolved binding has one selected candidate evidence key",
                                "selected_evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys":["helper|sample.py|function_definition|callee|12..34|"]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[{
                        "symbol_id":"helper",
                        "semantic_path":"helper",
                        "scope_path":null,
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"callee",
                        "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                        "byte_range":[12,34],
                        "signature":null,
                        "parameters":[],
                        "return_type":null,
                        "docstring":null
                    }],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":["helper|sample.py|function_definition|callee|12..34|"]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("tampered patch gate reasons should be rejected");

        assert!(error.to_string().contains("commit_gate.reason"));
    }

    #[test]
    fn replay_patch_evidence_against_trace_json_rejects_mismatched_trace_root() {
        prepare_python();

        let core = ArboristCore::new();
        let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[{
                            "name":"helper",
                            "symbol":{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }
                        }],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"helper",
                            "status":"resolved",
                            "reason":"resolved uniquely",
                            "selected_symbol_id":"helper",
                            "candidates":[{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }]
                        }],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"resolved binding has one selected candidate evidence key",
                                "selected_evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys":["helper|sample.py|function_definition|callee|12..34|"]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"helper",
                        "semantic_path":"helper",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"helper|sample.py|function_definition|trace_root|12..34|",
                        "byte_range":[12,34],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[],
                    "evidence_keys":{
                        "symbol":"helper|sample.py|function_definition|trace_root|12..34|",
                        "callers":[],
                        "callees":[]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("mismatched trace roots should be rejected");

        assert!(error.to_string().contains("trace.symbol.symbol_id"));
    }

    #[test]
    fn replay_patch_evidence_against_trace_json_rejects_mismatched_trace_root_file() {
        prepare_python();

        let core = ArboristCore::new();
        let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample_a.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return 1\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample_b.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"top_level|sample_b.py|function_definition|trace_root|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[],
                    "evidence_keys":{
                        "symbol":"top_level|sample_b.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":[]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("mismatched trace root files should be rejected");

        assert!(error.to_string().contains("trace.symbol.file_path"));
    }
}
