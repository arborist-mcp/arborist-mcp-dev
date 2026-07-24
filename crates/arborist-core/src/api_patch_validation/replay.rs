use std::path::Path;

use anyhow::{Result, bail};

use crate::model::*;
use crate::{language, patching};

use super::validate_patch_trace_validation_result;
use super::validate_trace_patch_evidence_replay_result;

pub(crate) fn validate_replay_patch_payload(patch: &PatchAstNodeResult) -> Result<()> {
    patch.validate_public_output()?;

    let document = language::parse_document(Path::new(&patch.file), &patch.updated_source)?;
    let expected_syntax_errors =
        patching::collect_syntax_errors(document.tree.root_node(), &patch.updated_source);
    if patch.validation.syntax_errors != expected_syntax_errors {
        bail!(
            "invalid patch.validation.syntax_errors: expected syntax errors derived from patch.updated_source"
        );
    }

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
    validate_replay_patch_payload(patch)?;
    trace.validate_public_output()?;
    validate_replay_trace_target(patch, trace)?;

    let trace_callers = trace
        .callers
        .iter()
        .map(|symbol| symbol.evidence_key.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let trace_callees = trace
        .callees
        .iter()
        .map(|symbol| symbol.evidence_key.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let trace_symbol = trace.symbol.evidence_key.clone();
    let normalized_trace_callers = normalized_evidence_key_set(trace_callers.iter());
    let normalized_trace_callees = normalized_evidence_key_set(trace_callees.iter());
    let normalized_trace_symbol = evidence_key_without_origin_type(&trace_symbol);

    let items = patch
        .validation
        .commit_gate
        .evidence_invariants
        .iter()
        .map(|invariant| {
            let (matched_in_trace, trace_match_scope) = if let Some(selected) =
                &invariant.selected_evidence_key
            {
                if trace_callees.contains(selected) {
                    (true, "callees".to_string())
                } else if trace_callers.contains(selected) {
                    (true, "callers".to_string())
                } else if trace_symbol == *selected {
                    (true, "symbol".to_string())
                } else if let Some(normalized_selected) = evidence_key_without_origin_type(selected)
                {
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

            TracePatchEvidenceReplayItem {
                name: invariant.name.clone(),
                status,
                selected_evidence_key: invariant.selected_evidence_key.clone(),
                matched_in_trace,
                trace_match_scope,
                candidate_evidence_keys: invariant.candidate_evidence_keys.clone(),
            }
        })
        .collect::<Vec<_>>();

    let matched_items = items.iter().filter(|item| item.status == "matched").count();
    let blocked_items = items.iter().filter(|item| item.status == "blocked").count();
    let consistent = items
        .iter()
        .all(|item| matches!(item.status.as_str(), "matched" | "blocked"));

    let result = TracePatchEvidenceReplayResult {
        consistent,
        matched_items,
        blocked_items,
        items,
    };
    validate_trace_patch_evidence_replay_result(&result)?;
    Ok(result)
}

fn normalized_evidence_key_set<'a>(
    keys: impl Iterator<Item = &'a String>,
) -> std::collections::BTreeSet<String> {
    keys.filter_map(|key| evidence_key_without_origin_type(key))
        .collect()
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
    let replay = replay_patch_evidence_against_trace(patch, trace)?;
    let result = build_patch_trace_validation_result(patch, replay);
    validate_patch_trace_validation_result(&result)?;
    Ok(result)
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

fn build_patch_trace_validation_result(
    patch: &PatchAstNodeResult,
    replay: TracePatchEvidenceReplayResult,
) -> PatchTraceValidationResult {
    let replay_status = summarize_replay_status(&replay);
    let patch_gate_status = patch.validation.commit_gate.status.clone();

    if !patch.validation.commit_gate.allowed {
        return PatchTraceValidationResult {
            allowed: false,
            status: "rejected_by_patch_gate".to_string(),
            reason: patch.validation.commit_gate.reason.clone(),
            patch_gate_status,
            replay_status,
            replay,
        };
    }

    if matches!(replay_status.as_str(), "missing" | "failed") {
        return PatchTraceValidationResult {
            allowed: false,
            status: "rejected_by_trace_replay".to_string(),
            reason: "trace replay did not confirm the patch evidence".to_string(),
            patch_gate_status,
            replay_status,
            replay,
        };
    }

    if replay_status == "blocked" && patch_gate_status != "allowed_with_bypass" {
        return PatchTraceValidationResult {
            allowed: false,
            status: "rejected_by_trace_replay".to_string(),
            reason: "trace replay found blocked evidence without an explicit bypass".to_string(),
            patch_gate_status,
            replay_status,
            replay,
        };
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

    PatchTraceValidationResult {
        allowed: true,
        status,
        reason,
        patch_gate_status,
        replay_status,
        replay,
    }
}
