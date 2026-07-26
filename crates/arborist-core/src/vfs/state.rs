use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use tree_sitter::Tree;

use crate::language::{normalize_absolute_path, normalize_path, read_source};
use crate::model::{LanguageId, VirtualFileSnapshot};
use crate::patching::collect_syntax_errors;

#[derive(Clone)]
pub(super) struct VirtualFileEntry {
    pub(super) path: PathBuf,
    pub(super) language_id: LanguageId,
    pub(super) disk_source: String,
    pub(super) source: String,
    pub(super) tree: Tree,
    pub(super) version: u64,
    pub(super) dirty: bool,
    pub(super) index_sync_pending: bool,
}

pub(super) fn normalized_virtual_path(path: &Path) -> Result<(PathBuf, String)> {
    let absolute_path = normalize_absolute_path(path)?;
    let normalized = normalize_path(&absolute_path);
    Ok((absolute_path, normalized))
}

pub(super) fn read_virtual_disk_source(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    read_source(path)
}

pub(super) fn snapshot_from_entry(
    file: &str,
    entry: &VirtualFileEntry,
) -> Result<VirtualFileSnapshot> {
    let snapshot = VirtualFileSnapshot {
        file: file.to_string(),
        source: entry.source.clone(),
        disk_source: entry.disk_source.clone(),
        dirty: entry.dirty,
        version: entry.version,
        syntax_error_count: collect_syntax_errors(entry.tree.root_node(), &entry.source).len(),
    };
    snapshot.validate_public_output()?;
    Ok(snapshot)
}

pub(super) fn validate_edit_range(
    source: &str,
    start_byte: usize,
    old_end_byte: usize,
) -> Result<()> {
    if start_byte > old_end_byte {
        bail!(
            "edit start_byte {} is after old_end_byte {}",
            start_byte,
            old_end_byte
        );
    }
    if old_end_byte > source.len() {
        bail!(
            "edit old_end_byte {} is out of bounds for source of length {}",
            old_end_byte,
            source.len()
        );
    }
    if !source.is_char_boundary(start_byte) || !source.is_char_boundary(old_end_byte) {
        bail!("edit range must align to UTF-8 character boundaries");
    }
    Ok(())
}
