use std::path::Path;

use pyo3::prelude::*;

use crate::{ArboristCore, source_position, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (
        file_path,
        semantic_path,
        new_code,
        bypass_reason=None,
        timeout_ms=None
    ))]
    fn patch_virtual_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.patch_virtual_ast_node_json_impl(
            file_path,
            semantic_path,
            new_code,
            bypass_reason,
            timeout_ms,
        )
    }

    #[pyo3(signature = (
        file_path,
        row,
        column,
        new_code,
        bypass_reason=None,
        timeout_ms=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn patch_virtual_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.patch_virtual_ast_node_at_position_json_impl(
            file_path,
            row,
            column,
            new_code,
            bypass_reason,
            timeout_ms,
        )
    }
}

impl ArboristCore {
    pub(crate) fn patch_virtual_ast_node_json_impl(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .patch_node_with_timeout(
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            )
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(crate) fn patch_virtual_ast_node_at_position_json_impl(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let position = source_position(row, column);
        let result = self
            .vfs
            .borrow_mut()
            .patch_node_at_position_with_timeout(
                Path::new(file_path),
                &position,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            )
            .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
