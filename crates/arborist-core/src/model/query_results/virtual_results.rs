use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::super::{
    PatchCommitGateReport, PatchValidationReport, ensure_nonblank, ensure_nonblank_strings,
};

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
        for (index, issue) in self.validation.syntax_errors.iter().enumerate() {
            issue.validate_trace_replay_input(index)?;
        }
        ensure_nonblank_strings(
            &self.validation.unresolved_identifiers,
            "virtual_edit.validation.unresolved_identifiers",
        )?;
        if !self.validation.resolved_identifiers.is_empty() {
            bail!(
                "invalid virtual_edit.validation.resolved_identifiers: buffer edit results must not report resolved identifiers"
            );
        }
        if !self.validation.ambiguous_identifiers.is_empty() {
            bail!(
                "invalid virtual_edit.validation.ambiguous_identifiers: buffer edit results must not report ambiguous identifiers"
            );
        }
        if !self.validation.binding_decisions.is_empty() {
            bail!(
                "invalid virtual_edit.validation.binding_decisions: buffer edit results must not report binding decisions"
            );
        }
        if self.validation.commit_gate != PatchCommitGateReport::default() {
            bail!(
                "invalid virtual_edit.validation.commit_gate: buffer edit results must leave commit_gate at the default not_evaluated state"
            );
        }
        Ok(())
    }
}

impl VirtualFileStatus {
    pub(crate) fn validate_public_output(&self, index: usize) -> Result<()> {
        ensure_nonblank(&self.file, &format!("virtual_statuses[{index}].file"))
    }
}
