use std::path::Path;

use pyo3::prelude::*;

use crate::{ArboristCore, to_json_result, to_py_error};

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
}

impl ArboristCore {
    pub(crate) fn open_virtual_file_json_impl(
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

    pub(crate) fn read_virtual_file_json_impl(
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

    pub(crate) fn list_virtual_files_json_impl(
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
}
