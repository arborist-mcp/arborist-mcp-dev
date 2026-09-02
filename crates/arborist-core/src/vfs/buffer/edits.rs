use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use tree_sitter::{InputEdit, ParseOptions};

use super::super::state::{VirtualFileEntry, normalized_virtual_path, validate_edit_range};
use super::super::{MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS, VirtualFileSystem};
use super::check_optional_deadline;
use crate::deadline::{CooperativeDeadline, DeadlineCheck};
use crate::language::{
    offset_for_position, parser_for_language, point_for_offset, validate_source_length,
};
use crate::model::validate_position_edit_batch;
use crate::model::{PatchValidationReport, PositionEdit, VirtualEditResult};
use crate::patching::{MAX_PATCH_REPLACEMENT_BYTES, collect_syntax_errors, splice_source};

impl VirtualFileSystem {
    pub fn apply_edit(
        &mut self,
        path: &Path,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
    ) -> Result<VirtualEditResult> {
        let (path, normalized) = normalized_virtual_path(path)?;
        self.prepare_virtual_edit(&path, &normalized, None)?;

        self.apply_loaded_edit(&path, &normalized, start_byte, old_end_byte, new_text)
    }

    pub fn apply_edit_with_timeout(
        &mut self,
        path: &Path,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
        timeout_ms: Option<u64>,
    ) -> Result<VirtualEditResult> {
        let Some(timeout_ms) = timeout_ms else {
            return self.apply_edit(path, start_byte, old_end_byte, new_text);
        };
        let deadline = CooperativeDeadline::new(
            Some(timeout_ms),
            MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS,
            "virtual buffer edit",
        )?;
        self.apply_edit_with_deadline(path, start_byte, old_end_byte, new_text, &deadline)
    }

    pub(in crate::vfs) fn apply_edit_with_deadline(
        &mut self,
        path: &Path,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
        deadline: &dyn DeadlineCheck,
    ) -> Result<VirtualEditResult> {
        deadline.check("virtual path validation")?;
        let (path, normalized) = normalized_virtual_path(path)?;
        let previous = self.entries.get(&normalized).cloned();
        let result = (|| {
            self.prepare_virtual_edit(&path, &normalized, Some(deadline))?;
            self.apply_loaded_edit_inner(
                &path,
                &normalized,
                start_byte,
                old_end_byte,
                new_text,
                Some(deadline),
            )
        })();
        if result.is_err() {
            self.restore_virtual_edit_entry(normalized, previous);
        }
        result
    }

    pub(in crate::vfs) fn apply_loaded_edit(
        &mut self,
        path: &Path,
        normalized: &str,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
    ) -> Result<VirtualEditResult> {
        self.apply_loaded_edit_inner(path, normalized, start_byte, old_end_byte, new_text, None)
    }

