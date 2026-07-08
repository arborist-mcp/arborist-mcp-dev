use std::collections::BTreeSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceEvidenceKeys {
    pub symbol: String,
    pub callers: Vec<String>,
    pub callees: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TracePatchEvidenceReplayItem {
    pub name: String,
    pub status: String,
    pub selected_evidence_key: Option<String>,
    pub matched_in_trace: bool,
    pub trace_match_scope: String,
    pub candidate_evidence_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TracePatchEvidenceReplayResult {
    pub consistent: bool,
    pub matched_items: usize,
    pub blocked_items: usize,
    pub items: Vec<TracePatchEvidenceReplayItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchTraceValidationResult {
    pub allowed: bool,
    pub status: String,
    pub reason: String,
    pub patch_gate_status: String,
    pub replay_status: String,
    pub replay: TracePatchEvidenceReplayResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceBackedPatchResult {
    pub patch: PatchAstNodeResult,
    pub trace_target: String,
    pub trace: Option<TraceSymbolGraphResult>,
    pub trace_validation: Option<PatchTraceValidationResult>,
    pub trace_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceSymbolGraphResult {
    pub symbol: SymbolMeta,
    pub callers: Vec<SymbolSummary>,
    pub callees: Vec<SymbolSummary>,
    pub evidence_keys: TraceEvidenceKeys,
    pub indexed_files: usize,
}

impl PatchCommitGateReport {
    fn validate_trace_replay_input(
        &self,
        applied: bool,
        bypass_applied: bool,
        syntax_error_count_expected: usize,
    ) -> Result<()> {
        ensure_nonblank(&self.status, "patch.validation.commit_gate.status")?;
        ensure_nonblank(&self.reason, "patch.validation.commit_gate.reason")?;
        if let Some(bypass_reason) = &self.bypass_reason {
            ensure_nonblank(bypass_reason, "patch.validation.commit_gate.bypass_reason")?;
        }
        if self.syntax_error_count != syntax_error_count_expected {
            bail!(
                "invalid patch.validation.commit_gate.syntax_error_count: expected {syntax_error_count_expected} to match patch.validation.syntax_errors"
            );
        }
        for (index, decision) in self.blocking_decisions.iter().enumerate() {
            let prefix = format!("patch.validation.commit_gate.blocking_decisions[{index}]");
            decision.validate_trace_replay_input(&prefix)?;
            if decision.status == "resolved" {
                bail!("invalid {prefix}.status: blocking decisions must not be resolved");
            }
        }
        for (index, invariant) in self.evidence_invariants.iter().enumerate() {
            invariant.validate_trace_replay_input(index)?;
        }

        let has_evidence_failure = self
            .evidence_invariants
            .iter()
            .any(|invariant| invariant.status == "failed");
        let has_gate_blocker = syntax_error_count_expected > 0
            || !self.blocking_decisions.is_empty()
            || has_evidence_failure;

        match self.status.as_str() {
            "allowed" => {
                if !self.allowed {
                    bail!(
                        "invalid patch.validation.commit_gate.allowed: expected true when status is allowed"
                    );
                }
                if self.bypass_reason.is_some() {
                    bail!(
                        "invalid patch.validation.commit_gate.bypass_reason: expected no bypass reason when status is allowed"
                    );
                }
                if has_gate_blocker {
                    bail!(
                        "invalid patch.validation.commit_gate.status: allowed patches must not report syntax errors, blocking decisions, or failed evidence invariants"
                    );
                }
            }
            "allowed_with_bypass" => {
                if !self.allowed {
                    bail!(
                        "invalid patch.validation.commit_gate.allowed: expected true when status is allowed_with_bypass"
                    );
                }
                if self.bypass_reason.is_none() {
                    bail!(
                        "invalid patch.validation.commit_gate.bypass_reason: expected a bypass reason when status is allowed_with_bypass"
                    );
                }
                if !has_gate_blocker {
                    bail!(
                        "invalid patch.validation.commit_gate.status: allowed_with_bypass requires syntax errors, blocking decisions, or failed evidence invariants"
                    );
                }
            }
            "rejected" => {
                if self.allowed {
                    bail!(
                        "invalid patch.validation.commit_gate.allowed: expected false when status is rejected"
                    );
                }
                if self.bypass_reason.is_some() {
                    bail!(
                        "invalid patch.validation.commit_gate.bypass_reason: expected no bypass reason when status is rejected"
                    );
                }
                if !has_gate_blocker {
                    bail!(
                        "invalid patch.validation.commit_gate.status: rejected patches must report syntax errors, blocking decisions, or failed evidence invariants"
                    );
                }
            }
            other => {
                bail!("invalid patch.validation.commit_gate.status: unsupported status `{other}`");
            }
        }

        if applied != self.allowed {
            bail!(
                "invalid patch.applied: expected patch.applied to match patch.validation.commit_gate.allowed"
            );
        }
        if bypass_applied != (self.status == "allowed_with_bypass") {
            bail!(
                "invalid patch.bypass_applied: expected patch.bypass_applied to match patch.validation.commit_gate.status"
            );
        }
        Ok(())
    }
}

impl PatchEvidenceInvariantReport {
    fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
        let prefix = format!("patch.validation.commit_gate.evidence_invariants[{index}]");
        ensure_nonblank(&self.name, &format!("{prefix}.name"))?;
        ensure_nonblank(&self.status, &format!("{prefix}.status"))?;
        ensure_nonblank(&self.reason, &format!("{prefix}.reason"))?;
        if let Some(selected_evidence_key) = &self.selected_evidence_key {
            ensure_nonblank(
                selected_evidence_key,
                &format!("{prefix}.selected_evidence_key"),
            )?;
        }
        ensure_nonblank_strings(
            &self.candidate_evidence_keys,
            &format!("{prefix}.candidate_evidence_keys"),
        )?;
        ensure_unique_strings(
            &self.candidate_evidence_keys,
            &format!("{prefix}.candidate_evidence_keys"),
        )?;
        match self.status.as_str() {
            "passed" => {
                let selected_evidence_key =
                    self.selected_evidence_key.as_deref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "invalid {prefix}.selected_evidence_key: expected a selected evidence key when status is passed"
                        )
                    })?;
                if !self
                    .candidate_evidence_keys
                    .iter()
                    .any(|candidate| candidate == selected_evidence_key)
                {
                    bail!(
                        "invalid {prefix}.selected_evidence_key: expected passed invariant selected evidence key to appear in candidate_evidence_keys"
                    );
                }
            }
            "blocked" => {
                if self.selected_evidence_key.is_some() {
                    bail!(
                        "invalid {prefix}.selected_evidence_key: expected no selected evidence key when status is blocked"
                    );
                }
            }
            "failed" => {}
            other => {
                bail!("invalid {prefix}.status: unsupported status `{other}`");
            }
        }
        Ok(())
    }
}

impl PatchValidationReport {
    fn validate_trace_replay_input(&self) -> Result<()> {
        for (index, issue) in self.syntax_errors.iter().enumerate() {
            issue.validate_trace_replay_input(index)?;
        }
        ensure_nonblank_strings(
            &self.unresolved_identifiers,
            "patch.validation.unresolved_identifiers",
        )?;
        for (index, binding) in self.resolved_identifiers.iter().enumerate() {
            binding.validate_trace_replay_input(index)?;
        }
        for (index, ambiguity) in self.ambiguous_identifiers.iter().enumerate() {
            ambiguity.validate_trace_replay_input(index)?;
        }
        for (index, decision) in self.binding_decisions.iter().enumerate() {
            decision.validate_trace_replay_input(&format!(
                "patch.validation.binding_decisions[{index}]"
            ))?;
        }
        self.validate_binding_summary_consistency()?;
        Ok(())
    }

    fn validate_binding_summary_consistency(&self) -> Result<()> {
        let mut expected_unresolved = Vec::new();
        let mut expected_resolved = Vec::new();
        let mut expected_ambiguous = Vec::new();

        for decision in &self.binding_decisions {
            match decision.status.as_str() {
                "resolved"
                    if !expected_unresolved
                        .iter()
                        .any(|name| name == &decision.name)
                        && !expected_ambiguous.iter().any(|name| name == &decision.name)
                        && !expected_resolved.iter().any(|name| name == &decision.name) =>
                {
                    expected_resolved.push(decision.name.clone());
                }
                "ambiguous"
                    if !expected_unresolved
                        .iter()
                        .any(|name| name == &decision.name) =>
                {
                    expected_resolved.retain(|name| name != &decision.name);
                    if !expected_ambiguous.iter().any(|name| name == &decision.name) {
                        expected_ambiguous.push(decision.name.clone());
                    }
                }
                "unresolved" => {
                    expected_resolved.retain(|name| name != &decision.name);
                    expected_ambiguous.retain(|name| name != &decision.name);
                    if !expected_unresolved
                        .iter()
                        .any(|name| name == &decision.name)
                    {
                        expected_unresolved.push(decision.name.clone());
                    }
                }
                _ => {}
            }
        }

        if self.unresolved_identifiers != expected_unresolved {
            bail!(
                "invalid patch.validation.unresolved_identifiers: expected unresolved identifier summary derived from patch.validation.binding_decisions"
            );
        }

        let resolved_names = self
            .resolved_identifiers
            .iter()
            .map(|binding| binding.name.clone())
            .collect::<Vec<_>>();
        if resolved_names != expected_resolved {
            bail!(
                "invalid patch.validation.resolved_identifiers: expected resolved binding summary derived from patch.validation.binding_decisions"
            );
        }

        let ambiguous_names = self
            .ambiguous_identifiers
            .iter()
            .map(|ambiguity| ambiguity.name.clone())
            .collect::<Vec<_>>();
        if ambiguous_names != expected_ambiguous {
            bail!(
                "invalid patch.validation.ambiguous_identifiers: expected ambiguous binding summary derived from patch.validation.binding_decisions"
            );
        }

        let mut seen_resolved = BTreeSet::new();
        for (index, binding) in self.resolved_identifiers.iter().enumerate() {
            if !seen_resolved.insert(binding.name.clone()) {
                bail!(
                    "invalid patch.validation.resolved_identifiers[{index}].name: duplicate resolved binding names are not allowed"
                );
            }
            let has_match = self.binding_decisions.iter().any(|decision| {
                decision.status == "resolved"
                    && decision.name == binding.name
                    && decision.selected_symbol_id.as_deref()
                        == Some(binding.symbol.symbol_id.as_str())
                    && decision.candidates.first() == Some(&binding.symbol)
            });
            if !has_match {
                bail!(
                    "invalid patch.validation.resolved_identifiers[{index}]: expected resolved binding summary to match a resolved patch.validation.binding_decisions entry"
                );
            }
        }

        let mut seen_ambiguous = BTreeSet::new();
        for (index, ambiguity) in self.ambiguous_identifiers.iter().enumerate() {
            if !seen_ambiguous.insert(ambiguity.name.clone()) {
                bail!(
                    "invalid patch.validation.ambiguous_identifiers[{index}].name: duplicate ambiguous binding names are not allowed"
                );
            }
            let has_match = self.binding_decisions.iter().any(|decision| {
                decision.status == "ambiguous"
                    && decision.name == ambiguity.name
                    && decision.reason == ambiguity.reason
                    && decision.candidates == ambiguity.candidates
            });
            if !has_match {
                bail!(
                    "invalid patch.validation.ambiguous_identifiers[{index}]: expected ambiguous binding summary to match an ambiguous patch.validation.binding_decisions entry"
                );
            }
        }

        Ok(())
    }
}

impl ValidationBinding {
    fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
        let prefix = format!("patch.validation.resolved_identifiers[{index}]");
        ensure_nonblank(&self.name, &format!("{prefix}.name"))?;
        self.symbol
            .validate_trace_replay_input(&format!("{prefix}.symbol"))
    }
}

