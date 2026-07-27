use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::language::normalize_absolute_path;
use crate::source_overlay::normalize_source_overrides_for_workspace;
use crate::source_overlay::source_override_for_path;

mod list;
mod read;
mod search;
mod trace;

pub const MAX_SYMBOL_LIMIT: usize = 10_000;

pub(crate) fn validate_symbol_limit(limit: usize) -> Result<()> {
    if limit > MAX_SYMBOL_LIMIT {
        return Err(anyhow!("symbol limit must not exceed {}", MAX_SYMBOL_LIMIT));
    }
    Ok(())
}

#[derive(Debug, Clone)]
enum SymbolQueryBackend {
    Workspace(PathBuf),
    Index(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::{MAX_SYMBOL_LIMIT, validate_symbol_limit};

    #[test]
    fn validates_symbol_limit_bounds() {
        assert!(validate_symbol_limit(0).is_ok());
        assert!(validate_symbol_limit(MAX_SYMBOL_LIMIT).is_ok());
        assert!(validate_symbol_limit(MAX_SYMBOL_LIMIT + 1).is_err());
    }
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
        let (_, file_override) = source_override_for_path(file_path, source)?;
        if let SymbolQueryBackend::Workspace(workspace_root) = &self.backend {
            self.file_overrides
                .extend(normalize_source_overrides_for_workspace(
                    workspace_root,
                    &file_override,
                    "workspace",
                )?);
        } else {
            self.file_overrides.extend(file_override);
        }
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

    pub(crate) fn dispatch_with_timeout<T>(
        &self,
        timeout_ms: Option<u64>,
        workspace: impl FnOnce(&Path, &BTreeMap<String, String>, Option<u64>) -> Result<T>,
        index: impl FnOnce(&Path, &BTreeMap<String, String>, Option<u64>) -> Result<T>,
    ) -> Result<T> {
        match &self.backend {
            SymbolQueryBackend::Workspace(workspace_root) => {
                workspace(workspace_root, &self.file_overrides, timeout_ms)
            }
            SymbolQueryBackend::Index(db_path) => index(db_path, &self.file_overrides, timeout_ms),
        }
    }
}
