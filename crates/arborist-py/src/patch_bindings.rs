use std::path::Path;

use arborist_core::{
    PatchAstNodeResult, TraceSymbolGraphResult, WorkspacePositionEdits,
    export_patch_diagnostics_sarif, patch_ast_node, patch_ast_node_at_position,
    preview_patch_ast_node, preview_patch_ast_node_at_position,
    preview_patch_ast_node_at_position_from_path, preview_patch_ast_node_from_path,
    preview_workspace_position_edits, replay_patch_evidence_against_trace,
    validate_patch_commit_with_trace,
};
use pyo3::prelude::*;

use crate::{ArboristCore, parse_json_arg, source_position, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (file_path, semantic_path, new_code, source=None, bypass_reason=None))]
    fn patch_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        self.patch_ast_node_json_impl(file_path, semantic_path, new_code, source, bypass_reason)
    }

    #[pyo3(signature = (file_path, row, column, new_code, source=None, bypass_reason=None))]
    fn patch_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        self.patch_ast_node_at_position_json_impl(
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
        )
    }

    #[pyo3(signature = (file_path, semantic_path, new_code, source=None, bypass_reason=None))]
    fn preview_patch_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        self.preview_patch_ast_node_json_impl(
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
        )
    }

    #[pyo3(signature = (file_path, row, column, new_code, source=None, bypass_reason=None))]
    fn preview_patch_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        self.preview_patch_ast_node_at_position_json_impl(
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
        )
    }

    fn patch_virtual_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        self.patch_virtual_ast_node_json_impl(file_path, semantic_path, new_code, bypass_reason)
    }

    #[pyo3(signature = (file_path, row, column, new_code, bypass_reason=None))]
    fn patch_virtual_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        self.patch_virtual_ast_node_at_position_json_impl(
            file_path,
            row,
            column,
            new_code,
            bypass_reason,
        )
    }

    pub(super) fn replay_patch_evidence_against_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let trace: TraceSymbolGraphResult = parse_json_arg(trace_json)?;
        let result = replay_patch_evidence_against_trace(&patch, &trace).map_err(to_py_error)?;
        to_json_result(&result)
    }

    pub(super) fn export_patch_diagnostics_sarif_json(&self, patch_json: &str) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let result = export_patch_diagnostics_sarif(&patch).map_err(to_py_error)?;
        to_json_result(&result)
    }

    pub(super) fn preview_workspace_position_edits_json(
        &self,
        files_json: &str,
    ) -> PyResult<String> {
        let files: Vec<WorkspacePositionEdits> = parse_json_arg(files_json)?;
        let result = preview_workspace_position_edits(&files).map_err(to_py_error)?;
        to_json_result(&result)
    }

    pub(super) fn validate_patch_commit_with_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let trace: TraceSymbolGraphResult = parse_json_arg(trace_json)?;
        let result = validate_patch_commit_with_trace(&patch, &trace).map_err(to_py_error)?;
        to_json_result(&result)
    }
}

impl ArboristCore {
    pub(super) fn patch_ast_node_json_impl(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => patch_ast_node(
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
            ),
            None => {
                let mut vfs = self.vfs.borrow_mut();
                let result = vfs
                    .patch_node(
                        Path::new(file_path),
                        semantic_path,
                        new_code,
                        bypass_reason.as_deref(),
                    )
                    .map_err(to_py_error)?;
                if result.applied {
                    vfs.commit_file(Path::new(file_path)).map_err(to_py_error)?;
                }
                Ok(result)
            }
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn patch_ast_node_at_position_json_impl(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let position = source_position(row, column);
        let result = match source {
            Some(source) => patch_ast_node_at_position(
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
            ),
            None => {
                let mut vfs = self.vfs.borrow_mut();
                let result = vfs
                    .patch_node_at_position(
                        Path::new(file_path),
                        &position,
                        new_code,
                        bypass_reason.as_deref(),
                    )
                    .map_err(to_py_error)?;
                if result.applied {
                    vfs.commit_file(Path::new(file_path)).map_err(to_py_error)?;
                }
                Ok(result)
            }
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn preview_patch_ast_node_json_impl(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => preview_patch_ast_node(
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
            ),
            None => preview_patch_ast_node_from_path(
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn preview_patch_ast_node_at_position_json_impl(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let position = source_position(row, column);
        let result = match source {
            Some(source) => preview_patch_ast_node_at_position(
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
            ),
            None => preview_patch_ast_node_at_position_from_path(
                Path::new(file_path),
                &position,
                new_code,
                bypass_reason.as_deref(),
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn patch_virtual_ast_node_json_impl(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .patch_node(
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
            )
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(super) fn patch_virtual_ast_node_at_position_json_impl(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let position = source_position(row, column);
        let result = self
            .vfs
            .borrow_mut()
            .patch_node_at_position(
                Path::new(file_path),
                &position,
                new_code,
                bypass_reason.as_deref(),
            )
            .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
