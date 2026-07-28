use anyhow::{Result, anyhow};

use super::buffer::check_optional_deadline;
use super::{MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS, VirtualFileSystem};
use crate::deadline::{CooperativeDeadline, DeadlineCheck};
use crate::model::VirtualFileStatus;
use crate::patching::collect_syntax_errors;

impl VirtualFileSystem {
    pub fn virtual_file_statuses(&mut self, dirty_only: bool) -> Result<Vec<VirtualFileStatus>> {
        self.virtual_file_statuses_inner(dirty_only, None)
    }

    pub fn virtual_file_statuses_with_timeout(
        &mut self,
        dirty_only: bool,
        timeout_ms: Option<u64>,
    ) -> Result<Vec<VirtualFileStatus>> {
        let Some(timeout_ms) = timeout_ms else {
            return self.virtual_file_statuses(dirty_only);
        };
        let deadline = CooperativeDeadline::new(
            Some(timeout_ms),
            MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS,
            "virtual file listing",
        )?;
        self.virtual_file_statuses_with_deadline(dirty_only, &deadline)
    }

    pub(super) fn virtual_file_statuses_with_deadline(
        &mut self,
        dirty_only: bool,
        deadline: &dyn DeadlineCheck,
    ) -> Result<Vec<VirtualFileStatus>> {
        deadline.check("virtual state snapshot")?;
        let previous = self.entries.clone();
        let result = self.virtual_file_statuses_inner(dirty_only, Some(deadline));
        if result.is_err() {
            self.entries = previous;
        }
        result
    }

    fn virtual_file_statuses_inner(
        &mut self,
        dirty_only: bool,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Vec<VirtualFileStatus>> {
        check_optional_deadline(deadline, "virtual file enumeration")?;
        let mut loaded_files: Vec<_> = self.entries.keys().cloned().collect();
        loaded_files.sort();
        for normalized in &loaded_files {
            check_optional_deadline(deadline, "virtual source refresh")?;
            if let Some(deadline) = deadline {
                self.refresh_if_clean_with_deadline(normalized, deadline)?;
            } else {
                self.refresh_if_clean(normalized)?;
            }
        }

        let mut statuses = Vec::with_capacity(self.entries.len());
        for file in loaded_files {
            check_optional_deadline(deadline, "virtual status collection")?;
            let entry = self
                .entries
                .get(&file)
                .ok_or_else(|| anyhow!("virtual file not loaded: {file}"))?;
            if dirty_only && !entry.dirty {
                continue;
            }

            statuses.push(VirtualFileStatus {
                file,
                dirty: entry.dirty,
                version: entry.version,
                syntax_error_count: collect_syntax_errors(entry.tree.root_node(), &entry.source)
                    .len(),
            });
            check_optional_deadline(deadline, "virtual status collection result")?;
        }

        for (index, status) in statuses.iter().enumerate() {
            check_optional_deadline(deadline, "virtual status validation")?;
            status.validate_public_output(index)?;
        }
        Ok(statuses)
    }
}
