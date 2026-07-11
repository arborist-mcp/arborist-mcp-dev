use std::path::Path;

use arborist_core::PositionEdit;
use pyo3::prelude::*;

use crate::{ArboristCore, parse_json_arg, to_json_result, to_py_error};

impl ArboristCore {
    pub(super) fn open_virtual_file_json_impl(
        &self,
        file_path: &str,
        source: Option<String>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .open_file(Path::new(file_path), source.as_deref())
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn read_virtual_file_json_impl(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .read_file(Path::new(file_path))
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn list_virtual_files_json_impl(&self, dirty_only: bool) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .virtual_file_statuses(dirty_only)
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

    pub(super) fn commit_virtual_file_json_impl(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .commit_file(Path::new(file_path))
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn discard_virtual_file_json_impl(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .discard_file(Path::new(file_path))
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn close_virtual_file_json_impl(
        &self,
        file_path: &str,
        persist: bool,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .close_file(Path::new(file_path), persist)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
