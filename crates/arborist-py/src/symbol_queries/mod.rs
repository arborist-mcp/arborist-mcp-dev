use std::path::{Path, PathBuf};

use pyo3::PyResult;
use pyo3::exceptions::PyValueError;

mod list;
mod read;
mod search;
mod trace;

pub(crate) struct SymbolQueryContext {
    workspace_root: PathBuf,
    index_db_path: Option<PathBuf>,
    file_path: Option<PathBuf>,
    source: Option<String>,
}

impl SymbolQueryContext {
    pub(crate) fn new(
        workspace_root: &str,
        index_db_path: Option<String>,
        file_path: Option<String>,
        source: Option<String>,
    ) -> Self {
        Self {
            workspace_root: PathBuf::from(workspace_root),
            index_db_path: index_db_path.map(PathBuf::from),
            file_path: file_path.map(PathBuf::from),
            source,
        }
    }

    pub(crate) fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub(crate) fn index_db_path(&self) -> Option<&Path> {
        self.index_db_path.as_deref()
    }

    pub(crate) fn file_path(&self) -> Option<&Path> {
        self.file_path.as_deref()
    }

    pub(crate) fn position_file_path(&self) -> &Path {
        self.file_path
            .as_deref()
            .expect("position queries always provide a file path")
    }

    pub(crate) fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    pub(crate) fn source_file_path(&self) -> PyResult<&Path> {
        self.file_path()
            .ok_or_else(|| PyValueError::new_err("file_path is required when source is provided"))
    }
}
