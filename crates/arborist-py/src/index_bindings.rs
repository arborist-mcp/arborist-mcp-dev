use std::path::Path;

use arborist_core::{
    WorkspaceScanLimits, inspect_symbol_index, migrate_symbol_index,
    rebuild_symbol_index_with_limits, refresh_symbol_index_for_file_with_limits,
    refresh_symbol_index_with_limits,
};
use pyo3::prelude::*;

use crate::{ArboristCore, to_json_result, to_py_error};

struct WorkspaceIndexScan {
    limits: WorkspaceScanLimits,
}

impl WorkspaceIndexScan {
    fn new(max_files: usize, max_file_bytes: Option<u64>) -> Self {
        Self {
            limits: WorkspaceScanLimits {
                max_files,
                max_file_bytes,
            },
        }
    }
}

impl ArboristCore {
    pub(super) fn rebuild_symbol_index_json_impl(
        &self,
        workspace_root: &str,
        db_path: &str,
        max_files: usize,
        max_file_bytes: Option<u64>,
    ) -> PyResult<String> {
        let scan = WorkspaceIndexScan::new(max_files, max_file_bytes);
        let result = rebuild_symbol_index_with_limits(
            Path::new(workspace_root),
            Path::new(db_path),
            scan.limits,
        )
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn inspect_symbol_index_json_impl(&self, db_path: &str) -> PyResult<String> {
        let result = inspect_symbol_index(Path::new(db_path)).map_err(to_py_error)?;

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
    ) -> PyResult<String> {
        let scan = WorkspaceIndexScan::new(max_files, max_file_bytes);
        let result = refresh_symbol_index_with_limits(
            Path::new(workspace_root),
            Path::new(db_path),
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
    ) -> PyResult<String> {
        let scan = WorkspaceIndexScan::new(max_files, max_file_bytes);
        let result = refresh_symbol_index_for_file_with_limits(
            Path::new(workspace_root),
            Path::new(db_path),
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
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .register_symbol_index(Path::new(workspace_root), Path::new(db_path))
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
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow()
            .refresh_registered_symbol_indexes(max_files, max_file_bytes)
            .map_err(to_py_error)?;
        to_json_result(&result)
    }
}
