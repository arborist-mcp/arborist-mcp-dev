mod index_bindings;
mod json_args;
mod patch_bindings;
mod patch_validation;
mod path_context;
mod symbol_queries;
mod vfs_bindings;

use std::cell::RefCell;
use std::path::Path;

use arborist_core::{
    PatchAstNodeResult, TraceDirection, TraceSymbolGraphResult, VirtualFileSystem,
    WorkspacePositionEdits, execute_tree_query_from_path_with_timeout,
    execute_tree_query_with_timeout, export_patch_diagnostics_sarif, get_semantic_skeleton,
    get_semantic_skeleton_from_path, preview_workspace_position_edits,
    replay_patch_evidence_against_trace, supported_languages, validate_patch_commit_with_trace,
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

    #[pyo3(signature = (file_path, query, source=None, max_captures=10_000, timeout_ms=None))]
    fn execute_tree_query_json(
        &self,
        file_path: &str,
        query: &str,
        source: Option<String>,
        max_captures: usize,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => execute_tree_query_with_timeout(
                Path::new(file_path),
                &source,
                query,
                max_captures,
                timeout_ms,
            ),
            None => execute_tree_query_from_path_with_timeout(
                Path::new(file_path),
                query,
                max_captures,
                timeout_ms,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", index_db_path=None, file_path=None, source=None, timeout_ms=None))]
    #[allow(clippy::too_many_arguments)]
    fn trace_symbol_graph_json(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.trace_symbol_graph_json_impl(
            workspace_root,
            symbol_path,
            direction,
            index_db_path,
            file_path,
            source,
            timeout_ms,
        )
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path=None, source=None, timeout_ms=None))]
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
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.trace_symbol_neighborhood_json_impl(
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

    #[pyo3(signature = (workspace_root, file_path, row, column, direction="both", source=None, index_db_path=None, timeout_ms=None))]
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
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.trace_symbol_graph_at_position_json_impl(
            workspace_root,
            file_path,
            row,
            column,
            direction,
            source,
            index_db_path,
            timeout_ms,
        )
    }

    #[pyo3(signature = (workspace_root, file_path, row, column, direction="both", max_depth=2, max_nodes=64, source=None, index_db_path=None, timeout_ms=None))]
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
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.trace_symbol_neighborhood_at_position_json_impl(
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
        self.list_symbols_json_impl(
            workspace_root,
            limit,
            index_db_path,
            file_path_contains,
            node_kind,
            file_path,
            source,
        )
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
        self.list_symbols_context_json_impl(
            workspace_root,
            limit,
            index_db_path,
            file_path_contains,
            node_kind,
            file_path,
            source,
        )
    }

    #[pyo3(signature = (workspace_root, limit=100, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
    #[allow(clippy::too_many_arguments)]
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
        self.list_symbols_neighborhood_context_json_impl(
            workspace_root,
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

    #[pyo3(signature = (workspace_root, limit=100, direction="both", max_depth=2, max_nodes=64, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None))]
    #[allow(clippy::too_many_arguments)]
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
        self.list_symbols_discovery_context_json_impl(
            workspace_root,
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

    fn export_patch_diagnostics_sarif_json(&self, patch_json: &str) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let result = export_patch_diagnostics_sarif(&patch).map_err(to_py_error)?;
        to_json_result(&result)
    }

    fn preview_workspace_position_edits_json(&self, files_json: &str) -> PyResult<String> {
        let files: Vec<WorkspacePositionEdits> = parse_json_arg(files_json)?;
        let result = preview_workspace_position_edits(&files).map_err(to_py_error)?;
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
        self.validate_patch_with_neighborhood_context_json_impl(
            workspace_root,
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            direction,
            NeighborhoodBounds::new(max_depth, max_nodes),
            index_db_path,
        )
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
        self.validate_patch_with_neighborhood_context_at_position_json_impl(
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
        )
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
        self.validate_patch_with_discovery_context_json_impl(
            workspace_root,
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            direction,
            NeighborhoodBounds::new(max_depth, max_nodes),
            index_db_path,
        )
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
        self.validate_patch_with_discovery_context_at_position_json_impl(
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
        )
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

fn source_position(row: usize, column: usize) -> arborist_core::Position {
    arborist_core::Position { row, column }
}

#[derive(Clone, Copy)]
struct NeighborhoodBounds {
    max_depth: usize,
    max_nodes: usize,
}

impl NeighborhoodBounds {
    fn new(max_depth: usize, max_nodes: usize) -> Self {
        Self {
            max_depth,
            max_nodes,
        }
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
mod tests;