impl ValidationAmbiguity {
    fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
        let prefix = format!("patch.validation.ambiguous_identifiers[{index}]");
        ensure_nonblank(&self.name, &format!("{prefix}.name"))?;
        ensure_nonblank(&self.reason, &format!("{prefix}.reason"))?;
        if self.candidates.len() < 2 {
            bail!(
                "invalid {prefix}.candidates: ambiguous bindings must contain at least two candidates"
            );
        }
        for (candidate_index, candidate) in self.candidates.iter().enumerate() {
            candidate
                .validate_trace_replay_input(&format!("{prefix}.candidates[{candidate_index}]"))?;
        }
        ensure_unique_symbol_evidence_keys(&self.candidates, &format!("{prefix}.candidates"))?;
        self.disambiguation_context
            .validate_trace_replay_input(&format!("{prefix}.disambiguation_context"))
    }
}

impl ValidationBindingDecision {
    fn validate_trace_replay_input(&self, field: &str) -> Result<()> {
        ensure_nonblank(&self.name, &format!("{field}.name"))?;
        ensure_nonblank(&self.status, &format!("{field}.status"))?;
        ensure_nonblank(&self.reason, &format!("{field}.reason"))?;
        if let Some(selected_symbol_id) = &self.selected_symbol_id {
            ensure_nonblank(selected_symbol_id, &format!("{field}.selected_symbol_id"))?;
        }
        for (index, candidate) in self.candidates.iter().enumerate() {
            candidate.validate_trace_replay_input(&format!("{field}.candidates[{index}]"))?;
        }
        ensure_unique_symbol_evidence_keys(&self.candidates, &format!("{field}.candidates"))?;

        match self.status.as_str() {
            "resolved" => {
                let selected_symbol_id = self.selected_symbol_id.as_deref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "invalid {field}.selected_symbol_id: expected a selected symbol id when status is resolved"
                    )
                })?;
                if self.candidates.len() != 1 {
                    bail!(
                        "invalid {field}.candidates: resolved bindings must contain exactly one candidate"
                    );
                }
                if self.candidates[0].symbol_id != selected_symbol_id {
                    bail!(
                        "invalid {field}.selected_symbol_id: expected resolved selected symbol id to match the only candidate"
                    );
                }
            }
            "ambiguous" => {
                if self.selected_symbol_id.is_some() {
                    bail!(
                        "invalid {field}.selected_symbol_id: expected no selected symbol id when status is ambiguous"
                    );
                }
                if self.candidates.len() < 2 {
                    bail!(
                        "invalid {field}.candidates: ambiguous bindings must contain at least two candidates"
                    );
                }
            }
            "unresolved" => {
                if self.selected_symbol_id.is_some() {
                    bail!(
                        "invalid {field}.selected_symbol_id: expected no selected symbol id when status is unresolved"
                    );
                }
                if !self.candidates.is_empty() {
                    bail!(
                        "invalid {field}.candidates: unresolved bindings must not contain candidates"
                    );
                }
            }
            other => {
                bail!("invalid {field}.status: unsupported status `{other}`");
            }
        }

        Ok(())
    }
}

