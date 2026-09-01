use std::path::Path;

use anyhow::{Context, Result, anyhow};

use super::super::state::{normalized_virtual_path, read_virtual_disk_source, snapshot_from_entry};
use super::super::{
    MAX_VIRTUAL_FILE_COMMIT_TIMEOUT_MS, MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS, VirtualFileSystem,
};
use super::check_optional_deadline;
use crate::deadline::{CooperativeDeadline, DeadlineCheck};
use crate::language::{
    normalize_absolute_path, parse_document_with_timeout, path_is_inside_workspace,
    write_source_atomic,
};
use crate::model::VirtualFileSnapshot;
use crate::symbols::refresh_symbol_index_for_file;

impl VirtualFileSystem {
    pub fn open_file(&mut self, path: &Path, source: Option<&str>) -> Result<VirtualFileSnapshot> {
        let (path, normalized) = normalized_virtual_path(path)?;
        self.open_file_inner(&path, &normalized, source, None)
    }

    pub fn open_file_with_timeout(
        &mut self,
        path: &Path,
        source: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<VirtualFileSnapshot> {
        let Some(timeout_ms) = timeout_ms else {
            return self.open_file(path, source);
        };
        let deadline = CooperativeDeadline::new(
            Some(timeout_ms),
            MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS,
            "virtual file open",
        )?;
        self.open_file_with_deadline(path, source, &deadline)
    }

    pub(in crate::vfs) fn open_file_with_deadline(
        &mut self,
        path: &Path,
        source: Option<&str>,
        deadline: &dyn DeadlineCheck,
    ) -> Result<VirtualFileSnapshot> {
        deadline.check("virtual path validation")?;
        let (path, normalized) = normalized_virtual_path(path)?;
        let previous = self.entries.get(&normalized).cloned();
        let result = self.open_file_inner(&path, &normalized, source, Some(deadline));
        if result.is_err() {
            match previous {
                Some(previous) => {
                    self.entries.insert(normalized, previous);
                }
                None => {
                    self.entries.remove(&normalized);
                }
            }
        }
        result
    }

    fn open_file_inner(
        &mut self,
        path: &Path,
        normalized: &str,
        source: Option<&str>,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<VirtualFileSnapshot> {
        check_optional_deadline(deadline, "virtual source load")?;
        self.ensure_loaded_inner(path, source, deadline)?;
        check_optional_deadline(deadline, "virtual source refresh")?;
        self.refresh_if_clean_inner(normalized, deadline)?;

        let entry = self
            .entries
            .get(normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
        let snapshot = snapshot_from_entry(normalized, entry)?;
        check_optional_deadline(deadline, "virtual file result validation")?;
        Ok(snapshot)
    }

    pub fn read_file(&mut self, path: &Path) -> Result<VirtualFileSnapshot> {
        self.open_file(path, None)
    }

    pub fn read_file_with_timeout(
        &mut self,
        path: &Path,
        timeout_ms: Option<u64>,
    ) -> Result<VirtualFileSnapshot> {
        let Some(timeout_ms) = timeout_ms else {
            return self.read_file(path);
        };
        let deadline = CooperativeDeadline::new(
            Some(timeout_ms),
            MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS,
            "virtual file read",
        )?;
        self.read_file_with_deadline(path, &deadline)
    }

    pub(in crate::vfs) fn read_file_with_deadline(
        &mut self,
        path: &Path,
        deadline: &dyn DeadlineCheck,
    ) -> Result<VirtualFileSnapshot> {
        self.open_file_with_deadline(path, None, deadline)
    }

    pub fn commit_file(&mut self, path: &Path) -> Result<VirtualFileSnapshot> {
        self.commit_file_with_timeout(path, None)
    }

    pub fn commit_file_with_timeout(
        &mut self,
        path: &Path,
        timeout_ms: Option<u64>,
    ) -> Result<VirtualFileSnapshot> {
        let deadline = CooperativeDeadline::new(
            timeout_ms,
            MAX_VIRTUAL_FILE_COMMIT_TIMEOUT_MS,
            "virtual file commit",
        )?;
        self.commit_file_with_deadline(path, &deadline)
    }

    pub(in crate::vfs) fn commit_file_with_deadline(
        &mut self,
        path: &Path,
        deadline: &dyn DeadlineCheck,
    ) -> Result<VirtualFileSnapshot> {
        deadline.check("virtual path validation")?;
        let (path, normalized) = normalized_virtual_path(path)?;
        deadline.check("virtual source load")?;
        self.ensure_loaded(&path, None)?;
        deadline.check("virtual source refresh")?;
        let source_changed = self.refresh_if_clean(&normalized)?;
        deadline.check("commit persistence")?;
        self.commit_loaded_file(&normalized, source_changed)
    }

    pub(in crate::vfs) fn commit_loaded_file(
        &mut self,
        normalized: &str,
        mut source_changed: bool,
    ) -> Result<VirtualFileSnapshot> {
        let committed_path = {
            let entry = self
                .entries
                .get_mut(normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;

            if entry.dirty {
                write_source_atomic(&entry.path, &entry.source)
                    .with_context(|| format!("failed to write {}", entry.path.display()))?;
                entry.disk_source = entry.source.clone();
                entry.dirty = false;
                source_changed = true;
            }

            entry.path.clone()
        };

        let index_sync_pending = self
            .entries
            .get(normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
            .index_sync_pending;
        if source_changed || index_sync_pending {
            self.entries
                .get_mut(normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
                .index_sync_pending = true;
            self.sync_registered_indexes(&committed_path)?;
            self.entries
                .get_mut(normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
                .index_sync_pending = false;
        }

        let entry = self
            .entries
            .get(normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
        snapshot_from_entry(normalized, entry)
    }

    pub fn discard_file(&mut self, path: &Path) -> Result<VirtualFileSnapshot> {
        self.discard_file_with_timeout(path, None)
    }

    pub fn discard_file_with_timeout(
        &mut self,
        path: &Path,
        timeout_ms: Option<u64>,
    ) -> Result<VirtualFileSnapshot> {
        let deadline = CooperativeDeadline::new(
            timeout_ms,
            MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS,
            "virtual file discard",
        )?;
        self.discard_file_with_deadline(path, &deadline)
    }

    pub(in crate::vfs) fn discard_file_with_deadline(
        &mut self,
        path: &Path,
        deadline: &dyn DeadlineCheck,
    ) -> Result<VirtualFileSnapshot> {
        deadline.check("virtual path validation")?;
        let (path, normalized) = normalized_virtual_path(path)?;
        let previous = self.entries.get(&normalized).cloned();
        let result = self.discard_file_inner(&path, &normalized, deadline);
        if result.is_err() {
            match previous {
                Some(previous) => {
                    self.entries.insert(normalized, previous);
                }
                None => {
                    self.entries.remove(&normalized);
                }
            }
        }
        result
    }

    fn discard_file_inner(
        &mut self,
        path: &Path,
        normalized: &str,
        deadline: &dyn DeadlineCheck,
    ) -> Result<VirtualFileSnapshot> {
        deadline.check("virtual source load")?;
        self.ensure_loaded(path, None)?;
        deadline.check("disk source read")?;

        let current = self
            .entries
            .get(normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
            .clone();
        let disk_source = read_virtual_disk_source(&current.path)?;
        deadline.check("disk source comparison")?;
        if current.source == disk_source && current.disk_source == disk_source {
            let snapshot = snapshot_from_entry(normalized, &current)?;
            deadline.check("discard result validation")?;
            return Ok(snapshot);
        }

        deadline.check("disk source parse")?;
        let document = parse_document_with_timeout(
            &current.path,
            &disk_source,
            DeadlineCheck::remaining_timeout_micros(deadline, "disk source parse")?.unwrap_or(0),
        )?;
        let mut updated = current;
        updated.language_id = document.language_id;
        updated.disk_source = disk_source.clone();
        updated.source = disk_source;
        updated.tree = document.tree;
        updated.version += 1;
        updated.dirty = false;
        let snapshot = snapshot_from_entry(normalized, &updated)?;
        deadline.check("virtual source replacement")?;
        self.entries.insert(normalized.to_string(), updated);
        Ok(snapshot)
    }

    pub fn close_file(&mut self, path: &Path, persist: bool) -> Result<VirtualFileSnapshot> {
        self.close_file_with_timeout(path, persist, None)
    }

    pub fn close_file_with_timeout(
        &mut self,
        path: &Path,
        persist: bool,
        timeout_ms: Option<u64>,
    ) -> Result<VirtualFileSnapshot> {
        let deadline = CooperativeDeadline::new(
            timeout_ms,
            MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS,
            "virtual file close",
        )?;
        self.close_file_with_deadline(path, persist, &deadline)
    }

    pub(in crate::vfs) fn close_file_with_deadline(
        &mut self,
        path: &Path,
        persist: bool,
        deadline: &dyn DeadlineCheck,
    ) -> Result<VirtualFileSnapshot> {
        deadline.check("virtual close dispatch")?;
        let snapshot = if persist {
            self.commit_file_with_deadline(path, deadline)?
        } else {
            self.discard_file_with_deadline(path, deadline)?
        };
        self.entries.remove(&snapshot.file);
        Ok(snapshot)
    }

    fn sync_registered_indexes(&self, file_path: &Path) -> Result<()> {
        let file_path = normalize_absolute_path(file_path)?;
        for (workspace_root, db_path) in &self.symbol_indexes {
            let workspace_root_path = Path::new(workspace_root);
            if path_is_inside_workspace(workspace_root_path, &file_path)? {
                refresh_symbol_index_for_file(workspace_root_path, db_path, &file_path)?;
            }
        }
        Ok(())
    }
}
