use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use super::super::VirtualFileSystem;
use super::super::state::{VirtualFileEntry, normalized_virtual_path, read_virtual_disk_source};
use super::check_optional_deadline;
use crate::deadline::DeadlineCheck;
use crate::language::{
    normalize_absolute_path, normalize_path, parse_document, path_is_inside_workspace,
};
use crate::workspace_scan::should_skip_index_path;

impl VirtualFileSystem {
    pub(in crate::vfs) fn ensure_loaded(
        &mut self,
        path: &Path,
        source_override: Option<&str>,
    ) -> Result<()> {
        self.ensure_loaded_inner(path, source_override, None)
    }

    pub(in crate::vfs::buffer) fn ensure_loaded_inner(
        &mut self,
        path: &Path,
        source_override: Option<&str>,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<()> {
        let (path, normalized) = normalized_virtual_path(path)?;
        match self.entries.get_mut(&normalized) {
            Some(entry) => {
                if let Some(source_override) = source_override {
                    check_optional_deadline(deadline, "virtual disk source read")?;
                    let disk_source = read_virtual_disk_source(&path)?;
                    check_optional_deadline(deadline, "virtual source parse")?;
                    let document = parse_document(&path, source_override)?;
                    check_optional_deadline(deadline, "virtual source replacement")?;
                    entry.path = path;
                    entry.language_id = document.language_id;
                    entry.disk_source = disk_source;
                    entry.source = source_override.to_string();
                    entry.tree = document.tree;
                    entry.version += 1;
                    entry.dirty = entry.source != entry.disk_source;
                }
            }
            None => {
                check_optional_deadline(deadline, "virtual disk source read")?;
                let disk_source = read_virtual_disk_source(&path)?;
                let initial_source = source_override.unwrap_or(&disk_source).to_string();
                check_optional_deadline(deadline, "virtual source parse")?;
                let document = parse_document(&path, &initial_source)?;
                let dirty = initial_source != disk_source;
                check_optional_deadline(deadline, "virtual source insertion")?;
                self.entries.insert(
                    normalized,
                    VirtualFileEntry {
                        path,
                        language_id: document.language_id,
                        disk_source,
                        source: initial_source,
                        tree: document.tree,
                        version: 0,
                        dirty,
                        index_sync_pending: false,
                    },
                );
            }
        }
        Ok(())
    }

    pub(in crate::vfs) fn refresh_if_clean(&mut self, normalized: &str) -> Result<bool> {
        self.refresh_if_clean_inner(normalized, None)
    }

    pub(in crate::vfs) fn refresh_if_clean_with_deadline(
        &mut self,
        normalized: &str,
        deadline: &dyn DeadlineCheck,
    ) -> Result<bool> {
        self.refresh_if_clean_inner(normalized, Some(deadline))
    }

    pub(in crate::vfs::buffer) fn refresh_if_clean_inner(
        &mut self,
        normalized: &str,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<bool> {
        let Some(entry) = self.entries.get_mut(normalized) else {
            return Ok(false);
        };
        if entry.dirty {
            return Ok(false);
        }

        check_optional_deadline(deadline, "virtual disk source read")?;
        let disk_source = read_virtual_disk_source(&entry.path)?;
        check_optional_deadline(deadline, "virtual disk source comparison")?;
        if disk_source == entry.disk_source {
            return Ok(false);
        }

        check_optional_deadline(deadline, "virtual source parse")?;
        let document = parse_document(&entry.path, &disk_source)?;
        check_optional_deadline(deadline, "virtual source replacement")?;
        entry.language_id = document.language_id;
        entry.disk_source = disk_source.clone();
        entry.source = disk_source;
        entry.tree = document.tree;
        entry.version += 1;
        Ok(true)
    }

    pub(in crate::vfs) fn virtual_overrides_for_workspace(
        &mut self,
        workspace_root: &Path,
    ) -> Result<BTreeMap<String, String>> {
        self.virtual_overrides_for_workspace_inner(workspace_root, None)
    }

    pub(in crate::vfs) fn virtual_overrides_for_workspace_with_deadline(
        &mut self,
        workspace_root: &Path,
        deadline: &dyn DeadlineCheck,
    ) -> Result<BTreeMap<String, String>> {
        self.virtual_overrides_for_workspace_inner(workspace_root, Some(deadline))
    }

    fn virtual_overrides_for_workspace_inner(
        &mut self,
        workspace_root: &Path,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<BTreeMap<String, String>> {
        let loaded_files: Vec<_> = self.entries.keys().cloned().collect();
        for normalized in &loaded_files {
            match deadline {
                Some(deadline) => self.refresh_if_clean_with_deadline(normalized, deadline)?,
                None => self.refresh_if_clean(normalized)?,
            };
        }

        let mut overrides = BTreeMap::new();
        for entry in self.entries.values() {
            check_optional_deadline(deadline, "virtual override collection")?;
            if !entry.dirty {
                continue;
            }

            let absolute_path = normalize_absolute_path(&entry.path)?;
            if path_is_inside_workspace(workspace_root, &absolute_path)?
                && !should_skip_index_path(workspace_root, &absolute_path)
                && crate::language::detect_language(&absolute_path).is_ok()
            {
                overrides.insert(normalize_path(&absolute_path), entry.source.clone());
            }
        }

        check_optional_deadline(deadline, "virtual override collection result")?;
        Ok(overrides)
    }
}
