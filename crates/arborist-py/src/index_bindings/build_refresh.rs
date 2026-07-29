use std::path::Path;

use arborist_core::{
    WorkspaceScanLimits, rebuild_symbol_index_with_limits,
    refresh_symbol_index_for_file_with_limits, refresh_symbol_index_with_limits,
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
}

impl ArboristCore {
    pub(crate) fn rebuild_symbol_index_json_impl(
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

    pub(crate) fn refresh_symbol_index_json_impl(
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

    pub(crate) fn refresh_symbol_index_for_file_json_impl(
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
}
