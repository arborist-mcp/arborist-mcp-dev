use std::path::Path;

use pyo3::prelude::*;

use crate::{ArboristCore, path_context::WorkspaceIndexPathContext, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (workspace_root, db_path, max_files=20_000, max_file_bytes=None, timeout_ms=None))]
    fn register_symbol_index_json(
        &self,
        workspace_root: &str,
        db_path: &str,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.register_symbol_index_json_impl(
            workspace_root,
            db_path,
            max_files,
            max_file_bytes,
            timeout_ms,
        )
    }

    #[pyo3(signature = (workspace_root, timeout_ms=None))]
    fn unregister_symbol_index_json(
        &self,
        workspace_root: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<bool> {
        self.unregister_symbol_index_json_impl(workspace_root, timeout_ms)
    }

    #[pyo3(signature = (timeout_ms=None))]
    fn list_symbol_indexes_json(&self, timeout_ms: Option<u64>) -> PyResult<String> {
        self.list_symbol_indexes_json_impl(timeout_ms)
    }

    #[pyo3(signature = (max_files=20_000, max_file_bytes=None, timeout_ms=None))]
    fn refresh_registered_symbol_indexes_json(
        &self,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.refresh_registered_symbol_indexes_json_impl(max_files, max_file_bytes, timeout_ms)
    }
}

impl ArboristCore {
    pub(crate) fn register_symbol_index_json_impl(
        &self,
        workspace_root: &str,
        db_path: &str,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let context = WorkspaceIndexPathContext::new(workspace_root, db_path);
        let result = self
            .vfs
            .borrow_mut()
            .register_symbol_index_with_limits(
                context.workspace_root(),
                context.db_path(),
                max_files,
                max_file_bytes,
                timeout_ms,
            )
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(crate) fn unregister_symbol_index_json_impl(
        &self,
        workspace_root: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<bool> {
        self.vfs
            .borrow_mut()
            .unregister_symbol_index_with_timeout(Path::new(workspace_root), timeout_ms)
            .map_err(to_py_error)
    }

    pub(crate) fn list_symbol_indexes_json_impl(
        &self,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow()
            .registered_symbol_indexes_checked_with_timeout(timeout_ms)
            .map_err(to_py_error)?;
        to_json_result(&result)
    }

    pub(crate) fn refresh_registered_symbol_indexes_json_impl(
        &self,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow()
            .refresh_registered_symbol_indexes(max_files, max_file_bytes, timeout_ms)
            .map_err(to_py_error)?;
        to_json_result(&result)
    }
}
