use std::path::Path;

use anyhow::Result;

use super::{MAX_SYMBOL_INDEX_REGISTRY_TIMEOUT_MS, VirtualFileSystem};
use crate::deadline::{CooperativeDeadline, DeadlineCheck};
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
        self.unregister_symbol_index_with_timeout(workspace_root, None)
    }

    pub fn unregister_symbol_index_with_timeout(
        &mut self,
        workspace_root: &Path,
        timeout_ms: Option<u64>,
    ) -> Result<bool> {
        let deadline = CooperativeDeadline::new(
            timeout_ms,
            MAX_SYMBOL_INDEX_REGISTRY_TIMEOUT_MS,
            "symbol index registry",
        )?;
        self.unregister_symbol_index_with_deadline(workspace_root, &deadline)
    }

    fn unregister_symbol_index_with_deadline(
        &mut self,
        workspace_root: &Path,
        deadline: &dyn DeadlineCheck,
    ) -> Result<bool> {
        deadline.check("workspace path normalization")?;
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let workspace_key = normalize_path(&workspace_root);
        deadline.check("registry mutation")?;
        Ok(self.symbol_indexes.remove(&workspace_key).is_some())
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
        self.registered_symbol_indexes_checked_with_timeout(None)
    }

    pub fn registered_symbol_indexes_checked_with_timeout(
        &self,
        timeout_ms: Option<u64>,
    ) -> Result<Vec<RegisteredSymbolIndex>> {
        let deadline = CooperativeDeadline::new(
            timeout_ms,
            MAX_SYMBOL_INDEX_REGISTRY_TIMEOUT_MS,
            "symbol index registry",
        )?;
        self.registered_symbol_indexes_checked_with_deadline(&deadline)
    }

    fn registered_symbol_indexes_checked_with_deadline(
        &self,
        deadline: &dyn DeadlineCheck,
    ) -> Result<Vec<RegisteredSymbolIndex>> {
        let mut indexes = Vec::with_capacity(self.symbol_indexes.len());
        for (workspace_root, db_path) in &self.symbol_indexes {
            deadline.check("registry collection")?;
            indexes.push(RegisteredSymbolIndex {
                workspace_root: workspace_root.clone(),
                db_path: normalize_path(db_path),
            });
        }
        deadline.check("registry sorting")?;
        indexes.sort_by(|left, right| left.workspace_root.cmp(&right.workspace_root));
        deadline.check("registry sorting")?;
        for (index, registered) in indexes.iter().enumerate() {
            deadline.check("result validation")?;
            registered.validate_public_output(index)?;
        }
        deadline.check("result validation")?;
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use anyhow::{Result, bail};

    use super::*;
    use crate::deadline::DeadlineCheck;
    use crate::vfs::tests::temp_workspace;

    struct FailOnCheck {
        checks: Cell<usize>,
        fail_on: usize,
    }

    impl DeadlineCheck for FailOnCheck {
        fn check(&self, phase: &str) -> Result<()> {
            let check = self.checks.get() + 1;
            self.checks.set(check);
            if check == self.fail_on {
                bail!("forced timeout during {phase}");
            }
            Ok(())
        }
    }

    #[test]
    fn validates_registry_timeout_bounds() {
        let workspace = temp_workspace();
        let mut vfs = VirtualFileSystem::new();

        let unregister_zero = vfs
            .unregister_symbol_index_with_timeout(&workspace, Some(0))
            .expect_err("zero timeout should fail");
        assert!(unregister_zero.to_string().contains("timeout_ms"));
        let list_excessive = vfs
            .registered_symbol_indexes_checked_with_timeout(Some(
                MAX_SYMBOL_INDEX_REGISTRY_TIMEOUT_MS + 1,
            ))
            .expect_err("excessive timeout should fail");
        assert!(list_excessive.to_string().contains("must not exceed"));
    }

    #[test]
    fn unregister_timeout_before_mutation_preserves_registration() {
        let workspace = temp_workspace();
        let workspace = normalize_absolute_path(&workspace).unwrap();
        let workspace_key = normalize_path(&workspace);
        let mut vfs = VirtualFileSystem::new();
        vfs.symbol_indexes
            .insert(workspace_key.clone(), workspace.join("symbols.db"));
        let deadline = FailOnCheck {
            checks: Cell::new(0),
            fail_on: 2,
        };

        let error = vfs
            .unregister_symbol_index_with_deadline(&workspace, &deadline)
            .expect_err("pre-mutation timeout should fail");

        assert!(error.to_string().contains("registry mutation"));
        assert!(vfs.symbol_indexes.contains_key(&workspace_key));
    }

    #[test]
    fn list_timeout_can_interrupt_registry_collection() {
        let workspace = temp_workspace();
        let workspace = normalize_absolute_path(&workspace).unwrap();
        let mut vfs = VirtualFileSystem::new();
        vfs.symbol_indexes
            .insert(normalize_path(&workspace), workspace.join("symbols.db"));
        let deadline = FailOnCheck {
            checks: Cell::new(0),
            fail_on: 1,
        };

        let error = vfs
            .registered_symbol_indexes_checked_with_deadline(&deadline)
            .expect_err("collection timeout should fail");

        assert!(error.to_string().contains("registry collection"));
    }
}
