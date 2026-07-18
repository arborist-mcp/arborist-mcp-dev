use std::path::Path;

use arborist_core::{
    patch_ast_node, patch_ast_node_at_position, preview_patch_ast_node,
    preview_patch_ast_node_at_position, preview_patch_ast_node_at_position_from_path,
    preview_patch_ast_node_from_path,
};
use pyo3::prelude::*;

use crate::{ArboristCore, source_position, to_json_result, to_py_error};

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
