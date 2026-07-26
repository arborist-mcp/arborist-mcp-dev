use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use tree_sitter::InputEdit;

use super::VirtualFileSystem;
use super::state::{
    VirtualFileEntry, normalized_virtual_path, read_virtual_disk_source, snapshot_from_entry,
    validate_edit_range,
};
use crate::MAX_POSITION_EDITS;
use crate::language::{
    normalize_absolute_path, normalize_path, offset_for_position, parse_document,
    parser_for_language, path_is_inside_workspace, point_for_offset, write_source_atomic,
};
use crate::model::{PatchValidationReport, PositionEdit, VirtualEditResult, VirtualFileSnapshot};
use crate::patching::{MAX_PATCH_REPLACEMENT_BYTES, collect_syntax_errors, splice_source};
use crate::symbols::refresh_symbol_index_for_file;
use crate::workspace_scan::should_skip_index_path;

impl VirtualFileSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_file(&mut self, path: &Path, source: Option<&str>) -> Result<VirtualFileSnapshot> {
        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, source)?;
        self.refresh_if_clean(&normalized)?;

        let entry = self
            .entries
            .get(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
        snapshot_from_entry(&normalized, entry)
    }

    pub fn read_file(&mut self, path: &Path) -> Result<VirtualFileSnapshot> {
        self.open_file(path, None)
    }

    pub fn apply_edit(
        &mut self,
        path: &Path,
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
        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let entry = self
            .entries
            .get_mut(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;

        validate_edit_range(&entry.source, start_byte, old_end_byte)?;
        let updated_source = splice_source(&entry.source, start_byte..old_end_byte, new_text);

        let edit = InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte: start_byte + new_text.len(),
            start_position: point_for_offset(&entry.source, start_byte)?,
            old_end_position: point_for_offset(&entry.source, old_end_byte)?,
            new_end_position: point_for_offset(&updated_source, start_byte + new_text.len())?,
        };

        entry.tree.edit(&edit);
        let mut parser = parser_for_language(entry.language_id)?;
        let new_tree = parser
            .parse(&updated_source, Some(&entry.tree))
            .ok_or_else(|| anyhow!("incremental parse failed for {}", entry.path.display()))?;

        let syntax_errors = collect_syntax_errors(new_tree.root_node(), &updated_source);
        entry.source = updated_source.clone();
        entry.tree = new_tree;
        entry.version += 1;
        entry.dirty = entry.source != entry.disk_source;

        let result = VirtualEditResult {
            file: normalized,
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
        if edits.len() > MAX_POSITION_EDITS {
            return Err(anyhow!(
                "invalid position edits: expected at most {} entries",
                MAX_POSITION_EDITS
            ));
        }
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
        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;
        let mut source_changed = self.refresh_if_clean(&normalized)?;

        let committed_path = {
            let entry = self
                .entries
                .get_mut(&normalized)
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

        if source_changed {
            self.sync_registered_indexes(&committed_path)?;
        }

        let entry = self
            .entries
            .get(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
        snapshot_from_entry(&normalized, entry)
    }

    pub fn discard_file(&mut self, path: &Path) -> Result<VirtualFileSnapshot> {
        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;

        let entry = self
            .entries
            .get_mut(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;

        let disk_source = read_virtual_disk_source(&entry.path)?;
        if entry.source == disk_source && entry.disk_source == disk_source {
            return snapshot_from_entry(&normalized, entry);
        }
        let document = parse_document(&entry.path, &disk_source)?;
        entry.language_id = document.language_id;
        entry.disk_source = disk_source.clone();
        entry.source = disk_source;
        entry.tree = document.tree;
        entry.version += 1;
        entry.dirty = false;

        snapshot_from_entry(&normalized, entry)
    }

    pub fn close_file(&mut self, path: &Path, persist: bool) -> Result<VirtualFileSnapshot> {
        let snapshot = if persist {
            self.commit_file(path)?
        } else {
            self.discard_file(path)?
        };
        self.entries.remove(&snapshot.file);
        Ok(snapshot)
    }

    pub(super) fn ensure_loaded(
        &mut self,
        path: &Path,
        source_override: Option<&str>,
    ) -> Result<()> {
        let (path, normalized) = normalized_virtual_path(path)?;
        match self.entries.get_mut(&normalized) {
            Some(entry) => {
                if let Some(source_override) = source_override {
                    let disk_source = read_virtual_disk_source(&path)?;
                    let document = parse_document(&path, source_override)?;
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
                let disk_source = read_virtual_disk_source(&path)?;
                let initial_source = source_override.unwrap_or(&disk_source).to_string();
                let document = parse_document(&path, &initial_source)?;
                let dirty = initial_source != disk_source;
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
                    },
                );
            }
        }
        Ok(())
    }

    pub(super) fn refresh_if_clean(&mut self, normalized: &str) -> Result<bool> {
        let Some(entry) = self.entries.get_mut(normalized) else {
            return Ok(false);
        };
        if entry.dirty {
            return Ok(false);
        }

        let disk_source = read_virtual_disk_source(&entry.path)?;
        if disk_source == entry.disk_source {
            return Ok(false);
        }

        let document = parse_document(&entry.path, &disk_source)?;
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
