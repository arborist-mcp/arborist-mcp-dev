use std::path::Path;

use arborist_core::{
    Position, validate_patch_with_discovery_context,
    validate_patch_with_discovery_context_at_position,
    validate_patch_with_discovery_context_at_position_from_index,
    validate_patch_with_discovery_context_from_index, validate_patch_with_graph_context,
    validate_patch_with_graph_context_at_position,
    validate_patch_with_graph_context_at_position_from_index,
    validate_patch_with_graph_context_from_index, validate_patch_with_neighborhood_context,
    validate_patch_with_neighborhood_context_at_position,
    validate_patch_with_neighborhood_context_at_position_from_index,
    validate_patch_with_neighborhood_context_from_index, validate_patch_with_trace_context,
    validate_patch_with_trace_context_at_position,
    validate_patch_with_trace_context_at_position_from_index,
    validate_patch_with_trace_context_from_index,
};
use pyo3::prelude::*;

use crate::{ArboristCore, parse_direction, to_json_result, to_py_error};

impl ArboristCore {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_patch_with_trace_context_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => validate_patch_with_trace_context_from_index(
                Path::new(&index_db_path),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
            (Some(source), None) => validate_patch_with_trace_context(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_trace_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                )
            }
            (None, None) => self.vfs.borrow_mut().validate_patch_with_trace_context(
                Path::new(workspace_root),
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_patch_with_trace_context_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_trace_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                )
            }
            (Some(source), None) => validate_patch_with_trace_context_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_trace_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_trace_context_at_position(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_patch_with_graph_context_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => validate_patch_with_graph_context_from_index(
                Path::new(&index_db_path),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (Some(source), None) => validate_patch_with_graph_context(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_graph_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self.vfs.borrow_mut().validate_patch_with_graph_context(
                Path::new(workspace_root),
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_patch_with_graph_context_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_graph_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => validate_patch_with_graph_context_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_graph_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_graph_context_at_position(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_patch_with_neighborhood_context_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_neighborhood_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => validate_patch_with_neighborhood_context(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_neighborhood_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_neighborhood_context(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_patch_with_neighborhood_context_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_neighborhood_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => validate_patch_with_neighborhood_context_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_neighborhood_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_neighborhood_context_at_position(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_patch_with_discovery_context_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_discovery_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => validate_patch_with_discovery_context(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_discovery_context_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    semantic_path,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self.vfs.borrow_mut().validate_patch_with_discovery_context(
                Path::new(workspace_root),
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_patch_with_discovery_context_at_position_json_impl(
        &self,
        workspace_root: &str,
        file_path: &str,
        row: usize,
        column: usize,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
        max_depth: usize,
        max_nodes: usize,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let position = Position { row, column };
        let result = match (source, index_db_path) {
            (Some(source), Some(index_db_path)) => {
                validate_patch_with_discovery_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (Some(source), None) => validate_patch_with_discovery_context_at_position(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                &position,
                new_code,
                bypass_reason.as_deref(),
                direction,
                max_depth,
                max_nodes,
            ),
            (None, Some(index_db_path)) => {
                let source =
                    arborist_core::read_source(Path::new(file_path)).map_err(to_py_error)?;
                validate_patch_with_discovery_context_at_position_from_index(
                    Path::new(&index_db_path),
                    Path::new(file_path),
                    &source,
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                )
            }
            (None, None) => self
                .vfs
                .borrow_mut()
                .validate_patch_with_discovery_context_at_position(
                    Path::new(workspace_root),
                    Path::new(file_path),
                    &position,
                    new_code,
                    bypass_reason.as_deref(),
                    direction,
                    max_depth,
                    max_nodes,
                ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
