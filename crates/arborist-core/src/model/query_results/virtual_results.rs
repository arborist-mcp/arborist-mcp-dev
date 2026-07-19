use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::super::{PatchValidationReport, ensure_nonblank};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VirtualFileSnapshot {
    pub file: String,
    pub source: String,
    pub disk_source: String,
    pub dirty: bool,
    pub version: u64,
    pub syntax_error_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VirtualEditResult {
    pub file: String,
    pub source: String,
    pub dirty: bool,
    pub version: u64,
    pub incremental_parse: bool,
    pub validation: PatchValidationReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VirtualFileStatus {
    pub file: String,
    pub dirty: bool,
    pub version: u64,
    pub syntax_error_count: usize,
}

impl VirtualFileSnapshot {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.file, "virtual_snapshot.file")?;
        if self.dirty != (self.source != self.disk_source) {
            bail!(
                "invalid virtual_snapshot.dirty: expected dirty to match whether source differs from disk_source"
            );
        }
        Ok(())
    }
}

impl VirtualEditResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.file, "virtual_edit.file")?;
        self.validation
            .validate_syntax_only_output("virtual_edit.validation")?;
        Ok(())
    }
}

impl VirtualFileStatus {
    pub(crate) fn validate_public_output(&self, index: usize) -> Result<()> {
        ensure_nonblank(&self.file, &format!("virtual_statuses[{index}].file"))
    }
}
