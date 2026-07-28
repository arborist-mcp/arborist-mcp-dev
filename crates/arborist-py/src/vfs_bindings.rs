use std::path::Path;

use arborist_core::PositionEdit;
use pyo3::prelude::*;

use crate::{ArboristCore, parse_json_arg, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (file_path, source=None, timeout_ms=None))]
    fn open_virtual_file_json(
        &self,
        file_path: &str,
        source: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.open_virtual_file_json_impl(file_path, source, timeout_ms)
    }

    #[pyo3(signature = (file_path, timeout_ms=None))]
    fn read_virtual_file_json(&self, file_path: &str, timeout_ms: Option<u64>) -> PyResult<String> {
        self.read_virtual_file_json_impl(file_path, timeout_ms)
    }

    #[pyo3(signature = (dirty_only, timeout_ms=None))]
    fn list_virtual_files_json(
        &self,
        dirty_only: bool,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.list_virtual_files_json_impl(dirty_only, timeout_ms)
    }

    fn apply_buffer_edit_json(
        &self,
        file_path: &str,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
    ) -> PyResult<String> {
        self.apply_buffer_edit_json_impl(file_path, start_byte, old_end_byte, new_text)
    }

    fn apply_position_edits_json(&self, file_path: &str, edits_json: &str) -> PyResult<String> {
        self.apply_position_edits_json_impl(file_path, edits_json)
    }

    #[pyo3(signature = (file_path, timeout_ms=None))]
    fn commit_virtual_file_json(
        &self,
        file_path: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.commit_virtual_file_json_impl(file_path, timeout_ms)
    }

    #[pyo3(signature = (file_path, timeout_ms=None))]
    fn discard_virtual_file_json(
        &self,
        file_path: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.discard_virtual_file_json_impl(file_path, timeout_ms)
    }

    #[pyo3(signature = (file_path, persist=false, timeout_ms=None))]
    fn close_virtual_file_json(
        &self,
        file_path: &str,
        persist: bool,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.close_virtual_file_json_impl(file_path, persist, timeout_ms)
    }
}

impl ArboristCore {
    pub(super) fn open_virtual_file_json_impl(
        &self,
        file_path: &str,
        source: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .open_file_with_timeout(Path::new(file_path), source.as_deref(), timeout_ms)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn read_virtual_file_json_impl(
        &self,
        file_path: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .read_file_with_timeout(Path::new(file_path), timeout_ms)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn list_virtual_files_json_impl(
        &self,
        dirty_only: bool,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .virtual_file_statuses_with_timeout(dirty_only, timeout_ms)
            .map_err(to_py_error)?;
        to_json_result(&result)
    }

    pub(super) fn apply_buffer_edit_json_impl(
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

    pub(super) fn apply_position_edits_json_impl(
        &self,
        file_path: &str,
        edits_json: &str,
    ) -> PyResult<String> {
        let edits: Vec<PositionEdit> = parse_json_arg(edits_json)?;
        let result = self
            .vfs
            .borrow_mut()
            .apply_position_edits(Path::new(file_path), &edits)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn commit_virtual_file_json_impl(
        &self,
        file_path: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .commit_file_with_timeout(Path::new(file_path), timeout_ms)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn discard_virtual_file_json_impl(
        &self,
        file_path: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .discard_file_with_timeout(Path::new(file_path), timeout_ms)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn close_virtual_file_json_impl(
        &self,
        file_path: &str,
        persist: bool,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .close_file_with_timeout(Path::new(file_path), persist, timeout_ms)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
