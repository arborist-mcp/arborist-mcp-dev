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

    pub(crate) fn required_file_path(&self) -> PyResult<&Path> {
        self.file_path
            .as_deref()
            .ok_or_else(|| PyValueError::new_err("file_path is required"))
    }

    pub(crate) fn position_file_path(&self) -> PyResult<&Path> {
        self.file_path
            .as_deref()
            .ok_or_else(|| PyValueError::new_err("file_path is required for position queries"))
    }

    pub(crate) fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    pub(crate) fn source_file_path(&self) -> PyResult<&Path> {
        self.file_path()
            .ok_or_else(|| PyValueError::new_err("file_path is required when source is provided"))
    }
}

#[cfg(test)]
mod tests {
    use super::SymbolQueryContext;

    #[test]
    fn source_context_exposes_owned_paths_and_source() {
        let context = SymbolQueryContext::new(
            "workspace",
            Some("symbols.db".to_string()),
            Some("src/main.cpp".to_string()),
            Some("int main() {}".to_string()),
        );

        assert_eq!(context.workspace_root().to_string_lossy(), "workspace");
        assert_eq!(
            context.index_db_path().map(|path| path.to_string_lossy()),
            Some("symbols.db".into())
        );
        assert_eq!(
            context.file_path().map(|path| path.to_string_lossy()),
            Some("src/main.cpp".into())
        );
        assert_eq!(context.source(), Some("int main() {}"));
        assert!(context.source_file_path().is_ok());
        assert!(context.required_file_path().is_ok());
        assert!(context.position_file_path().is_ok());
    }

    #[test]
    fn source_file_path_requires_a_file_path() {
        let context = SymbolQueryContext::new("workspace", None, None, Some("source".into()));

        assert!(context.source_file_path().is_err());
        assert!(context.required_file_path().is_err());
        assert!(context.position_file_path().is_err());
    }
}
