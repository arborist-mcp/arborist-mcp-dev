use std::path::Path;

use anyhow::{Result, bail};

use crate::model::*;
use crate::{language, patching};

use super::validate_patch_trace_validation_result;
use super::validate_trace_patch_evidence_replay_result;

pub(crate) fn validate_replay_patch_payload_with_deadline(
    patch: &PatchAstNodeResult,
    deadline: Option<&dyn crate::deadline::DeadlineCheck>,
) -> Result<()> {
    check_deadline(deadline, "validating patch payload")?;
    patch.validate_public_output()?;

    check_deadline(deadline, "parsing updated patch source")?;
    let document = language::parse_document(Path::new(&patch.file), &patch.updated_source)?;
    check_deadline(deadline, "validating patch syntax")?;
    let expected_syntax_errors = patching::collect_syntax_errors_with_deadline(
        document.tree.root_node(),
        &patch.updated_source,
        deadline,
    )?;
    if patch.validation.syntax_errors != expected_syntax_errors {
        bail!(
            "invalid patch.validation.syntax_errors: expected syntax errors derived from patch.updated_source"
        );
    }

    check_deadline(deadline, "validating patch commit gate")?;
    let expected_commit_gate = patching::evaluate_patch_commit_gate(
        &patch.validation,
        patch.validation.commit_gate.bypass_reason.as_deref(),
    );
    let commit_gate = &patch.validation.commit_gate;

    if commit_gate.status != expected_commit_gate.status {
        bail!(
            "invalid patch.validation.commit_gate.status: expected `{}` derived from patch.validation",
            expected_commit_gate.status
        );
    }
    if commit_gate.allowed != expected_commit_gate.allowed {
        bail!(
            "invalid patch.validation.commit_gate.allowed: expected {} derived from patch.validation",
            expected_commit_gate.allowed
        );
    }
    if commit_gate.reason != expected_commit_gate.reason {
        bail!(
            "invalid patch.validation.commit_gate.reason: expected reason derived from patch.validation"
        );
    }
    if commit_gate.bypass_reason != expected_commit_gate.bypass_reason {
        bail!(
            "invalid patch.validation.commit_gate.bypass_reason: expected bypass reason derived from patch.validation"
        );
    }
    if commit_gate.blocking_decisions != expected_commit_gate.blocking_decisions {
        bail!(
            "invalid patch.validation.commit_gate.blocking_decisions: expected blocking decisions derived from patch.validation.binding_decisions"
        );
    }
    if commit_gate.evidence_invariants != expected_commit_gate.evidence_invariants {
        bail!(
            "invalid patch.validation.commit_gate.evidence_invariants: expected evidence invariants derived from patch.validation.binding_decisions"
        );
    }
    if commit_gate.syntax_error_count != expected_commit_gate.syntax_error_count {
        bail!(
            "invalid patch.validation.commit_gate.syntax_error_count: expected syntax error count derived from patch.validation.syntax_errors"
        );
    }

    check_deadline(deadline, "finishing patch payload validation")?;
    Ok(())
}

pub(crate) fn validate_replay_trace_target(
    patch: &PatchAstNodeResult,
    trace: &TraceSymbolGraphResult,
) -> Result<()> {
    if trace.symbol.symbol_id != patch.resolved_symbol_id {
        bail!(
            "invalid trace.symbol.symbol_id: expected `{}` to match patch.resolved_symbol_id",
            patch.resolved_symbol_id
        );
    }
    if trace.symbol.semantic_path != patch.resolved_path {
        bail!(
            "invalid trace.symbol.semantic_path: expected `{}` to match patch.resolved_path",
            patch.resolved_path
        );
    }
    if trace.symbol.file_path != patch.file {
        bail!(
            "invalid trace.symbol.file_path: expected `{}` to match patch.file",
            patch.file
        );
    }

    Ok(())
}

pub fn replay_patch_evidence_against_trace(
    patch: &PatchAstNodeResult,
    trace: &TraceSymbolGraphResult,
) -> Result<TracePatchEvidenceReplayResult> {
    replay_patch_evidence_against_trace_inner(patch, trace, None)
}

pub fn replay_patch_evidence_against_trace_with_timeout(
    patch: &PatchAstNodeResult,
    trace: &TraceSymbolGraphResult,
    timeout_ms: Option<u64>,
) -> Result<TracePatchEvidenceReplayResult> {
    let deadline = super::patch_analysis_deadline(timeout_ms, "patch evidence replay")?;
    replay_patch_evidence_against_trace_inner(patch, trace, Some(&deadline))
}

#[cfg(test)]
pub(crate) fn replay_patch_evidence_against_trace_with_deadline(
    patch: &PatchAstNodeResult,
    trace: &TraceSymbolGraphResult,
    deadline: &dyn crate::deadline::DeadlineCheck,
) -> Result<TracePatchEvidenceReplayResult> {
    replay_patch_evidence_against_trace_inner(patch, trace, Some(deadline))
}

