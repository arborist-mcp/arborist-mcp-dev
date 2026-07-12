use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::primitives::Position;
use super::{SymbolSummary, ensure_nonblank};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidationIssue {
    pub kind: String,
    pub message: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_point: Position,
    pub end_point: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidationBinding {
    pub name: String,
    pub symbol: SymbolSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidationAmbiguity {
    pub name: String,
    pub candidates: Vec<SymbolSummary>,
    pub reason: String,
    pub disambiguation_context: DisambiguationContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidationBindingDecision {
    pub name: String,
    pub status: String,
    pub reason: String,
    pub selected_symbol_id: Option<String>,
    pub candidates: Vec<SymbolSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchEvidenceInvariantReport {
    pub name: String,
    pub status: String,
    pub reason: String,
    pub selected_evidence_key: Option<String>,
    pub candidate_evidence_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchCommitGateReport {
    pub status: String,
    pub allowed: bool,
    pub reason: String,
    pub bypass_reason: Option<String>,
    pub blocking_decisions: Vec<ValidationBindingDecision>,
    pub evidence_invariants: Vec<PatchEvidenceInvariantReport>,
    pub syntax_error_count: usize,
}

impl Default for PatchCommitGateReport {
    fn default() -> Self {
        Self {
            status: "not_evaluated".to_string(),
            allowed: false,
            reason: "patch commit gate has not been evaluated".to_string(),
            bypass_reason: None,
            blocking_decisions: Vec::new(),
            evidence_invariants: Vec::new(),
            syntax_error_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct DisambiguationContext {
    pub active_include_family: Option<String>,
    pub preferred_family: Option<String>,
    pub visible_include_families: Vec<String>,
    pub candidate_include_families: Vec<String>,
    pub candidate_symbol_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct PatchValidationReport {
    pub syntax_errors: Vec<ValidationIssue>,
    pub unresolved_identifiers: Vec<String>,
    pub resolved_identifiers: Vec<ValidationBinding>,
    pub ambiguous_identifiers: Vec<ValidationAmbiguity>,
    pub binding_decisions: Vec<ValidationBindingDecision>,
    pub commit_gate: PatchCommitGateReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchAstNodeResult {
    pub file: String,
    pub target_path: String,
    pub resolved_path: String,
    pub resolved_symbol_id: String,
    pub applied: bool,
    pub bypass_applied: bool,
    pub updated_source: String,
    pub validation: PatchValidationReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchPreviewResult {
    pub patch: PatchAstNodeResult,
    pub unified_diff: String,
    pub changed: bool,
}

impl PatchAstNodeResult {
    pub fn validate_trace_replay_input(&self) -> Result<()> {
        ensure_nonblank(&self.file, "patch.file")?;
        ensure_nonblank(&self.target_path, "patch.target_path")?;
        ensure_nonblank(&self.resolved_path, "patch.resolved_path")?;
        ensure_nonblank(&self.resolved_symbol_id, "patch.resolved_symbol_id")?;
        self.validation.validate_trace_replay_input()?;
        self.validation.commit_gate.validate_trace_replay_input(
            self.applied,
            self.bypass_applied,
            self.validation.syntax_errors.len(),
        )
    }

    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.updated_source, "patch.updated_source")?;
        self.validate_trace_replay_input()
    }
}

impl PatchPreviewResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        if self.changed == self.unified_diff.is_empty() {
            bail!("invalid patch_preview.changed: expected changed to match unified_diff presence");
        }
        Ok(())
    }
}
