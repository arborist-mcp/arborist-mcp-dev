use std::path::Path;

use arborist_core::{
    patch_ast_node_at_position_with_timeout, patch_ast_node_with_timeout,
    preview_patch_ast_node_at_position_from_path_with_timeout,
    preview_patch_ast_node_at_position_with_timeout, preview_patch_ast_node_from_path_with_timeout,
    preview_patch_ast_node_with_timeout,
};
use pyo3::prelude::*;

use crate::{ArboristCore, source_position, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (
        file_path,
        semantic_path,
        new_code,
        source=None,
        bypass_reason=None,
        timeout_ms=None
    ))]
    fn patch_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.patch_ast_node_json_impl(
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            timeout_ms,
        )
    }

    #[pyo3(signature = (
        file_path,
        row,
        column,
        new_code,
        source=None,
        bypass_reason=None,
        timeout_ms=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn patch_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.patch_ast_node_at_position_json_impl(
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
            timeout_ms,
        )
    }

    #[pyo3(signature = (
        file_path,
        semantic_path,
        new_code,
        source=None,
        bypass_reason=None,
        timeout_ms=None
    ))]
    fn preview_patch_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.preview_patch_ast_node_json_impl(
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            timeout_ms,
        )
    }

    #[pyo3(signature = (
        file_path,
        row,
        column,
        new_code,
        source=None,
        bypass_reason=None,
        timeout_ms=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn preview_patch_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.preview_patch_ast_node_at_position_json_impl(
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
            timeout_ms,
        )
    }
}

impl ArboristCore {
    pub(crate) fn patch_ast_node_json_impl(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => patch_ast_node_with_timeout(
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
            None => self.vfs.borrow_mut().patch_node_and_commit_with_timeout(
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn patch_ast_node_at_position_json_impl(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let position = source_position(row, column);
        let result = match source {
            Some(source) => patch_ast_node_at_position_with_timeout(
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
            None => self
                .vfs
                .borrow_mut()
                .patch_node_at_position_and_commit_with_timeout(
                    Path::new(file_path),
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    timeout_ms,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(crate) fn preview_patch_ast_node_json_impl(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => preview_patch_ast_node_with_timeout(
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
            None => preview_patch_ast_node_from_path_with_timeout(
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn preview_patch_ast_node_at_position_json_impl(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let position = source_position(row, column);
        let result = match source {
            Some(source) => preview_patch_ast_node_at_position_with_timeout(
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
            None => preview_patch_ast_node_at_position_from_path_with_timeout(
                Path::new(file_path),
                &position,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