fn replay_patch_evidence_against_trace_inner(
    patch: &PatchAstNodeResult,
    trace: &TraceSymbolGraphResult,
    deadline: Option<&dyn crate::deadline::DeadlineCheck>,
) -> Result<TracePatchEvidenceReplayResult> {
    validate_replay_patch_payload_with_deadline(patch, deadline)?;
    check_deadline(deadline, "validating trace payload")?;
    trace.validate_public_output()?;
    check_deadline(deadline, "validating trace target")?;
    validate_replay_trace_target(patch, trace)?;

    let trace_callers = evidence_key_set(&trace.callers, deadline, "collecting trace callers")?;
    let trace_callees = evidence_key_set(&trace.callees, deadline, "collecting trace callees")?;
    let trace_symbol = trace.symbol.evidence_key.clone();
    let normalized_trace_callers =
        normalized_evidence_key_set(trace_callers.iter(), deadline, "normalizing trace callers")?;
    let normalized_trace_callees =
        normalized_evidence_key_set(trace_callees.iter(), deadline, "normalizing trace callees")?;
    let normalized_trace_symbol = evidence_key_without_origin_type(&trace_symbol);

    let mut items = Vec::with_capacity(patch.validation.commit_gate.evidence_invariants.len());
    let mut matched_items = 0usize;
    let mut blocked_items = 0usize;
    let mut consistent = true;
    for invariant in &patch.validation.commit_gate.evidence_invariants {
        check_deadline(deadline, "replaying patch evidence")?;
        let (matched_in_trace, trace_match_scope) = if let Some(selected) =
            &invariant.selected_evidence_key
        {
            if trace_callees.contains(selected) {
                (true, "callees".to_string())
            } else if trace_callers.contains(selected) {
                (true, "callers".to_string())
            } else if trace_symbol == *selected {
                (true, "symbol".to_string())
            } else if let Some(normalized_selected) = evidence_key_without_origin_type(selected) {
                if normalized_trace_callees.contains(&normalized_selected) {
                    (true, "callees".to_string())
                } else if normalized_trace_callers.contains(&normalized_selected) {
                    (true, "callers".to_string())
                } else if normalized_trace_symbol.as_ref() == Some(&normalized_selected) {
                    (true, "symbol".to_string())
                } else if is_patch_scope_evidence_key(selected) {
                    (true, "patch_scope".to_string())
                } else {
                    (false, "none".to_string())
                }
            } else if is_patch_scope_evidence_key(selected) {
                (true, "patch_scope".to_string())
            } else {
                (false, "none".to_string())
            }
        } else {
            (false, "none".to_string())
        };

        let status = match invariant.status.as_str() {
            "passed" if matched_in_trace => "matched",
            "passed" => "missing",
            "blocked" => "blocked",
            _ => "failed",
        }
        .to_string();
        matched_items += usize::from(status == "matched");
        blocked_items += usize::from(status == "blocked");
        consistent &= matches!(status.as_str(), "matched" | "blocked");

        items.push(TracePatchEvidenceReplayItem {
            name: invariant.name.clone(),
            status,
            selected_evidence_key: invariant.selected_evidence_key.clone(),
            matched_in_trace,
            trace_match_scope,
            candidate_evidence_keys: invariant.candidate_evidence_keys.clone(),
        });
    }

    let result = TracePatchEvidenceReplayResult {
        consistent,
        matched_items,
        blocked_items,
        items,
    };
    check_deadline(deadline, "validating patch evidence replay result")?;
    validate_trace_patch_evidence_replay_result(&result)?;
    check_deadline(deadline, "finishing patch evidence replay")?;
    Ok(result)
}

fn evidence_key_set(
    symbols: &[SymbolSummary],
    deadline: Option<&dyn crate::deadline::DeadlineCheck>,
    phase: &str,
) -> Result<std::collections::BTreeSet<String>> {
    let mut keys = std::collections::BTreeSet::new();
    for symbol in symbols {
        check_deadline(deadline, phase)?;
        keys.insert(symbol.evidence_key.clone());
    }
    Ok(keys)
}

fn normalized_evidence_key_set<'a>(
    keys: impl Iterator<Item = &'a String>,
    deadline: Option<&dyn crate::deadline::DeadlineCheck>,
    phase: &str,
) -> Result<std::collections::BTreeSet<String>> {
    let mut normalized = std::collections::BTreeSet::new();
    for key in keys {
        check_deadline(deadline, phase)?;
        if let Some(key) = evidence_key_without_origin_type(key) {
            normalized.insert(key);
        }
    }
    Ok(normalized)
}

fn evidence_key_without_origin_type(evidence_key: &str) -> Option<String> {
    let parts = evidence_key.splitn(6, '|').collect::<Vec<_>>();
    if parts.len() != 6 {
        return None;
    }

    Some(format!(
        "{}|{}|{}|{}|{}",
        parts[0], parts[1], parts[2], parts[4], parts[5]
    ))
}

