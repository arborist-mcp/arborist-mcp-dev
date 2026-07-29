use std::path::Path;

use arborist_core::{inspect_symbol_index_with_timeout, migrate_symbol_index_with_timeout};
use pyo3::prelude::*;

use crate::{ArboristCore, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(signature = (db_path, timeout_ms=None))]
    fn inspect_symbol_index_json(
        &self,
        db_path: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.inspect_symbol_index_json_impl(db_path, timeout_ms)
    }

    #[pyo3(signature = (db_path, timeout_ms=None))]
    fn migrate_symbol_index_json(
        &self,
        db_path: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.migrate_symbol_index_json_impl(db_path, timeout_ms)
    }
}

impl ArboristCore {
    pub(crate) fn inspect_symbol_index_json_impl(
        &self,
        db_path: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = inspect_symbol_index_with_timeout(Path::new(db_path), timeout_ms)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }

    pub(crate) fn migrate_symbol_index_json_impl(
        &self,
        db_path: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let result = migrate_symbol_index_with_timeout(Path::new(db_path), timeout_ms)
            .map_err(to_py_error)?;

        to_json_result(&result)
    }
}
