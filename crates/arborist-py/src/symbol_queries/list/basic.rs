use arborist_core::{
    list_symbols_from_index_filtered_with_timeout,
    list_symbols_from_index_with_source_filtered_with_timeout,
    list_symbols_in_file_with_source_filtered_with_timeout,
    list_symbols_with_source_filtered_with_timeout,
};
use pyo3::prelude::*;

use super::super::SymbolQueryContext;
use crate::{ArboristCore, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (workspace_root, limit=100, index_db_path=None, file_path_contains=None, node_kind=None, file_path=None, source=None, timeout_ms=None))]
    #[allow(clippy::too_many_arguments)]
    fn list_symbols_json(
        &self,
        workspace_root: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.list_symbols_json_impl(
            workspace_root,
            limit,
            index_db_path,
            file_path_contains,
            node_kind,
            file_path,
            source,
            timeout_ms,
        )
    }
}

impl ArboristCore {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn list_symbols_json_impl(
        &self,
        workspace_root: &str,
        limit: usize,
        index_db_path: Option<String>,
        file_path_contains: Option<String>,
        node_kind: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let context = SymbolQueryContext::new(workspace_root, index_db_path, file_path, source);
        let result = match (context.source(), context.index_db_path()) {
            (Some(source), Some(index_db_path)) => {
                list_symbols_from_index_with_source_filtered_with_timeout(
                    index_db_path,
                    context.source_file_path()?,
                    source,
                    limit,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                    timeout_ms,
                )
            }
            (Some(source), None) => list_symbols_with_source_filtered_with_timeout(
                context.workspace_root(),
                context.source_file_path()?,
                source,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
                timeout_ms,
            ),
            (None, Some(index_db_path)) => list_symbols_from_index_filtered_with_timeout(
                index_db_path,
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
                timeout_ms,
            ),
            (None, None) if context.file_path().is_some() => {
                let file_path = context.required_file_path()?;
                let snapshot = self
                    .vfs
                    .borrow_mut()
                    .read_file_with_timeout(file_path, timeout_ms)
                    .map_err(to_py_error)?;
                list_symbols_in_file_with_source_filtered_with_timeout(
                    context.workspace_root(),
                    file_path,
                    &snapshot.source,
                    limit,
                    file_path_contains.as_deref(),
                    node_kind.as_deref(),
                    timeout_ms,
                )
            }
            (None, None) => self.vfs.borrow_mut().list_symbols_filtered_with_timeout(
                context.workspace_root(),
                limit,
                file_path_contains.as_deref(),
                node_kind.as_deref(),
                timeout_ms,
            ),
        }
        .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
