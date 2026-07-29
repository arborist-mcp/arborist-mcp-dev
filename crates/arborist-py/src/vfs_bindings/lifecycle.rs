use std::path::Path;

use pyo3::prelude::*;

use crate::{ArboristCore, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
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
    pub(crate) fn commit_virtual_file_json_impl(
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

    pub(crate) fn discard_virtual_file_json_impl(
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

    pub(crate) fn close_virtual_file_json_impl(
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
