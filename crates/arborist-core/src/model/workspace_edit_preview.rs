use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::{PatchValidationReport, PositionEdit, ensure_nonblank};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspacePositionEdits {
    pub file_path: String,
    pub source: Option<String>,
    pub edits: Vec<PositionEdit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceEditPreviewFile {
    pub file: String,
    pub source: String,
    pub unified_diff: String,
    pub changed: bool,
    pub validation: PatchValidationReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceEditPreviewResult {
    pub changed: bool,
    pub files: Vec<WorkspaceEditPreviewFile>,
}

impl WorkspacePositionEdits {
    pub(crate) fn validate_input(&self, index: usize) -> Result<()> {
        ensure_nonblank(
            &self.file_path,
            &format!("workspace_edits[{index}].file_path"),
        )
    }
}

impl WorkspaceEditPreviewResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        if self.files.is_empty() {
            bail!("invalid workspace_edit_preview.files: expected at least one file");
        }

        let mut changed = false;
        for (index, file) in self.files.iter().enumerate() {
            ensure_nonblank(
                &file.file,
                &format!("workspace_edit_preview.files[{index}].file"),
            )?;
            if file.changed == file.unified_diff.is_empty() {
                bail!(
                    "invalid workspace_edit_preview.files[{index}].changed: expected changed to match unified_diff presence"
                );
            }
            changed |= file.changed;
        }
        if self.changed != changed {
            bail!("invalid workspace_edit_preview.changed: expected changed to match file changes");
        }
        Ok(())
    }
}
