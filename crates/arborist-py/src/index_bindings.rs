use std::path::Path;

use arborist_core::{
    WorkspaceScanLimits, inspect_symbol_index_with_timeout, migrate_symbol_index,
    rebuild_symbol_index_with_limits, refresh_symbol_index_for_file_with_limits,
    refresh_symbol_index_with_limits,
};
use pyo3::prelude::*;

use crate::{ArboristCore, path_context::WorkspaceIndexPathContext, to_json_result, to_py_error};

struct WorkspaceIndexScan {
    limits: WorkspaceScanLimits,
}

impl WorkspaceIndexScan {
    fn new(max_files: usize, max_file_bytes: Option<u64>, timeout_ms: Option<u64>) -> Self {
        Self {
            limits: WorkspaceScanLimits {
                max_files,
                max_file_bytes,
                timeout_ms,
            },
        }
    }
}

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (workspace_root, db_path, max_files=20_000, max_file_bytes=None, timeout_ms=None))]
    fn rebuild_symbol_index_json(
        &self,
        workspace_root: &str,
        db_path: &str,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.rebuild_symbol_index_json_impl(
            workspace_root,
            db_path,
            max_files,
            max_file_bytes,
            timeout_ms,
        )
    }

    #[pyo3(signature = (db_path, timeout_ms=None))]
    fn inspect_symbol_index_json(
        &self,
        db_path: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.inspect_symbol_index_json_impl(db_path, timeout_ms)
    }

    fn migrate_symbol_index_json(&self, db_path: &str) -> PyResult<String> {
        self.migrate_symbol_index_json_impl(db_path)
    }

    #[pyo3(signature = (workspace_root, db_path, max_files=20_000, max_file_bytes=None, timeout_ms=None))]
    fn refresh_symbol_index_json(
        &self,
        workspace_root: &str,
        db_path: &str,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.refresh_symbol_index_json_impl(
            workspace_root,
            db_path,
            max_files,
            max_file_bytes,
            timeout_ms,
        )
    }

    #[pyo3(signature = (workspace_root, db_path, file_path, max_files=20_000, max_file_bytes=None, timeout_ms=None))]
    fn refresh_symbol_index_for_file_json(
        &self,
        workspace_root: &str,
        db_path: &str,
        file_path: &str,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.refresh_symbol_index_for_file_json_impl(
            workspace_root,
            db_path,
            file_path,
            max_files,
            max_file_bytes,
            timeout_ms,
        )
    }

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

    fn unregister_symbol_index_json(&self, workspace_root: &str) -> PyResult<bool> {
        self.unregister_symbol_index_json_impl(workspace_root)
    }

    fn list_symbol_indexes_json(&self) -> PyResult<String> {
        self.list_symbol_indexes_json_impl()
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
    pub(super) fn rebuild_symbol_index_json_impl(
        &self,
        workspace_root: &str,
        db_path: &str,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let context = WorkspaceIndexPathContext::new(workspace_root, db_path);
        let scan = WorkspaceIndexScan::new(max_files, max_file_bytes, timeout_ms);
        let result = rebuild_symbol_index_with_limits(
            context.workspace_root(),
            context.db_path(),
            scan.limits,
        )
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn inspect_symbol_index_json_impl(
        &self,
        db_path: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = inspect_symbol_index_with_timeout(Path::new(db_path), timeout_ms)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn migrate_symbol_index_json_impl(&self, db_path: &str) -> PyResult<String> {
        let result = migrate_symbol_index(Path::new(db_path)).map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn refresh_symbol_index_json_impl(
        &self,
        workspace_root: &str,
        db_path: &str,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let context = WorkspaceIndexPathContext::new(workspace_root, db_path);
        let scan = WorkspaceIndexScan::new(max_files, max_file_bytes, timeout_ms);
        let result = refresh_symbol_index_with_limits(
            context.workspace_root(),
            context.db_path(),
            scan.limits,
        )
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn refresh_symbol_index_for_file_json_impl(
        &self,
        workspace_root: &str,
        db_path: &str,
        file_path: &str,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let context = WorkspaceIndexPathContext::new(workspace_root, db_path);
        let scan = WorkspaceIndexScan::new(max_files, max_file_bytes, timeout_ms);
        let result = refresh_symbol_index_for_file_with_limits(
            context.workspace_root(),
            context.db_path(),
            Path::new(file_path),
            scan.limits,
        )
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn register_symbol_index_json_impl(
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

    pub(super) fn unregister_symbol_index_json_impl(&self, workspace_root: &str) -> PyResult<bool> {
        self.vfs
            .borrow_mut()
            .unregister_symbol_index(Path::new(workspace_root))
            .map_err(to_py_error)
    }

    pub(super) fn list_symbol_indexes_json_impl(&self) -> PyResult<String> {
        let result = self
            .vfs
            .borrow()
            .registered_symbol_indexes_checked()
            .map_err(to_py_error)?;
        to_json_result(&result)
    }

    pub(super) fn refresh_registered_symbol_indexes_json_impl(
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
