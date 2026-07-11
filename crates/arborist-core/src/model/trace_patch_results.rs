use std::collections::BTreeSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::{
    DisambiguationContext, PatchAstNodeResult, PatchCommitGateReport, PatchEvidenceInvariantReport,
    PatchValidationReport, SymbolMeta, SymbolNeighborhoodContextResult, SymbolReadResult,
    SymbolSummary, TraceDirection, ValidationAmbiguity, ValidationBinding,
    ValidationBindingDecision, ValidationIssue, ensure_nonblank, ensure_nonblank_strings,
    ensure_unique_strings, ensure_unique_symbol_evidence_keys, point_is_after,
};

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
pub struct GraphBackedPatchResult {
    pub patch: PatchAstNodeResult,
    pub trace_target: String,
    pub trace: Option<TraceSymbolGraphResult>,
    pub neighborhood: Option<TraceSymbolNeighborhoodResult>,
    pub trace_validation: Option<PatchTraceValidationResult>,
    pub trace_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NeighborhoodContextPatchResult {
    pub patch: PatchAstNodeResult,
    pub trace_target: String,
    pub trace: Option<TraceSymbolGraphResult>,
    pub neighborhood_context: Option<SymbolNeighborhoodContextResult>,
    pub trace_validation: Option<PatchTraceValidationResult>,
    pub trace_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryContextPatchResult {
    pub patch: PatchAstNodeResult,
    pub trace_target: String,
    pub trace: Option<TraceSymbolGraphResult>,
    pub read: Option<SymbolReadResult>,
    pub neighborhood_context: Option<SymbolNeighborhoodContextResult>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceSymbolNeighborhoodNode {
    pub symbol: SymbolSummary,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceSymbolNeighborhoodEdge {
    pub from_symbol_id: String,
    pub to_symbol_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceSymbolNeighborhoodResult {
    pub symbol: SymbolMeta,
    pub direction: TraceDirection,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub truncated: bool,
    pub indexed_files: usize,
    pub nodes: Vec<TraceSymbolNeighborhoodNode>,
    pub edges: Vec<TraceSymbolNeighborhoodEdge>,
}

impl PatchCommitGateReport {
    pub(super) fn validate_trace_replay_input(
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
    pub(super) fn validate_trace_replay_input(&self) -> Result<()> {
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
    pub(super) fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
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
    pub(super) fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
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

    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.validate_trace_replay_input()
    }
}

impl TraceSymbolNeighborhoodNode {
    pub(super) fn validate_public_output(&self, index: usize) -> Result<()> {
        self.symbol
            .validate_trace_replay_input(&format!("trace_neighborhood.nodes[{index}].symbol"))?;
        Ok(())
    }
}

impl TraceSymbolNeighborhoodEdge {
    pub(super) fn validate_public_output(&self, index: usize) -> Result<()> {
        ensure_nonblank(
            &self.from_symbol_id,
            &format!("trace_neighborhood.edges[{index}].from_symbol_id"),
        )?;
        ensure_nonblank(
            &self.to_symbol_id,
            &format!("trace_neighborhood.edges[{index}].to_symbol_id"),
        )?;
        if self.from_symbol_id == self.to_symbol_id {
            bail!("invalid trace_neighborhood.edges[{index}]: self-edges are not allowed");
        }
        Ok(())
    }
}

impl TracePatchEvidenceReplayItem {
    fn validate_public_output(&self, index: usize) -> Result<()> {
        let prefix = format!("trace_replay.items[{index}]");
        ensure_nonblank(&self.name, &format!("{prefix}.name"))?;
        ensure_nonblank(&self.status, &format!("{prefix}.status"))?;
        if let Some(selected_evidence_key) = &self.selected_evidence_key {
            ensure_nonblank(
                selected_evidence_key,
                &format!("{prefix}.selected_evidence_key"),
            )?;
        }
        ensure_nonblank(
            &self.trace_match_scope,
            &format!("{prefix}.trace_match_scope"),
        )?;
        ensure_nonblank_strings(
            &self.candidate_evidence_keys,
            &format!("{prefix}.candidate_evidence_keys"),
        )?;
        ensure_unique_strings(
            &self.candidate_evidence_keys,
            &format!("{prefix}.candidate_evidence_keys"),
        )?;

        match self.trace_match_scope.as_str() {
            "callers" | "callees" | "symbol" | "patch_scope" | "none" => {}
            other => {
                bail!("invalid {prefix}.trace_match_scope: unsupported scope `{other}`");
            }
        }
        if self.matched_in_trace && self.trace_match_scope == "none" {
            bail!(
                "invalid {prefix}.trace_match_scope: expected a concrete scope when matched_in_trace is true"
            );
        }
        if !self.matched_in_trace && self.trace_match_scope != "none" {
            bail!(
                "invalid {prefix}.trace_match_scope: expected `none` when matched_in_trace is false"
            );
        }

        match self.status.as_str() {
            "matched" => {
                if !self.matched_in_trace {
                    bail!(
                        "invalid {prefix}.matched_in_trace: expected matched replay items to be matched in trace"
                    );
                }
                if self.selected_evidence_key.is_none() {
                    bail!(
                        "invalid {prefix}.selected_evidence_key: expected matched replay items to include a selected evidence key"
                    );
                }
            }
            "missing" => {
                if self.matched_in_trace {
                    bail!(
                        "invalid {prefix}.matched_in_trace: expected missing replay items not to be matched in trace"
                    );
                }
                if self.selected_evidence_key.is_none() {
                    bail!(
                        "invalid {prefix}.selected_evidence_key: expected missing replay items to include a selected evidence key"
                    );
                }
            }
            "blocked" => {
                if self.matched_in_trace {
                    bail!(
                        "invalid {prefix}.matched_in_trace: expected blocked replay items not to be matched in trace"
                    );
                }
                if self.selected_evidence_key.is_some() {
                    bail!(
                        "invalid {prefix}.selected_evidence_key: expected blocked replay items not to include a selected evidence key"
                    );
                }
            }
            "failed" => {}
            other => {
                bail!("invalid {prefix}.status: unsupported replay status `{other}`");
            }
        }

        Ok(())
    }
}

impl TracePatchEvidenceReplayResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        for (index, item) in self.items.iter().enumerate() {
            item.validate_public_output(index)?;
        }

        let expected_matched_items = self
            .items
            .iter()
            .filter(|item| item.status == "matched")
            .count();
        if self.matched_items != expected_matched_items {
            bail!(
                "invalid trace_replay.matched_items: expected matched_items to match replay item statuses"
            );
        }

        let expected_blocked_items = self
            .items
            .iter()
            .filter(|item| item.status == "blocked")
            .count();
        if self.blocked_items != expected_blocked_items {
            bail!(
                "invalid trace_replay.blocked_items: expected blocked_items to match replay item statuses"
            );
        }

        let expected_consistent = self
            .items
            .iter()
            .all(|item| matches!(item.status.as_str(), "matched" | "blocked"));
        if self.consistent != expected_consistent {
            bail!(
                "invalid trace_replay.consistent: expected consistent to match replay item statuses"
            );
        }

        Ok(())
    }
}

impl PatchTraceValidationResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.status, "trace_validation.status")?;
        ensure_nonblank(&self.reason, "trace_validation.reason")?;
        ensure_nonblank(
            &self.patch_gate_status,
            "trace_validation.patch_gate_status",
        )?;
        ensure_nonblank(&self.replay_status, "trace_validation.replay_status")?;
        self.replay.validate_public_output()?;

        let expected_replay_status = summarize_replay_status(&self.replay);
        if self.replay_status != expected_replay_status {
            bail!(
                "invalid trace_validation.replay_status: expected replay_status to match replay item statuses"
            );
        }

        match self.patch_gate_status.as_str() {
            "allowed" | "allowed_with_bypass" | "rejected" => {}
            other => {
                bail!(
                    "invalid trace_validation.patch_gate_status: unsupported patch gate status `{other}`"
                );
            }
        }

        match self.status.as_str() {
            "rejected_by_patch_gate" => {
                if self.allowed {
                    bail!(
                        "invalid trace_validation.allowed: rejected_by_patch_gate results must not be allowed"
                    );
                }
                if self.patch_gate_status != "rejected" {
                    bail!(
                        "invalid trace_validation.patch_gate_status: rejected_by_patch_gate results must report a rejected patch gate"
                    );
                }
            }
            "rejected_by_trace_replay" => {
                if self.allowed {
                    bail!(
                        "invalid trace_validation.allowed: rejected_by_trace_replay results must not be allowed"
                    );
                }
                if self.patch_gate_status == "rejected" {
                    bail!(
                        "invalid trace_validation.patch_gate_status: rejected_by_trace_replay results require the patch gate to have allowed the patch"
                    );
                }
                if !matches!(
                    self.replay_status.as_str(),
                    "missing" | "failed" | "blocked"
                ) {
                    bail!(
                        "invalid trace_validation.replay_status: rejected_by_trace_replay results require missing, failed, or blocked replay evidence"
                    );
                }
                if self.replay_status == "blocked"
                    && self.patch_gate_status == "allowed_with_bypass"
                {
                    bail!(
                        "invalid trace_validation.patch_gate_status: blocked replay evidence with an allowed_with_bypass patch gate should not be rejected by trace replay"
                    );
                }
            }
            "allowed" => {
                if !self.allowed {
                    bail!("invalid trace_validation.allowed: allowed results must be allowed");
                }
                if self.patch_gate_status != "allowed" {
                    bail!(
                        "invalid trace_validation.patch_gate_status: allowed results must report an allowed patch gate"
                    );
                }
                if self.replay_status != "matched" {
                    bail!(
                        "invalid trace_validation.replay_status: allowed results require matched replay evidence"
                    );
                }
            }
            "allowed_with_bypass" => {
                if !self.allowed {
                    bail!(
                        "invalid trace_validation.allowed: allowed_with_bypass results must be allowed"
                    );
                }
                if self.patch_gate_status != "allowed_with_bypass" {
                    bail!(
                        "invalid trace_validation.patch_gate_status: allowed_with_bypass results must report an allowed_with_bypass patch gate"
                    );
                }
                if !matches!(self.replay_status.as_str(), "matched" | "blocked") {
                    bail!(
                        "invalid trace_validation.replay_status: allowed_with_bypass results require matched or blocked replay evidence"
                    );
                }
            }
            other => {
                bail!(
                    "invalid trace_validation.status: unsupported trace validation status `{other}`"
                );
            }
        }

        Ok(())
    }
}

impl TraceBackedPatchResult {
    pub(crate) fn trace_skip_reason_for_syntax_errors() -> &'static str {
        "trace skipped because patch validation reported syntax errors"
    }