fn is_patch_scope_evidence_key(evidence_key: &str) -> bool {
    matches!(
        evidence_key.split('|').nth(3),
        Some("local_scope" | "module_scope")
    )
}

pub fn validate_patch_commit_with_trace(
    patch: &PatchAstNodeResult,
    trace: &TraceSymbolGraphResult,
) -> Result<PatchTraceValidationResult> {
    validate_patch_commit_with_trace_inner(patch, trace, None)
}

pub fn validate_patch_commit_with_trace_with_timeout(
    patch: &PatchAstNodeResult,
    trace: &TraceSymbolGraphResult,
    timeout_ms: Option<u64>,
) -> Result<PatchTraceValidationResult> {
    let deadline = super::patch_analysis_deadline(timeout_ms, "patch trace validation")?;
    validate_patch_commit_with_trace_inner(patch, trace, Some(&deadline))
}

#[cfg(test)]
pub(crate) fn validate_patch_commit_with_trace_with_deadline(
    patch: &PatchAstNodeResult,
    trace: &TraceSymbolGraphResult,
    deadline: &dyn crate::deadline::DeadlineCheck,
) -> Result<PatchTraceValidationResult> {
    validate_patch_commit_with_trace_inner(patch, trace, Some(deadline))
}

fn validate_patch_commit_with_trace_inner(
    patch: &PatchAstNodeResult,
    trace: &TraceSymbolGraphResult,
    deadline: Option<&dyn crate::deadline::DeadlineCheck>,
) -> Result<PatchTraceValidationResult> {
    let replay = replay_patch_evidence_against_trace_inner(patch, trace, deadline)?;
    check_deadline(deadline, "building patch trace validation result")?;
    let result = build_patch_trace_validation_result(patch, replay, deadline)?;
    check_deadline(deadline, "validating patch trace validation result")?;
    validate_patch_trace_validation_result(&result)?;
    check_deadline(deadline, "finishing patch trace validation")?;
    Ok(result)
}

fn summarize_replay_status(
    replay: &TracePatchEvidenceReplayResult,
    deadline: Option<&dyn crate::deadline::DeadlineCheck>,
) -> Result<String> {
    let mut missing = false;
    let mut blocked = false;
    for item in &replay.items {
        check_deadline(deadline, "summarizing patch evidence replay")?;
        match item.status.as_str() {
            "failed" => return Ok("failed".to_string()),
            "missing" => missing = true,
            "blocked" => blocked = true,
            _ => {}
        }
    }
    if missing {
        return Ok("missing".to_string());
    }
    if blocked {
        return Ok("blocked".to_string());
    }
    Ok("matched".to_string())
}

fn build_patch_trace_validation_result(
    patch: &PatchAstNodeResult,
    replay: TracePatchEvidenceReplayResult,
    deadline: Option<&dyn crate::deadline::DeadlineCheck>,
) -> Result<PatchTraceValidationResult> {
    let replay_status = summarize_replay_status(&replay, deadline)?;
    let patch_gate_status = patch.validation.commit_gate.status.clone();

    if !patch.validation.commit_gate.allowed {
        return Ok(PatchTraceValidationResult {
            allowed: false,
            status: "rejected_by_patch_gate".to_string(),
            reason: patch.validation.commit_gate.reason.clone(),
            patch_gate_status,
            replay_status,
            replay,
        });
    }

    if matches!(replay_status.as_str(), "missing" | "failed") {
        return Ok(PatchTraceValidationResult {
            allowed: false,
            status: "rejected_by_trace_replay".to_string(),
            reason: "trace replay did not confirm the patch evidence".to_string(),
            patch_gate_status,
            replay_status,
            replay,
        });
    }

    if replay_status == "blocked" && patch_gate_status != "allowed_with_bypass" {
        return Ok(PatchTraceValidationResult {
            allowed: false,
            status: "rejected_by_trace_replay".to_string(),
            reason: "trace replay found blocked evidence without an explicit bypass".to_string(),
            patch_gate_status,
            replay_status,
            replay,
        });
    }

    let (status, reason) = if patch.validation.commit_gate.status == "allowed_with_bypass" {
        (
            "allowed_with_bypass".to_string(),
            "patch gate allowed the write with bypass and trace replay did not contradict the evidence".to_string(),
        )
    } else {
        (
            "allowed".to_string(),
            "patch gate and trace replay both accepted the evidence".to_string(),
        )
    };

    Ok(PatchTraceValidationResult {
        allowed: true,
        status,
        reason,
        patch_gate_status,
        replay_status,
        replay,
    })
}

fn check_deadline(
    deadline: Option<&dyn crate::deadline::DeadlineCheck>,
    phase: &str,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(())
}
