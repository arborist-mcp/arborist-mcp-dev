use std::cell::RefCell;
use std::path::Path;

use arborist_core::{
    PatchAstNodeResult, PositionEdit, TraceDirection, TraceSymbolGraphResult, VirtualFileSystem,
    execute_tree_query, execute_tree_query_from_path, get_semantic_skeleton,
    get_semantic_skeleton_from_path, patch_ast_node, rebuild_symbol_index,
    refresh_symbol_index_for_file, replay_patch_evidence_against_trace, supported_languages,
    trace_symbol_graph_from_index, validate_patch_commit_with_trace,
    validate_patch_with_trace_context, validate_patch_with_trace_context_from_path,
};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

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

    fn replay_patch_evidence_against_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = serde_json::from_str(patch_json)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let trace: TraceSymbolGraphResult = serde_json::from_str(trace_json)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let result = replay_patch_evidence_against_trace(&patch, &trace);
        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn validate_patch_commit_with_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = serde_json::from_str(patch_json)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let trace: TraceSymbolGraphResult = serde_json::from_str(trace_json)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let result = validate_patch_commit_with_trace(&patch, &trace);
        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, file_path, semantic_path, new_code, source=None, bypass_reason=None, direction="both"))]
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
        let result = self.vfs.borrow().registered_symbol_indexes();
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
        let result = self.vfs.borrow().virtual_file_statuses(dirty_only);
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
        let edits: Vec<PositionEdit> = serde_json::from_str(edits_json)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
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