impl DisambiguationContext {
    fn validate_trace_replay_input(&self, field: &str) -> Result<()> {
        if let Some(active_include_family) = &self.active_include_family {
            ensure_nonblank(
                active_include_family,
                &format!("{field}.active_include_family"),
            )?;
        }
        if let Some(preferred_family) = &self.preferred_family {
            ensure_nonblank(preferred_family, &format!("{field}.preferred_family"))?;
        }
        ensure_nonblank_strings(
            &self.visible_include_families,
            &format!("{field}.visible_include_families"),
        )?;
        ensure_nonblank_strings(
            &self.candidate_include_families,
            &format!("{field}.candidate_include_families"),
        )?;
        ensure_nonblank_strings(
            &self.candidate_symbol_ids,
            &format!("{field}.candidate_symbol_ids"),
        )?;
        Ok(())
    }
}

impl ValidationIssue {
    fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
        let prefix = format!("patch.validation.syntax_errors[{index}]");
        ensure_nonblank(&self.kind, &format!("{prefix}.kind"))?;
        ensure_nonblank(&self.message, &format!("{prefix}.message"))?;
        match self.kind.as_str() {
            "error" | "missing" => {}
            other => {
                bail!("invalid {prefix}.kind: unsupported syntax issue kind `{other}`");
            }
        }
        if self.start_byte > self.end_byte {
            bail!("invalid {prefix}: start byte is after end byte");
        }
        if point_is_after(&self.start_point, &self.end_point) {
            bail!("invalid {prefix}: start point is after end point");
        }
        Ok(())
    }
}