    pub(crate) fn trace_skip_reason_for_patch_gate_rejection() -> &'static str {
        "trace skipped because patch validation rejected the patch"
    }

    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                Self::trace_skip_reason_for_syntax_errors()
            } else {
                Self::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }

        Ok(())
    }
}

impl GraphBackedPatchResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.neighborhood.is_some() {
                bail!(
                    "invalid neighborhood: expected no neighborhood when the patch was not safely applied"
                );
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors()
            } else {
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let neighborhood = self.neighborhood.as_ref().ok_or_else(|| {
            anyhow::anyhow!("invalid neighborhood: expected neighborhood for applied patches")
        })?;
        neighborhood.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }
        if neighborhood.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid neighborhood.symbol.symbol_id: expected neighborhood root symbol id to match patch.resolved_symbol_id"
            );
        }
        if neighborhood.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid neighborhood.symbol.semantic_path: expected neighborhood root semantic path to match patch.resolved_path"
            );
        }
        if neighborhood.symbol.file_path != self.patch.file {
            bail!(
                "invalid neighborhood.symbol.file_path: expected neighborhood root file path to match patch.file"
            );
        }
        if neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
            bail!(
                "invalid neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
            );
        }

        Ok(())
    }
}

impl NeighborhoodContextPatchResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.neighborhood_context.is_some() {
                bail!(
                    "invalid neighborhood_context: expected no neighborhood_context when the patch was not safely applied"
                );
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors()
            } else {
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let neighborhood_context = self.neighborhood_context.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid neighborhood_context: expected neighborhood_context for applied patches"
            )
        })?;
        neighborhood_context.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }

        let neighborhood = &neighborhood_context.neighborhood;
        if neighborhood.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root symbol id to match patch.resolved_symbol_id"
            );
        }
        if neighborhood.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.semantic_path: expected neighborhood root semantic path to match patch.resolved_path"
            );
        }
        if neighborhood.symbol.file_path != self.patch.file {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.file_path: expected neighborhood root file path to match patch.file"
            );
        }
        if neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
            );
        }

        Ok(())
    }
}

