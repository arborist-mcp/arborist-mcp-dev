mod index_bindings;
mod json_args;
mod patch_bindings;
mod patch_validation;
mod path_context;
mod symbol_queries;
mod vfs_bindings;

use std::cell::RefCell;
use std::path::Path;

#[cfg(test)]
use arborist_core::{PatchAstNodeResult, TraceSymbolGraphResult};
use arborist_core::{
    TraceDirection, VirtualFileSystem, execute_tree_query_from_path_with_timeout,
    execute_tree_query_with_timeout, get_semantic_skeleton, get_semantic_skeleton_from_path,
    supported_languages,
};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

pub(crate) use crate::json_args::parse_json_arg;
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
