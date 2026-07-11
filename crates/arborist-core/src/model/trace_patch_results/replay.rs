use anyhow::{Result, bail};

use super::super::{ensure_nonblank, ensure_nonblank_strings, ensure_unique_strings};
use super::{
    PatchTraceValidationResult, TracePatchEvidenceReplayItem, TracePatchEvidenceReplayResult,
};

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
