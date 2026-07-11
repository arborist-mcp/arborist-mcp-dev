use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{ensure_path_inside_workspace, normalize_absolute_path};
use crate::source_overlay::source_override_for_path;

mod list;
mod read;
mod search;
mod trace;

#[derive(Debug, Clone)]
enum SymbolQueryBackend {
    Workspace(PathBuf),
    Index(PathBuf),
}

#[derive(Debug, Clone)]
pub struct SymbolQueryContext {
    backend: SymbolQueryBackend,
    file_overrides: BTreeMap<String, String>,
}

impl SymbolQueryContext {
    pub fn workspace(workspace_root: &Path) -> Result<Self> {
        Ok(Self {
            backend: SymbolQueryBackend::Workspace(normalize_absolute_path(workspace_root)?),
            file_overrides: BTreeMap::new(),
        })
    }

    pub fn index(db_path: &Path) -> Result<Self> {
        Ok(Self {
            backend: SymbolQueryBackend::Index(normalize_absolute_path(db_path)?),
            file_overrides: BTreeMap::new(),
        })
    }

    pub fn with_source_overlay(mut self, file_path: &Path, source: &str) -> Result<Self> {
        self.add_source_overlay(file_path, source)?;
        Ok(self)
    }

    pub fn add_source_overlay(&mut self, file_path: &Path, source: &str) -> Result<()> {
        let (file_path, file_override) = source_override_for_path(file_path, source)?;
        if let SymbolQueryBackend::Workspace(workspace_root) = &self.backend {
            ensure_path_inside_workspace(workspace_root, &file_path)?;
        }
        self.file_overrides.extend(file_override);
        Ok(())
    }

    fn dispatch<T>(
        &self,
        workspace: impl FnOnce(&Path, &BTreeMap<String, String>) -> Result<T>,
        index: impl FnOnce(&Path, &BTreeMap<String, String>) -> Result<T>,
    ) -> Result<T> {
        match &self.backend {
            SymbolQueryBackend::Workspace(workspace_root) => {
                workspace(workspace_root, &self.file_overrides)
            }
            SymbolQueryBackend::Index(db_path) => index(db_path, &self.file_overrides),
        }
    }
}