impl TraceSymbolGraphResult {
    pub fn validate_trace_replay_input(&self) -> Result<()> {
        self.symbol.validate_trace_replay_input("trace.symbol")?;
        if self.symbol.origin_type != "trace_root" {
            bail!(
                "invalid trace.symbol.origin_type: expected traced root symbol origin type to be `trace_root`"
            );
        }
        for (index, caller) in self.callers.iter().enumerate() {
            caller.validate_trace_replay_input(&format!("trace.callers[{index}]"))?;
        }
        for (index, callee) in self.callees.iter().enumerate() {
            callee.validate_trace_replay_input(&format!("trace.callees[{index}]"))?;
        }
        ensure_unique_symbol_evidence_keys(&self.callers, "trace.callers")?;
        ensure_unique_symbol_evidence_keys(&self.callees, "trace.callees")?;

        let expected_callers = self
            .callers
            .iter()
            .map(|symbol| symbol.evidence_key.clone())
            .collect::<Vec<_>>();
        let expected_callees = self
            .callees
            .iter()
            .map(|symbol| symbol.evidence_key.clone())
            .collect::<Vec<_>>();

        if self.evidence_keys.symbol != self.symbol.evidence_key {
            bail!(
                "invalid trace.evidence_keys.symbol: expected traced symbol evidence key to match trace.symbol.evidence_key"
            );
        }
        if self.evidence_keys.callers != expected_callers {
            bail!(
                "invalid trace.evidence_keys.callers: expected caller evidence keys to match trace.callers"
            );
        }
        if self.evidence_keys.callees != expected_callees {
            bail!(
                "invalid trace.evidence_keys.callees: expected callee evidence keys to match trace.callees"
            );
        }

        Ok(())
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolIndexStats {
    pub db_path: String,
    pub indexed_files: usize,
    pub indexed_symbols: usize,
    pub rebuilt_files: usize,
    pub reused_files: usize,
}

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
pub struct RegisteredSymbolIndex {
    pub workspace_root: String,
    pub db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VirtualFileStatus {
    pub file: String,
    pub dirty: bool,
    pub version: u64,
    pub syntax_error_count: usize,
}

#[cfg(test)]
mod tests {
    use super::{
        PatchAstNodeResult, PatchTraceValidationResult, Position, PositionEdit, QueryCaptureResult,
        RegisteredSymbolIndex, SemanticSkeleton, SymbolIndexStats, TraceBackedPatchResult,
        TracePatchEvidenceReplayResult, TraceSymbolGraphResult, VirtualEditResult,
        VirtualFileSnapshot, VirtualFileStatus,
    };

    #[test]
    fn position_rejects_unknown_fields() {
        let error = serde_json::from_str::<Position>(r#"{"row":0,"column":0,"character":0}"#)
            .expect_err("positions should reject unknown fields");

        assert!(error.to_string().contains("unknown field `character`"));
    }

    #[test]
    fn position_edit_rejects_unknown_fields() {
        let error = serde_json::from_str::<PositionEdit>(
            r#"{"start":{"row":0,"column":0},"end":{"row":0,"column":0},"new_text":"x","newText":"x"}"#,
        )
        .expect_err("position edits should reject unknown fields");

        assert!(error.to_string().contains("unknown field `newText`"));
    }

    #[test]
    fn patch_result_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<PatchAstNodeResult>(
            r#"{
                "file":"sample.py",
                "target_path":"top_level",
                "resolved_path":"top_level",
                "resolved_symbol_id":"top_level",
                "applied":true,
                "bypass_applied":false,
                "updated_source":"def top_level() -> int:\n    return 1\n",
                "validation":{
                    "syntax_errors":[],
                    "unresolved_identifiers":[],
                    "resolved_identifiers":[],
                    "ambiguous_identifiers":[],
                    "binding_decisions":[],
                    "commit_gate":{
                        "status":"allowed",
                        "allowed":true,
                        "reason":"ok",
                        "bypass_reason":null,
                        "blocking_decisions":[],
                        "evidence_invariants":[],
                        "syntax_error_count":0,
                        "unexpected":true
                    }
                }
            }"#,
        )
        .expect_err("patch results should reject unknown nested fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn patch_result_rejects_missing_nested_fields() {
        let error = serde_json::from_str::<PatchAstNodeResult>(
            r#"{
                "file":"sample.py",
                "target_path":"top_level",
                "resolved_path":"top_level",
                "resolved_symbol_id":"top_level",
                "applied":true,
                "bypass_applied":false,
                "updated_source":"def top_level() -> int:\n    return 1\n",
                "validation":{
                    "syntax_errors":[],
                    "resolved_identifiers":[],
                    "ambiguous_identifiers":[],
                    "binding_decisions":[],
                    "commit_gate":{
                        "status":"allowed",
                        "allowed":true,
                        "reason":"ok",
                        "bypass_reason":null,
                        "blocking_decisions":[],
                        "evidence_invariants":[],
                        "syntax_error_count":0
                    }
                }
            }"#,
        )
        .expect_err("patch results should reject missing nested validation fields");

        assert!(error.to_string().contains("missing field"));
    }

    #[test]
    fn trace_result_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<TraceSymbolGraphResult>(
            r#"{
                "symbol":{
                    "symbol_id":"top_level",
                    "semantic_path":"top_level",
                    "file_path":"sample.py",
                    "node_kind":"function_definition",
                    "origin_type":"trace_root",
                    "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range":[0,10],
                    "parameters":[],
                    "dependencies":[],
                    "references":[],
                    "unexpected":true
                },
                "callers":[],
                "callees":[],
                "evidence_keys":{
                    "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers":[],
                    "callees":[]
                },
                "indexed_files":1
            }"#,
        )
        .expect_err("trace results should reject unknown nested fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn trace_result_rejects_missing_nested_fields() {
        let error = serde_json::from_str::<TraceSymbolGraphResult>(
            r#"{
                "symbol":{
                    "symbol_id":"top_level"
                },
                "callers":[],
                "callees":[],
                "evidence_keys":{
                    "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers":[],
                    "callees":[]
                },
                "indexed_files":1
            }"#,
        )
        .expect_err("trace results should reject missing nested symbol fields");

        assert!(error.to_string().contains("missing field"));
    }

