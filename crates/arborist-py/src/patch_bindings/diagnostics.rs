use arborist_core::{
    PatchAstNodeResult, WorkspacePositionEdits, export_patch_diagnostics_sarif_with_timeout,
    preview_workspace_position_edits_with_timeout,
};
use pyo3::prelude::*;

use crate::{ArboristCore, parse_json_arg, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(
        name = "export_patch_diagnostics_sarif_json",
        signature = (patch_json, timeout_ms=None)
    )]
    fn export_patch_diagnostics_sarif_json_binding(
        &self,
        patch_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.export_patch_diagnostics_sarif_json_with_timeout_impl(patch_json, timeout_ms)
    }

    #[pyo3(signature = (files_json, timeout_ms=None))]
    pub(crate) fn preview_workspace_position_edits_json(
        &self,
        files_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let files: Vec<WorkspacePositionEdits> = parse_json_arg(files_json)?;
        let result = preview_workspace_position_edits_with_timeout(&files, timeout_ms)
            .map_err(to_py_error)?;
        to_json_result(&result)
    }
}

impl ArboristCore {
    pub(crate) fn export_patch_diagnostics_sarif_json_with_timeout_impl(
        &self,
        patch_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let result =
            export_patch_diagnostics_sarif_with_timeout(&patch, timeout_ms).map_err(to_py_error)?;
        to_json_result(&result)
    }
}
