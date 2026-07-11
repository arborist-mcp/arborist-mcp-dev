use crate::model::{
    PatchCommitGateReport, PatchEvidenceInvariantReport, PatchValidationReport,
    ValidationBindingDecision,
};

pub(crate) fn evaluate_patch_commit_gate(
    validation: &PatchValidationReport,
    bypass_reason: Option<&str>,
) -> PatchCommitGateReport {
    let blocking_decisions = validation
        .binding_decisions
        .iter()
        .filter(|decision| decision.status != "resolved")
        .cloned()
        .collect::<Vec<_>>();
    let evidence_invariants = validation
        .binding_decisions
        .iter()
        .map(evaluate_binding_evidence_invariant)
        .collect::<Vec<_>>();
    let has_evidence_failure = evidence_invariants
        .iter()
        .any(|invariant| invariant.status == "failed");
    let bypass_reason = bypass_reason
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(str::to_string);

    if validation.syntax_errors.is_empty() && blocking_decisions.is_empty() && !has_evidence_failure
    {
        return PatchCommitGateReport {
            status: "allowed".to_string(),
            allowed: true,
            reason: "syntax and symbol binding validation passed".to_string(),
            bypass_reason: None,
            blocking_decisions,
            evidence_invariants,
            syntax_error_count: 0,
        };
    }

    if let Some(bypass_reason) = bypass_reason {
        return PatchCommitGateReport {
            status: "allowed_with_bypass".to_string(),
            allowed: true,
            reason:
                "validation reported blocking evidence, but an explicit bypass reason was provided"
                    .to_string(),
            bypass_reason: Some(bypass_reason),
            blocking_decisions,
            evidence_invariants,
            syntax_error_count: validation.syntax_errors.len(),
        };
    }

    PatchCommitGateReport {
        status: "rejected".to_string(),
        allowed: false,
        reason: rejected_patch_reason(validation, &blocking_decisions),
        bypass_reason: None,
        blocking_decisions,
        evidence_invariants,
        syntax_error_count: validation.syntax_errors.len(),
    }
}

fn evaluate_binding_evidence_invariant(
    decision: &ValidationBindingDecision,
) -> PatchEvidenceInvariantReport {
    let candidate_evidence_keys = decision
        .candidates
        .iter()
        .map(|candidate| candidate.evidence_key.clone())
        .collect::<Vec<_>>();

    match decision.status.as_str() {
        "resolved" => resolved_evidence_invariant(decision, candidate_evidence_keys),
        "ambiguous" => PatchEvidenceInvariantReport {
            name: decision.name.clone(),
            status: "blocked".to_string(),
            reason: "multiple candidate evidence keys remain visible".to_string(),
            selected_evidence_key: None,
            candidate_evidence_keys,
        },
        "unresolved" => PatchEvidenceInvariantReport {
            name: decision.name.clone(),
            status: "blocked".to_string(),
            reason: "no candidate evidence key is available for this binding".to_string(),
            selected_evidence_key: None,
            candidate_evidence_keys,
        },
        _ => PatchEvidenceInvariantReport {
            name: decision.name.clone(),
            status: "failed".to_string(),
            reason: format!("unknown binding decision status: {}", decision.status),
            selected_evidence_key: None,
            candidate_evidence_keys,
        },
    }
}

fn resolved_evidence_invariant(
    decision: &ValidationBindingDecision,
    candidate_evidence_keys: Vec<String>,
) -> PatchEvidenceInvariantReport {
    let selected_candidate = decision.selected_symbol_id.as_ref().and_then(|symbol_id| {
        decision
            .candidates
            .iter()
            .find(|candidate| &candidate.symbol_id == symbol_id)
    });
    let selected_evidence_key = selected_candidate.map(|candidate| candidate.evidence_key.clone());

    if decision.candidates.len() != 1 {
        return PatchEvidenceInvariantReport {
            name: decision.name.clone(),
            status: "failed".to_string(),
            reason: "resolved binding must have exactly one candidate".to_string(),
            selected_evidence_key,
            candidate_evidence_keys,
        };
    }

    if selected_evidence_key
        .as_ref()
        .is_none_or(|evidence_key| evidence_key.is_empty())
    {
        return PatchEvidenceInvariantReport {
            name: decision.name.clone(),
            status: "failed".to_string(),
            reason: "resolved binding is missing selected evidence key".to_string(),
            selected_evidence_key,
            candidate_evidence_keys,
        };
    }

    PatchEvidenceInvariantReport {
        name: decision.name.clone(),
        status: "passed".to_string(),
        reason: "resolved binding has one selected candidate evidence key".to_string(),
        selected_evidence_key,
        candidate_evidence_keys,
    }
}

fn rejected_patch_reason(
    validation: &PatchValidationReport,
    blocking_decisions: &[ValidationBindingDecision],
) -> String {
    if !validation.syntax_errors.is_empty() {
        return "syntax validation failed".to_string();
    }

    if blocking_decisions
        .iter()
        .any(|decision| decision.status == "ambiguous")
    {
        return "symbol binding is ambiguous".to_string();
    }

    if blocking_decisions
        .iter()
        .any(|decision| decision.status == "unresolved")
    {
        return "symbol binding is unresolved".to_string();
    }

    if validation
        .binding_decisions
        .iter()
        .any(|decision| decision.status == "resolved" && decision.candidates.len() != 1)
    {
        return "symbol evidence invariant failed".to_string();
    }

    "patch validation failed".to_string()
}
