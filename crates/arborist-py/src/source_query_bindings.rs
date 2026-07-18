use std::path::Path;

use arborist_core::{
    execute_tree_query_from_path_with_timeout, execute_tree_query_with_timeout,
    get_semantic_skeleton, get_semantic_skeleton_from_path,
};
use pyo3::prelude::*;

use crate::{ArboristCore, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
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
