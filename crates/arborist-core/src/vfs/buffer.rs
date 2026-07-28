use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use tree_sitter::InputEdit;

use super::state::{
    VirtualFileEntry, normalized_virtual_path, read_virtual_disk_source, snapshot_from_entry,
    validate_edit_range,
};
use super::{
    MAX_VIRTUAL_FILE_COMMIT_TIMEOUT_MS, MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS, VirtualFileSystem,
};
use crate::deadline::{CooperativeDeadline, DeadlineCheck};
use crate::language::{
    normalize_absolute_path, normalize_path, offset_for_position, parse_document,
    parser_for_language, path_is_inside_workspace, point_for_offset, validate_source_length,
    write_source_atomic,
};
use crate::model::validate_position_edit_batch;
use crate::model::{PatchValidationReport, PositionEdit, VirtualEditResult, VirtualFileSnapshot};
use crate::patching::{MAX_PATCH_REPLACEMENT_BYTES, collect_syntax_errors, splice_source};
use crate::symbols::refresh_symbol_index_for_file;
use crate::workspace_scan::should_skip_index_path;

pub(super) fn check_optional_deadline(
    deadline: Option<&dyn DeadlineCheck>,
    phase: &str,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(())
}

impl VirtualFileSystem {
    pub fn new() -> Self {
        Self::default()
    }

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

    pub(super) fn open_file_with_deadline(
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

    pub(super) fn read_file_with_deadline(
        &mut self,
        path: &Path,
        deadline: &dyn DeadlineCheck,
    ) -> Result<VirtualFileSnapshot> {
        self.open_file_with_deadline(path, None, deadline)
    }

    pub fn apply_edit(
        &mut self,
        path: &Path,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
    ) -> Result<VirtualEditResult> {
        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        self.apply_loaded_edit(&path, &normalized, start_byte, old_end_byte, new_text)
    }

    pub(super) fn apply_loaded_edit(
        &mut self,
        path: &Path,
        normalized: &str,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
    ) -> Result<VirtualEditResult> {
        if new_text.len() > MAX_PATCH_REPLACEMENT_BYTES {
            return Err(anyhow!(
                "invalid new_text: edit exceeds max bytes ({})",
                MAX_PATCH_REPLACEMENT_BYTES
            ));
        }

        let entry = self
            .entries
            .get_mut(normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;

        validate_edit_range(&entry.source, start_byte, old_end_byte)?;
        let result_len = entry
            .source
            .len()
            .checked_sub(old_end_byte - start_byte)
            .and_then(|length| length.checked_add(new_text.len()))
            .ok_or_else(|| anyhow!("updated source size overflowed"))?;
        validate_source_length(path, result_len)?;
        let updated_source = splice_source(&entry.source, start_byte..old_end_byte, new_text);

        let edit = InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte: start_byte + new_text.len(),
            start_position: point_for_offset(&entry.source, start_byte)?,
            old_end_position: point_for_offset(&entry.source, old_end_byte)?,
            new_end_position: point_for_offset(&updated_source, start_byte + new_text.len())?,
        };

        let mut edited_tree = entry.tree.clone();
        edited_tree.edit(&edit);
        let mut parser = parser_for_language(entry.language_id)?;
        let new_tree = parser
            .parse(&updated_source, Some(&edited_tree))
            .ok_or_else(|| anyhow!("incremental parse failed for {}", entry.path.display()))?;

        let syntax_errors = collect_syntax_errors(new_tree.root_node(), &updated_source);
        entry.source = updated_source.clone();
        entry.tree = new_tree;
        entry.version += 1;
        entry.dirty = entry.source != entry.disk_source;

        let result = VirtualEditResult {
            file: normalized.to_string(),
            source: updated_source,
            dirty: entry.dirty,
            version: entry.version,
            incremental_parse: true,
            validation: PatchValidationReport {
                syntax_errors,
                unresolved_identifiers: Vec::new(),
                resolved_identifiers: Vec::new(),
                ambiguous_identifiers: Vec::new(),
                binding_decisions: Vec::new(),
                commit_gate: Default::default(),
            },
        };
        result.validate_public_output()?;
        Ok(result)
    }

    pub fn apply_position_edits(
        &mut self,
        path: &Path,
        edits: &[PositionEdit],
    ) -> Result<VirtualEditResult> {
        validate_position_edit_batch(edits, "position edits")?;
        if edits.is_empty() {
            let (path, normalized) = normalized_virtual_path(path)?;
            self.ensure_loaded(&path, None)?;
            self.refresh_if_clean(&normalized)?;

            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            let result = VirtualEditResult {
                file: normalized,
                source: entry.source.clone(),
                dirty: entry.dirty,
                version: entry.version,
                incremental_parse: true,
                validation: PatchValidationReport {
                    syntax_errors: collect_syntax_errors(entry.tree.root_node(), &entry.source),
                    unresolved_identifiers: Vec::new(),
                    resolved_identifiers: Vec::new(),
                    ambiguous_identifiers: Vec::new(),
                    binding_decisions: Vec::new(),
                    commit_gate: Default::default(),
                },
            };
            result.validate_public_output()?;
            return Ok(result);
        }

        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let previous = self
            .entries
            .get(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
            .clone();

        let mut last_result = None;
        for (index, edit) in edits.iter().enumerate() {
            let result = (|| -> Result<VirtualEditResult> {
                let source = self
                    .entries
                    .get(&normalized)
                    .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
                    .source
                    .clone();
                let start_byte = offset_for_position(&source, &edit.start)?;
                let old_end_byte = offset_for_position(&source, &edit.end)?;
                self.apply_edit(&path, start_byte, old_end_byte, &edit.new_text)
            })()
            .with_context(|| format!("failed to apply position edit at index {index}"));

            match result {
                Ok(result) => last_result = Some(result),
                Err(error) => {
                    self.entries.insert(normalized, previous);
                    return Err(error);
                }
            }
        }

        last_result.ok_or_else(|| anyhow!("position edits did not produce a result"))
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

    pub(super) fn commit_file_with_deadline(
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

    pub(super) fn commit_loaded_file(
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

    pub(super) fn discard_file_with_deadline(
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
        let document = parse_document(&current.path, &disk_source)?;
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

    pub(super) fn close_file_with_deadline(
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

    pub(super) fn ensure_loaded(
        &mut self,
        path: &Path,
        source_override: Option<&str>,
    ) -> Result<()> {
        self.ensure_loaded_inner(path, source_override, None)
    }

    fn ensure_loaded_inner(
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

    pub(super) fn refresh_if_clean(&mut self, normalized: &str) -> Result<bool> {
        self.refresh_if_clean_inner(normalized, None)
    }

    pub(super) fn refresh_if_clean_with_deadline(
        &mut self,
        normalized: &str,
        deadline: &dyn DeadlineCheck,
    ) -> Result<bool> {
        self.refresh_if_clean_inner(normalized, Some(deadline))
    }

    fn refresh_if_clean_inner(
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

    pub(super) fn virtual_overrides_for_workspace(
        &mut self,
        workspace_root: &Path,
    ) -> Result<BTreeMap<String, String>> {
        let loaded_files: Vec<_> = self.entries.keys().cloned().collect();
        for normalized in &loaded_files {
            self.refresh_if_clean(normalized)?;
        }

        let mut overrides = BTreeMap::new();
        for entry in self.entries.values() {
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

        Ok(overrides)
    }
}
