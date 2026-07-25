use std::path::Path;

use anyhow::Result;

use super::VirtualFileSystem;
use crate::language::{normalize_absolute_path, normalize_path};
use crate::model::{RegisteredSymbolIndex, SymbolIndexStats};
use crate::symbols::{rebuild_symbol_index_with_limits, refresh_symbol_index_with_limits};
use crate::workspace_scan::{WorkspaceScanLimits, validate_workspace_scan_limits};

impl VirtualFileSystem {
    pub fn register_symbol_index(
        &mut self,
        workspace_root: &Path,
        db_path: &Path,
    ) -> Result<SymbolIndexStats> {
        self.register_symbol_index_with_scan_limits(
            workspace_root,
            db_path,
            WorkspaceScanLimits::default(),
        )
    }

    pub fn register_symbol_index_with_limits(
        &mut self,
        workspace_root: &Path,
        db_path: &Path,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> Result<SymbolIndexStats> {
        self.register_symbol_index_with_scan_limits(
            workspace_root,
            db_path,
            WorkspaceScanLimits {
                max_files,
                max_file_bytes,
                timeout_ms,
            },
        )
    }

    fn register_symbol_index_with_scan_limits(
        &mut self,
        workspace_root: &Path,
        db_path: &Path,
        limits: WorkspaceScanLimits,
    ) -> Result<SymbolIndexStats> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let db_path = normalize_absolute_path(db_path)?;
        let stats = rebuild_symbol_index_with_limits(&workspace_root, &db_path, limits)?;
        self.symbol_indexes
            .insert(normalize_path(&workspace_root), db_path);
        Ok(stats)
    }

    pub fn unregister_symbol_index(&mut self, workspace_root: &Path) -> Result<bool> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        Ok(self
            .symbol_indexes
            .remove(&normalize_path(&workspace_root))
            .is_some())
    }

    pub fn registered_symbol_indexes(&self) -> Vec<RegisteredSymbolIndex> {
        let mut indexes: Vec<_> = self
            .symbol_indexes
            .iter()
            .map(|(workspace_root, db_path)| RegisteredSymbolIndex {
                workspace_root: workspace_root.clone(),
                db_path: normalize_path(db_path),
            })
            .collect();
        indexes.sort_by(|left, right| left.workspace_root.cmp(&right.workspace_root));
        indexes
    }

    pub fn registered_symbol_indexes_checked(&self) -> Result<Vec<RegisteredSymbolIndex>> {
        let indexes = self.registered_symbol_indexes();
        for (index, registered) in indexes.iter().enumerate() {
            registered.validate_public_output(index)?;
        }
        Ok(indexes)
    }

    pub fn refresh_registered_symbol_indexes(
        &self,
        max_files: usize,
        max_file_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> Result<Vec<SymbolIndexStats>> {
        let limits = WorkspaceScanLimits {
            max_files,
            max_file_bytes,
            timeout_ms,
        };
        validate_workspace_scan_limits(limits)?;

        self.registered_symbol_indexes()
            .into_iter()
            .map(|registered| {
                refresh_symbol_index_with_limits(
                    Path::new(&registered.workspace_root),
                    Path::new(&registered.db_path),
                    limits,
                )
            })
            .collect()
    }
}
