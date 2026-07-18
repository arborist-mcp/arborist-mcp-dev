mod index_bindings;
mod json_args;
mod patch_bindings;
mod patch_validation;
mod path_context;
mod source_query_bindings;
mod symbol_queries;
mod vfs_bindings;

use std::cell::RefCell;

#[cfg(test)]
use arborist_core::{PatchAstNodeResult, TraceSymbolGraphResult};
use arborist_core::{TraceDirection, VirtualFileSystem, supported_languages};
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