    fn apply_loaded_edit_inner(
        &mut self,
        path: &Path,
        normalized: &str,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<VirtualEditResult> {
        check_optional_deadline(deadline, "virtual edit validation")?;
        if new_text.len() > MAX_PATCH_REPLACEMENT_BYTES {
            return Err(anyhow!(
                "invalid new_text: edit exceeds max bytes ({})",
                MAX_PATCH_REPLACEMENT_BYTES
            ));
        }

        let (updated_source, new_tree, dirty, version, result) = {
            let entry = self
                .entries
                .get(normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;

            validate_edit_range(&entry.source, start_byte, old_end_byte)?;
            let result_len = entry
                .source
                .len()
                .checked_sub(old_end_byte - start_byte)
                .and_then(|length| length.checked_add(new_text.len()))
                .ok_or_else(|| anyhow!("updated source size overflowed"))?;
            validate_source_length(path, result_len)?;
            let new_end_byte = start_byte
                .checked_add(new_text.len())
                .ok_or_else(|| anyhow!("updated edit range overflowed"))?;

            check_optional_deadline(deadline, "virtual source splice")?;
            let updated_source = splice_source(&entry.source, start_byte..old_end_byte, new_text);
            let edit = InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position: point_for_offset(&entry.source, start_byte)?,
                old_end_position: point_for_offset(&entry.source, old_end_byte)?,
                new_end_position: point_for_offset(&updated_source, new_end_byte)?,
            };

            let mut edited_tree = entry.tree.clone();
            edited_tree.edit(&edit);
            let mut parser = parser_for_language(entry.language_id)?;
            check_optional_deadline(deadline, "virtual incremental parse")?;
            let timeout_micros = deadline
                .map(|deadline| {
                    DeadlineCheck::remaining_timeout_micros(deadline, "virtual incremental parse")
                })
                .transpose()?
                .flatten()
                .unwrap_or(0);
            let new_tree = if timeout_micros > 0 {
                let parse_deadline = Instant::now() + Duration::from_micros(timeout_micros);
                let mut progress_callback =
                    |_: &tree_sitter::ParseState| Instant::now() >= parse_deadline;
                let parse_options = ParseOptions::new().progress_callback(&mut progress_callback);
                let mut read_source = |byte_offset: usize, _position: tree_sitter::Point| {
                    updated_source
                        .as_bytes()
                        .get(byte_offset..)
                        .unwrap_or_default()
                };
                parser.parse_with_options(&mut read_source, Some(&edited_tree), Some(parse_options))
            } else {
                parser.parse(&updated_source, Some(&edited_tree))
            }
            .ok_or_else(|| {
                if timeout_micros > 0 {
                    anyhow!(
                        "incremental parse timed out after {} microseconds for {}",
                        timeout_micros,
                        entry.path.display()
                    )
                } else {
                    anyhow!("incremental parse failed for {}", entry.path.display())
                }
            })?;

            check_optional_deadline(deadline, "virtual syntax collection")?;
            let syntax_errors = collect_syntax_errors(new_tree.root_node(), &updated_source);
            let dirty = updated_source != entry.disk_source;
            let version = entry
                .version
                .checked_add(1)
                .ok_or_else(|| anyhow!("virtual file version overflowed"))?;
            let result = VirtualEditResult {
                file: normalized.to_string(),
                source: updated_source.clone(),
                dirty,
                version,
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
            check_optional_deadline(deadline, "virtual edit result validation")?;
            result.validate_public_output()?;
            check_optional_deadline(deadline, "virtual edit commit")?;
            (updated_source, new_tree, dirty, version, result)
        };

        let entry = self
            .entries
            .get_mut(normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
        entry.source = updated_source;
        entry.tree = new_tree;
        entry.version = version;
        entry.dirty = dirty;
        Ok(result)
    }

    pub fn apply_position_edits(
        &mut self,
        path: &Path,
        edits: &[PositionEdit],
    ) -> Result<VirtualEditResult> {
        validate_position_edit_batch(edits, "position edits")?;
        let (path, normalized) = normalized_virtual_path(path)?;
        self.prepare_virtual_edit(&path, &normalized, None)?;
        if edits.is_empty() {
            return self.current_virtual_edit_result(&normalized, None);
        }

        let previous = self
            .entries
            .get(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
            .clone();
        let result = self.apply_loaded_position_edits(&path, &normalized, edits, None);
        if result.is_err() {
            self.entries.insert(normalized, previous);
        }
        result
    }

    pub fn apply_position_edits_with_timeout(
        &mut self,
        path: &Path,
        edits: &[PositionEdit],
        timeout_ms: Option<u64>,
    ) -> Result<VirtualEditResult> {
        let Some(timeout_ms) = timeout_ms else {
            return self.apply_position_edits(path, edits);
        };
        let deadline = CooperativeDeadline::new(
            Some(timeout_ms),
            MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS,
            "virtual position edits",
        )?;
        self.apply_position_edits_with_deadline(path, edits, &deadline)
    }

    pub(in crate::vfs) fn apply_position_edits_with_deadline(
        &mut self,
        path: &Path,
        edits: &[PositionEdit],
        deadline: &dyn DeadlineCheck,
    ) -> Result<VirtualEditResult> {
        deadline.check("position edit validation")?;
        validate_position_edit_batch(edits, "position edits")?;
        deadline.check("virtual path validation")?;
        let (path, normalized) = normalized_virtual_path(path)?;
        let previous = self.entries.get(&normalized).cloned();
        let result = (|| {
            self.prepare_virtual_edit(&path, &normalized, Some(deadline))?;
            if edits.is_empty() {
                self.current_virtual_edit_result(&normalized, Some(deadline))
            } else {
                self.apply_loaded_position_edits(&path, &normalized, edits, Some(deadline))
            }
        })();
        if result.is_err() {
            self.restore_virtual_edit_entry(normalized, previous);
        }
        result
    }

    fn prepare_virtual_edit(
        &mut self,
        path: &Path,
        normalized: &str,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<()> {
        check_optional_deadline(deadline, "virtual source load")?;
        self.ensure_loaded_inner(path, None, deadline)?;
        check_optional_deadline(deadline, "virtual source refresh")?;
        self.refresh_if_clean_inner(normalized, deadline)?;
        Ok(())
    }

    fn current_virtual_edit_result(
        &self,
        normalized: &str,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<VirtualEditResult> {
        let entry = self
            .entries
            .get(normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
        check_optional_deadline(deadline, "virtual syntax collection")?;
        let result = VirtualEditResult {
            file: normalized.to_string(),
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
        check_optional_deadline(deadline, "virtual edit result validation")?;
        Ok(result)
    }

    fn apply_loaded_position_edits(
        &mut self,
        path: &Path,
        normalized: &str,
        edits: &[PositionEdit],
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<VirtualEditResult> {
        let mut last_result = None;
        for (index, edit) in edits.iter().enumerate() {
            let result = (|| -> Result<VirtualEditResult> {
                check_optional_deadline(deadline, "position edit resolution")?;
                let source = self
                    .entries
                    .get(normalized)
                    .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
                    .source
                    .clone();
                let start_byte = offset_for_position(&source, &edit.start)?;
                let old_end_byte = offset_for_position(&source, &edit.end)?;
                self.apply_loaded_edit_inner(
                    path,
                    normalized,
                    start_byte,
                    old_end_byte,
                    &edit.new_text,
                    deadline,
                )
            })()
            .with_context(|| format!("failed to apply position edit at index {index}"));
            last_result = Some(result?);
        }

        last_result.ok_or_else(|| anyhow!("position edits did not produce a result"))
    }

    fn restore_virtual_edit_entry(
        &mut self,
        normalized: String,
        previous: Option<VirtualFileEntry>,
    ) {
        match previous {
            Some(previous) => {
                self.entries.insert(normalized, previous);
            }
            None => {
                self.entries.remove(&normalized);
            }
        }
    }
}
