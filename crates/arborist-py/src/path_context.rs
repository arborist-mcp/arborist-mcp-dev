use std::path::{Path, PathBuf};

pub(super) struct WorkspaceIndexPathContext {
    workspace_root: PathBuf,
    db_path: PathBuf,
}

impl WorkspaceIndexPathContext {
    pub(super) fn new(workspace_root: &str, db_path: &str) -> Self {
        Self {
            workspace_root: PathBuf::from(workspace_root),
            db_path: PathBuf::from(db_path),
        }
    }

    pub(super) fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub(super) fn db_path(&self) -> &Path {
        &self.db_path
    }
}

#[cfg(test)]
mod tests {
    use super::WorkspaceIndexPathContext;

    #[test]
    fn contexts_expose_owned_paths() {
        let index = WorkspaceIndexPathContext::new("workspace", "symbols.db");

        assert_eq!(index.workspace_root().to_string_lossy(), "workspace");
        assert_eq!(index.db_path().to_string_lossy(), "symbols.db");
    }
}