    #[test]
    fn replay_result_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<TracePatchEvidenceReplayResult>(
            r#"{
                "consistent":true,
                "matched_items":1,
                "blocked_items":0,
                "items":[{
                    "name":"helper",
                    "status":"matched",
                    "selected_evidence_key":"helper|sample.py|function_definition|local_file|0..10|",
                    "matched_in_trace":true,
                    "trace_match_scope":"callees",
                    "candidate_evidence_keys":["helper|sample.py|function_definition|local_file|0..10|"],
                    "unexpected":true
                }]
            }"#,
        )
        .expect_err("replay results should reject unknown nested fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn trace_validation_result_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<PatchTraceValidationResult>(
            r#"{
                "allowed":true,
                "status":"allowed",
                "reason":"ok",
                "patch_gate_status":"allowed",
                "replay_status":"matched",
                "replay":{
                    "consistent":true,
                    "matched_items":1,
                    "blocked_items":0,
                    "items":[{
                        "name":"helper",
                        "status":"matched",
                        "selected_evidence_key":"helper|sample.py|function_definition|local_file|0..10|",
                        "matched_in_trace":true,
                        "trace_match_scope":"callees",
                        "candidate_evidence_keys":["helper|sample.py|function_definition|local_file|0..10|"]
                    }],
                    "unexpected":true
                }
            }"#,
        )
        .expect_err("trace validation results should reject unknown nested replay fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn trace_backed_patch_result_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<TraceBackedPatchResult>(
            r#"{
                "patch":{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":false,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return missing_helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":["missing_helper"],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"missing_helper",
                            "status":"unresolved",
                            "reason":"identifier is not visible from the patched symbol scope",
                            "selected_symbol_id":null,
                            "candidates":[]
                        }],
                        "commit_gate":{
                            "status":"rejected",
                            "allowed":false,
                            "reason":"symbol binding could not be resolved",
                            "bypass_reason":null,
                            "blocking_decisions":[{
                                "name":"missing_helper",
                                "status":"unresolved",
                                "reason":"identifier is not visible from the patched symbol scope",
                                "selected_symbol_id":null,
                                "candidates":[]
                            }],
                            "evidence_invariants":[{
                                "name":"missing_helper",
                                "status":"blocked",
                                "reason":"no candidate evidence key is available for this binding",
                                "selected_evidence_key":null,
                                "candidate_evidence_keys":[]
                            }],
                            "syntax_error_count":0
                        }
                    }
                },
                "trace_target":"top_level",
                "trace":null,
                "trace_validation":null,
                "trace_error":"trace skipped because patch validation rejected the patch",
                "unexpected":true
            }"#,
        )
        .expect_err("trace-backed patch results should reject unknown top-level fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn semantic_skeleton_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<SemanticSkeleton>(
            r#"{
                "file":"sample.py",
                "skeleton":"def top_level() -> int:\n    return 1\n",
                "available_paths":["top_level"],
                "available_symbols":[{
                    "symbol_id":"sample.py::top_level",
                    "semantic_path":"top_level",
                    "node_kind":"function_definition",
                    "byte_range":[0,10],
                    "parameters":[],
                    "unexpected":true
                }]
            }"#,
        )
        .expect_err("semantic skeletons should reject unknown nested symbol fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn query_capture_result_rejects_unknown_fields() {
        let error = serde_json::from_str::<QueryCaptureResult>(
            r#"{
                "capture_name":"name",
                "node_kind":"identifier",
                "text":"top_level",
                "owner_symbol_id":"sample.py::top_level",
                "owner_semantic_path":"top_level",
                "owner_scope_path":null,
                "start_byte":0,
                "end_byte":9,
                "start_point":{"row":0,"column":0},
                "end_point":{"row":0,"column":9},
                "unexpected":true
            }"#,
        )
        .expect_err("query capture results should reject unknown fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn symbol_index_stats_reject_unknown_fields() {
        let error = serde_json::from_str::<SymbolIndexStats>(
            r#"{
                "db_path":"symbols.db",
                "indexed_files":1,
                "indexed_symbols":2,
                "rebuilt_files":1,
                "reused_files":0,
                "unexpected":true
            }"#,
        )
        .expect_err("symbol index stats should reject unknown fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn virtual_results_reject_unknown_fields() {
        let snapshot_error = serde_json::from_str::<VirtualFileSnapshot>(
            r#"{
                "file":"sample.py",
                "source":"def top_level() -> int:\n    return 1\n",
                "disk_source":"def top_level() -> int:\n    return 1\n",
                "dirty":false,
                "version":1,
                "syntax_error_count":0,
                "unexpected":true
            }"#,
        )
        .expect_err("virtual file snapshots should reject unknown fields");
        assert!(
            snapshot_error
                .to_string()
                .contains("unknown field `unexpected`")
        );

        let edit_error = serde_json::from_str::<VirtualEditResult>(
            r#"{
                "file":"sample.py",
                "source":"def top_level() -> int:\n    return 1\n",
                "dirty":false,
                "version":1,
                "incremental_parse":true,
                "validation":{
                    "syntax_errors":[],
                    "unresolved_identifiers":[],
                    "resolved_identifiers":[],
                    "ambiguous_identifiers":[],
                    "binding_decisions":[],
                    "commit_gate":{
                        "status":"allowed",
                        "allowed":true,
                        "reason":"ok",
                        "bypass_reason":null,
                        "blocking_decisions":[],
                        "evidence_invariants":[],
                        "syntax_error_count":0
                    }
                },
                "unexpected":true
            }"#,
        )
        .expect_err("virtual edit results should reject unknown fields");
        assert!(
            edit_error
                .to_string()
                .contains("unknown field `unexpected`")
        );

        let registration_error = serde_json::from_str::<RegisteredSymbolIndex>(
            r#"{
                "workspace_root":"workspace",
                "db_path":"symbols.db",
                "unexpected":true
            }"#,
        )
        .expect_err("registered symbol index results should reject unknown fields");
        assert!(
            registration_error
                .to_string()
                .contains("unknown field `unexpected`")
        );

        let status_error = serde_json::from_str::<VirtualFileStatus>(
            r#"{
                "file":"sample.py",
                "dirty":false,
                "version":1,
                "syntax_error_count":0,
                "unexpected":true
            }"#,
        )
        .expect_err("virtual file status results should reject unknown fields");
        assert!(
            status_error
                .to_string()
                .contains("unknown field `unexpected`")
        );
    }
}
