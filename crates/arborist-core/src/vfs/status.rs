use anyhow::Result;

use super::VirtualFileSystem;
use crate::model::VirtualFileStatus;
use crate::patching::collect_syntax_errors;

impl VirtualFileSystem {
    pub fn virtual_file_statuses(&mut self, dirty_only: bool) -> Result<Vec<VirtualFileStatus>> {
        let loaded_files: Vec<_> = self.entries.keys().cloned().collect();
        for normalized in &loaded_files {
            self.refresh_if_clean(normalized)?;
        }

        let mut statuses: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(file, entry)| {
                if dirty_only && !entry.dirty {
                    return None;
                }

                Some(VirtualFileStatus {
                    file: file.clone(),
                    dirty: entry.dirty,
                    version: entry.version,
                    syntax_error_count: collect_syntax_errors(
                        entry.tree.root_node(),
                        &entry.source,
                    )
                    .len(),
                })
            })
            .collect();
        statuses.sort_by(|left, right| left.file.cmp(&right.file));
        for (index, status) in statuses.iter().enumerate() {
            status.validate_public_output(index)?;
        }
        Ok(statuses)
    }
}
