use std::collections::BTreeSet;

use anyhow::{Result, bail};

use super::super::{
    DisambiguationContext, PatchCommitGateReport, PatchEvidenceInvariantReport,
    PatchValidationReport, ValidationAmbiguity, ValidationBinding, ValidationBindingDecision,
    ValidationIssue, ensure_nonblank, ensure_nonblank_strings, ensure_unique_strings,
    ensure_unique_symbol_evidence_keys, point_is_after,
};

impl PatchCommitGateReport {
    pub(crate) fn validate_trace_replay_input(
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
    pub(crate) fn validate_trace_replay_input(&self) -> Result<()> {
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
    pub(crate) fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
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
    pub(crate) fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
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
