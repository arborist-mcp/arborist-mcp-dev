use std::collections::BTreeSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub const SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION: &str = "1";

mod query_results;
mod trace_patch_results;
pub use query_results::{
    RegisteredSymbolIndex, SymbolContextResult, SymbolIndexHealth, SymbolIndexStats,
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult,
    SymbolReadResult, SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
    SymbolSearchMatchDetail, SymbolSearchNeighborhoodContextResult, SymbolSearchResult,
    VirtualEditResult, VirtualFileSnapshot, VirtualFileStatus,
};
pub use trace_patch_results::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    PatchTraceValidationResult, TraceBackedPatchResult, TraceEvidenceKeys,
    TracePatchEvidenceReplayItem, TracePatchEvidenceReplayResult, TraceSymbolGraphResult,
    TraceSymbolNeighborhoodEdge, TraceSymbolNeighborhoodNode, TraceSymbolNeighborhoodResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
    Python,
    C,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub row: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PositionEdit {
    pub start: Position,
    pub end: Position,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SemanticSkeleton {
    pub file: String,
    pub skeleton: String,
    pub available_paths: Vec<String>,
    pub available_symbols: Vec<SemanticSkeletonSymbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct SemanticSkeletonSymbol {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub node_kind: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QueryCaptureResult {
    pub capture_name: String,
    pub node_kind: String,
    pub text: String,
    pub owner_symbol_id: Option<String>,
    pub owner_semantic_path: Option<String>,
    pub owner_scope_path: Option<String>,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_point: Position,
    pub end_point: Position,
}

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

impl SemanticSkeleton {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.file, "skeleton.file")?;
        ensure_nonblank_strings(&self.available_paths, "skeleton.available_paths")?;
        if self.available_paths.len() != self.available_symbols.len() {
            bail!(
                "invalid skeleton.available_symbols: expected available_symbols to align with skeleton.available_paths"
            );
        }

        for (index, symbol) in self.available_symbols.iter().enumerate() {
            symbol.validate_public_output(index)?;
            if self.available_paths[index] != symbol.semantic_path {
                bail!(
                    "invalid skeleton.available_paths[{index}]: expected available_paths to match skeleton.available_symbols semantic paths"
                );
            }
        }

        Ok(())
    }
}

impl SemanticSkeletonSymbol {
    fn validate_public_output(&self, index: usize) -> Result<()> {
        let prefix = format!("skeleton.available_symbols[{index}]");
        ensure_nonblank(&self.symbol_id, &format!("{prefix}.symbol_id"))?;
        ensure_nonblank(&self.semantic_path, &format!("{prefix}.semantic_path"))?;
        if let Some(scope_path) = &self.scope_path {
            ensure_nonblank(scope_path, &format!("{prefix}.scope_path"))?;
        }
        ensure_nonblank(&self.node_kind, &format!("{prefix}.node_kind"))?;
        if self.byte_range.0 > self.byte_range.1 {
            bail!("invalid {prefix}.byte_range: start byte is after end byte");
        }
        if let Some(signature) = &self.signature {
            ensure_nonblank(signature, &format!("{prefix}.signature"))?;
        }
        ensure_nonblank_strings(&self.parameters, &format!("{prefix}.parameters"))?;
        if let Some(return_type) = &self.return_type {
            ensure_nonblank(return_type, &format!("{prefix}.return_type"))?;
        }
        if let Some(docstring) = &self.docstring {
            ensure_nonblank(docstring, &format!("{prefix}.docstring"))?;
        }
        Ok(())
    }
}

impl QueryCaptureResult {
    pub(crate) fn validate_public_output(&self, index: usize) -> Result<()> {
        let prefix = format!("query.captures[{index}]");
        ensure_nonblank(&self.capture_name, &format!("{prefix}.capture_name"))?;
        ensure_nonblank(&self.node_kind, &format!("{prefix}.node_kind"))?;
        if self.start_byte > self.end_byte {
            bail!("invalid {prefix}: start byte is after end byte");
        }
        if point_is_after(&self.start_point, &self.end_point) {
            bail!("invalid {prefix}: start point is after end point");
        }

        match (&self.owner_symbol_id, &self.owner_semantic_path) {
            (Some(owner_symbol_id), Some(owner_semantic_path)) => {
                ensure_nonblank(owner_symbol_id, &format!("{prefix}.owner_symbol_id"))?;
                ensure_nonblank(
                    owner_semantic_path,
                    &format!("{prefix}.owner_semantic_path"),
                )?;
            }
            (None, None) => {}
            _ => {
                bail!(
                    "invalid {prefix}: expected owner_symbol_id and owner_semantic_path to either both be present or both be absent"
                );
            }
        }

        if let Some(owner_scope_path) = &self.owner_scope_path {
            ensure_nonblank(owner_scope_path, &format!("{prefix}.owner_scope_path"))?;
            if self.owner_semantic_path.is_none() {
                bail!(
                    "invalid {prefix}.owner_scope_path: expected owner_scope_path only when owner_semantic_path is present"
                );
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TraceDirection {
    Callers,
    Callees,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolMeta {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub evidence_key: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
    pub dependencies: Vec<String>,
    pub references: Vec<String>,
}

pub struct SymbolMetaInit {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
    pub dependencies: Vec<String>,
    pub references: Vec<String>,
}

impl SymbolMeta {
    pub fn new(init: SymbolMetaInit) -> Self {
        let evidence_key = symbol_evidence_key(
            &init.symbol_id,
            &init.file_path,
            &init.node_kind,
            &init.origin_type,
            init.byte_range,
            init.signature.as_deref(),
        );

        Self {
            symbol_id: init.symbol_id,
            semantic_path: init.semantic_path,
            scope_path: init.scope_path,
            file_path: init.file_path,
            node_kind: init.node_kind,
            origin_type: init.origin_type,
            evidence_key,
            byte_range: init.byte_range,
            signature: init.signature,
            parameters: init.parameters,
            return_type: init.return_type,
            docstring: init.docstring,
            dependencies: init.dependencies,
            references: init.references,
        }
    }

    pub fn with_origin_type(&self, origin_type: &str) -> Self {
        Self::new(SymbolMetaInit {
            symbol_id: self.symbol_id.clone(),
            semantic_path: self.semantic_path.clone(),
            scope_path: self.scope_path.clone(),
            file_path: self.file_path.clone(),
            node_kind: self.node_kind.clone(),
            origin_type: origin_type.to_string(),
            byte_range: self.byte_range,
            signature: self.signature.clone(),
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            docstring: self.docstring.clone(),
            dependencies: self.dependencies.clone(),
            references: self.references.clone(),
        })
    }

    pub fn validate_trace_replay_input(&self, field: &str) -> Result<()> {
        validate_symbol_identity(
            SymbolIdentityRef {
                symbol_id: &self.symbol_id,
                semantic_path: &self.semantic_path,
                file_path: &self.file_path,
                node_kind: &self.node_kind,
                origin_type: &self.origin_type,
                evidence_key: &self.evidence_key,
                byte_range: self.byte_range,
                signature: self.signature.as_deref(),
            },
            field,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSummary {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub evidence_key: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
}

pub struct SymbolSummaryInit {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
}

impl SymbolSummary {
    pub fn new(init: SymbolSummaryInit) -> Self {
        let evidence_key = symbol_evidence_key(
            &init.symbol_id,
            &init.file_path,
            &init.node_kind,
            &init.origin_type,
            init.byte_range,
            init.signature.as_deref(),
        );

        Self {
            symbol_id: init.symbol_id,
            semantic_path: init.semantic_path,
            scope_path: init.scope_path,
            file_path: init.file_path,
            node_kind: init.node_kind,
            origin_type: init.origin_type,
            evidence_key,
            byte_range: init.byte_range,
            signature: init.signature,
            parameters: init.parameters,
            return_type: init.return_type,
            docstring: init.docstring,
        }
    }

    pub fn validate_trace_replay_input(&self, field: &str) -> Result<()> {
        validate_symbol_identity(
            SymbolIdentityRef {
                symbol_id: &self.symbol_id,
                semantic_path: &self.semantic_path,
                file_path: &self.file_path,
                node_kind: &self.node_kind,
                origin_type: &self.origin_type,
                evidence_key: &self.evidence_key,
                byte_range: self.byte_range,
                signature: self.signature.as_deref(),
            },
            field,
        )
    }
}

fn symbol_evidence_key(
    symbol_id: &str,
    file_path: &str,
    node_kind: &str,
    origin_type: &str,
    byte_range: (usize, usize),
    signature: Option<&str>,
) -> String {
    format!(
        "{symbol_id}|{file_path}|{node_kind}|{origin_type}|{}..{}|{}",
        byte_range.0,
        byte_range.1,
        signature.unwrap_or("")
    )
}

fn ensure_nonblank(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("invalid {field}: value must not be blank");
    }
    Ok(())
}

fn ensure_nonblank_strings(values: &[String], field: &str) -> Result<()> {
    if let Some(index) = values.iter().position(|value| value.trim().is_empty()) {
        bail!("invalid {field}[{index}]: value must not be blank");
    }
    Ok(())
}

fn ensure_unique_strings(values: &[String], field: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !seen.insert(value.clone()) {
            bail!("invalid {field}[{index}]: duplicate values are not allowed");
        }
    }
    Ok(())
}

fn ensure_unique_symbol_evidence_keys(symbols: &[SymbolSummary], field: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (index, symbol) in symbols.iter().enumerate() {
        if !seen.insert(symbol.evidence_key.clone()) {
            bail!("invalid {field}[{index}].evidence_key: duplicate evidence keys are not allowed");
        }
    }
    Ok(())
}

fn point_is_after(start: &Position, end: &Position) -> bool {
    start.row > end.row || (start.row == end.row && start.column > end.column)
}

struct SymbolIdentityRef<'a> {
    symbol_id: &'a str,
    semantic_path: &'a str,
    file_path: &'a str,
    node_kind: &'a str,
    origin_type: &'a str,
    evidence_key: &'a str,
    byte_range: (usize, usize),
    signature: Option<&'a str>,
}

fn validate_symbol_identity(identity: SymbolIdentityRef<'_>, field: &str) -> Result<()> {
    ensure_nonblank(identity.symbol_id, &format!("{field}.symbol_id"))?;
    ensure_nonblank(identity.semantic_path, &format!("{field}.semantic_path"))?;
    ensure_nonblank(identity.file_path, &format!("{field}.file_path"))?;
    ensure_nonblank(identity.node_kind, &format!("{field}.node_kind"))?;
    ensure_nonblank(identity.origin_type, &format!("{field}.origin_type"))?;
    ensure_nonblank(identity.evidence_key, &format!("{field}.evidence_key"))?;
    if identity.byte_range.0 > identity.byte_range.1 {
        bail!("invalid {field}.byte_range: start byte is after end byte");
    }

    let expected = symbol_evidence_key(
        identity.symbol_id,
        identity.file_path,
        identity.node_kind,
        identity.origin_type,
        identity.byte_range,
        identity.signature,
    );
    if identity.evidence_key != expected {
        bail!("invalid {field}.evidence_key: expected evidence key to match symbol identity");
    }

    Ok(())
}

#[cfg(test)]
mod tests;
