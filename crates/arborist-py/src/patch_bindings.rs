use std::path::Path;

use arborist_core::{
    PatchAstNodeResult, TraceSymbolGraphResult, WorkspacePositionEdits,
    export_patch_diagnostics_sarif_with_timeout, patch_ast_node_at_position_with_timeout,
    patch_ast_node_with_timeout, preview_patch_ast_node_at_position_from_path_with_timeout,
    preview_patch_ast_node_at_position_with_timeout, preview_patch_ast_node_from_path_with_timeout,
    preview_patch_ast_node_with_timeout, preview_workspace_position_edits_with_timeout,
    replay_patch_evidence_against_trace_with_timeout,
    validate_patch_commit_with_trace_with_timeout,
};
use pyo3::prelude::*;

use crate::{ArboristCore, parse_json_arg, source_position, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (
        file_path,
        semantic_path,
        new_code,
        source=None,
        bypass_reason=None,
        timeout_ms=None
    ))]
    fn patch_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.patch_ast_node_json_impl(
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            timeout_ms,
        )
    }

    #[pyo3(signature = (
        file_path,
        row,
        column,
        new_code,
        source=None,
        bypass_reason=None,
        timeout_ms=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn patch_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.patch_ast_node_at_position_json_impl(
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
            timeout_ms,
        )
    }

    #[pyo3(signature = (
        file_path,
        semantic_path,
        new_code,
        source=None,
        bypass_reason=None,
        timeout_ms=None
    ))]
    fn preview_patch_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.preview_patch_ast_node_json_impl(
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            timeout_ms,
        )
    }

    #[pyo3(signature = (
        file_path,
        row,
        column,
        new_code,
        source=None,
        bypass_reason=None,
        timeout_ms=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn preview_patch_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.preview_patch_ast_node_at_position_json_impl(
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
            timeout_ms,
        )
    }

    #[pyo3(signature = (
        file_path,
        semantic_path,
        new_code,
        bypass_reason=None,
        timeout_ms=None
    ))]
    fn patch_virtual_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.patch_virtual_ast_node_json_impl(
            file_path,
            semantic_path,
            new_code,
            bypass_reason,
            timeout_ms,
        )
    }

    #[pyo3(signature = (
        file_path,
        row,
        column,
        new_code,
        bypass_reason=None,
        timeout_ms=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn patch_virtual_ast_node_at_position_json(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.patch_virtual_ast_node_at_position_json_impl(
            file_path,
            row,
            column,
            new_code,
            bypass_reason,
            timeout_ms,
        )
    }

    #[pyo3(
        name = "replay_patch_evidence_against_trace_json",
        signature = (patch_json, trace_json, timeout_ms=None)
    )]
    fn replay_patch_evidence_against_trace_json_binding(
        &self,
        patch_json: &str,
        trace_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.replay_patch_evidence_against_trace_json_with_timeout_impl(
            patch_json, trace_json, timeout_ms,
        )
    }

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
    pub(super) fn preview_workspace_position_edits_json(
        &self,
        files_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let files: Vec<WorkspacePositionEdits> = parse_json_arg(files_json)?;
        let result = preview_workspace_position_edits_with_timeout(&files, timeout_ms)
            .map_err(to_py_error)?;
        to_json_result(&result)
    }

    #[pyo3(
        name = "validate_patch_commit_with_trace_json",
        signature = (patch_json, trace_json, timeout_ms=None)
    )]
    fn validate_patch_commit_with_trace_json_binding(
        &self,
        patch_json: &str,
        trace_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.validate_patch_commit_with_trace_json_with_timeout_impl(
            patch_json, trace_json, timeout_ms,
        )
    }
}

impl ArboristCore {
    #[cfg(test)]
    pub(super) fn replay_patch_evidence_against_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        self.replay_patch_evidence_against_trace_json_with_timeout_impl(
            patch_json, trace_json, None,
        )
    }

    pub(super) fn replay_patch_evidence_against_trace_json_with_timeout_impl(
        &self,
        patch_json: &str,
        trace_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let trace: TraceSymbolGraphResult = parse_json_arg(trace_json)?;
        let result = replay_patch_evidence_against_trace_with_timeout(&patch, &trace, timeout_ms)
            .map_err(to_py_error)?;
        to_json_result(&result)
    }

    pub(super) fn export_patch_diagnostics_sarif_json_with_timeout_impl(
        &self,
        patch_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let result =
            export_patch_diagnostics_sarif_with_timeout(&patch, timeout_ms).map_err(to_py_error)?;
        to_json_result(&result)
    }

    #[cfg(test)]
    pub(super) fn validate_patch_commit_with_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        self.validate_patch_commit_with_trace_json_with_timeout_impl(patch_json, trace_json, None)
    }

    pub(super) fn validate_patch_commit_with_trace_json_with_timeout_impl(
        &self,
        patch_json: &str,
        trace_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let trace: TraceSymbolGraphResult = parse_json_arg(trace_json)?;
        let result = validate_patch_commit_with_trace_with_timeout(&patch, &trace, timeout_ms)
            .map_err(to_py_error)?;
        to_json_result(&result)
    }

    pub(super) fn patch_ast_node_json_impl(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => patch_ast_node_with_timeout(
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
            None => self.vfs.borrow_mut().patch_node_and_commit_with_timeout(
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn patch_ast_node_at_position_json_impl(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let position = source_position(row, column);
        let result = match source {
            Some(source) => patch_ast_node_at_position_with_timeout(
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
            None => self
                .vfs
                .borrow_mut()
                .patch_node_at_position_and_commit_with_timeout(
                    Path::new(file_path),
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    timeout_ms,
                ),
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
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => preview_patch_ast_node_with_timeout(
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
            None => preview_patch_ast_node_from_path_with_timeout(
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn preview_patch_ast_node_at_position_json_impl(
        &self,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let position = source_position(row, column);
        let result = match source {
            Some(source) => preview_patch_ast_node_at_position_with_timeout(
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            ),
            None => preview_patch_ast_node_at_position_from_path_with_timeout(
                Path::new(file_path),
                &position,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
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
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .patch_node_with_timeout(
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
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
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let position = source_position(row, column);
        let result = self
            .vfs
            .borrow_mut()
            .patch_node_at_position_with_timeout(
                Path::new(file_path),
                &position,
                new_code,
                bypass_reason.as_deref(),
                timeout_ms,
            )
            .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