impl DiscoveryContextPatchResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.read.is_some() {
                bail!("invalid read: expected no read when the patch was not safely applied");
            }
            if self.neighborhood_context.is_some() {
                bail!(
                    "invalid neighborhood_context: expected no neighborhood_context when the patch was not safely applied"
                );
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors()
            } else {
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let read = self
            .read
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid read: expected read for applied patches"))?;
        read.validate_public_output()?;
        let neighborhood_context = self.neighborhood_context.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid neighborhood_context: expected neighborhood_context for applied patches"
            )
        })?;
        neighborhood_context.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }
        if read.indexed_files != trace.indexed_files {
            bail!(
                "invalid read.indexed_files: expected read.indexed_files to match trace.indexed_files"
            );
        }
        if read.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid read.symbol.symbol_id: expected read symbol id to match patch.resolved_symbol_id"
            );
        }
        if read.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid read.symbol.semantic_path: expected read semantic path to match patch.resolved_path"
            );
        }
        if read.symbol.file_path != self.patch.file {
            bail!("invalid read.symbol.file_path: expected read file path to match patch.file");
        }
        let neighborhood = &neighborhood_context.neighborhood;
        if neighborhood.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root symbol id to match patch.resolved_symbol_id"
            );
        }
        if neighborhood.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.semantic_path: expected neighborhood root semantic path to match patch.resolved_path"
            );
        }
        if neighborhood.symbol.file_path != self.patch.file {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.file_path: expected neighborhood root file path to match patch.file"
            );
        }
        if neighborhood.indexed_files != trace.indexed_files {
            bail!(
                "invalid neighborhood_context.neighborhood.indexed_files: expected neighborhood indexed_files to match trace.indexed_files"
            );
        }
        if read.symbol.symbol_id != trace.symbol.symbol_id {
            bail!("invalid read.symbol.symbol_id: expected read symbol id to match trace root");
        }
        if neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
            );
        }

        Ok(())
    }
}

fn summarize_replay_status(replay: &TracePatchEvidenceReplayResult) -> String {
    if replay.items.iter().any(|item| item.status == "failed") {
        return "failed".to_string();
    }
    if replay.items.iter().any(|item| item.status == "missing") {
        return "missing".to_string();
    }
    if replay.items.iter().any(|item| item.status == "blocked") {
        return "blocked".to_string();
    }
    "matched".to_string()
}
