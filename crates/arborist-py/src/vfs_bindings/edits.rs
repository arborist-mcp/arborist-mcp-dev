use std::path::Path;

use arborist_core::PositionEdit;
use pyo3::prelude::*;

use crate::{ArboristCore, parse_json_arg, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (file_path, start_byte, old_end_byte, new_text, timeout_ms=None))]
    fn apply_buffer_edit_json(
        &self,
        file_path: &str,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.apply_buffer_edit_json_impl(file_path, start_byte, old_end_byte, new_text, timeout_ms)
    }

    #[pyo3(signature = (file_path, edits_json, timeout_ms=None))]
    fn apply_position_edits_json(
        &self,
        file_path: &str,
        edits_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.apply_position_edits_json_impl(file_path, edits_json, timeout_ms)
    }
}

impl ArboristCore {
    pub(crate) fn apply_buffer_edit_json_impl(
        &self,
        file_path: &str,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .apply_edit_with_timeout(
                Path::new(file_path),
                start_byte,
                old_end_byte,
                new_text,
                timeout_ms,
            )
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(crate) fn apply_position_edits_json_impl(
        &self,
        file_path: &str,
        edits_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let edits: Vec<PositionEdit> = parse_json_arg(edits_json)?;
        let result = self
            .vfs
            .borrow_mut()
            .apply_position_edits_with_timeout(Path::new(file_path), &edits, timeout_ms)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
