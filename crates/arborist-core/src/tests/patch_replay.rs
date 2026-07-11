use std::fs;

use super::support::temporary_dir;
use super::{
    TraceDirection, patch_ast_node_from_path, replay_patch_evidence_against_trace,
    trace_symbol_graph, validate_patch_commit_with_trace, validate_patch_trace_validation_result,
    validate_patch_with_discovery_context, validate_patch_with_discovery_context_from_path,
    validate_patch_with_graph_context, validate_patch_with_graph_context_from_path,
    validate_patch_with_neighborhood_context, validate_patch_with_neighborhood_context_from_path,
    validate_patch_with_trace_context, validate_patch_with_trace_context_from_path,
    validate_trace_backed_patch_result, validate_trace_patch_evidence_replay_result,
};
#[test]
fn keeps_blocked_replay_items_for_ambiguous_patch_evidence() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let caller = dir.join("caller.c");

    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "#include \"alpha.h\"\n#include \"zeta.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    let replay = replay_patch_evidence_against_trace(&patch, &trace).unwrap();

    assert!(replay.consistent);
    assert_eq!(replay.matched_items, 0);
    assert_eq!(replay.blocked_items, 1);
    assert_eq!(replay.items.len(), 1);
    assert_eq!(replay.items[0].status, "blocked");
    assert!(!replay.items[0].matched_in_trace);
    assert_eq!(replay.items[0].trace_match_scope, "none");
    assert_eq!(replay.items[0].candidate_evidence_keys.len(), 2);
}

#[test]
fn trace_validation_requires_bypass_for_blocked_replay_items() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let caller = dir.join("caller.c");

    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "#include \"alpha.h\"\n#include \"zeta.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        Some("human selected the include family"),
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    let bypass_decision = validate_patch_commit_with_trace(&patch, &trace).unwrap();
    assert!(bypass_decision.allowed);
    assert_eq!(bypass_decision.status, "allowed_with_bypass");
    assert_eq!(bypass_decision.replay_status, "blocked");

    let mut inconsistent_patch = patch.clone();
    inconsistent_patch.validation.commit_gate.status = "allowed".to_string();
    inconsistent_patch.validation.commit_gate.bypass_reason = None;
    inconsistent_patch.bypass_applied = false;

    let rejected = validate_patch_commit_with_trace(&inconsistent_patch, &trace)
        .expect_err("tampered blocked replay payloads should be rejected");
    assert!(rejected.to_string().contains("commit_gate.status"));
}

#[test]
fn allows_trace_validated_patch_commit_when_replay_matches() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    let decision = validate_patch_commit_with_trace(&patch, &trace).unwrap();

    assert!(decision.allowed);
    assert_eq!(decision.status, "allowed");
    assert_eq!(decision.patch_gate_status, "allowed");
    assert_eq!(decision.replay_status, "matched");
    assert!(decision.replay.consistent);
}

#[test]
fn rejects_tampered_trace_validation_replay_status() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    let mut decision = validate_patch_commit_with_trace(&patch, &trace).unwrap();
    decision.replay_status = "blocked".to_string();

    let error = validate_patch_trace_validation_result(&decision)
        .expect_err("tampered replay_status should be rejected");
    assert!(error.to_string().contains("trace_validation.replay_status"));
}

#[test]
fn rejects_trace_validated_patch_commit_when_replay_is_missing() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Callers).unwrap();
    let decision = validate_patch_commit_with_trace(&patch, &trace).unwrap();

    assert!(!decision.allowed);
    assert_eq!(decision.status, "rejected_by_trace_replay");
    assert_eq!(decision.patch_gate_status, "allowed");
    assert_eq!(decision.replay_status, "missing");
    assert!(!decision.replay.consistent);
}

#[test]
fn rejects_tampered_trace_evidence_key_summaries_during_replay() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let selected_evidence_key = patch.validation.commit_gate.evidence_invariants[0]
        .selected_evidence_key
        .clone()
        .expect("resolved patch evidence should have a selected key");
    let mut trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Callers).unwrap();
    assert!(trace.callees.is_empty());
    trace.evidence_keys.callees.push(selected_evidence_key);

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("tampered trace evidence summaries should be rejected");
    assert!(replay.to_string().contains("trace.evidence_keys.callees"));

    let decision = validate_patch_commit_with_trace(&patch, &trace)
        .expect_err("tampered trace evidence summaries should be rejected");
    assert!(decision.to_string().contains("trace.evidence_keys.callees"));
}

#[test]
fn rejects_mismatched_trace_root_during_replay() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "helper", TraceDirection::Both).unwrap();

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("mismatched trace roots should be rejected");
    assert!(replay.to_string().contains("trace.symbol.symbol_id"));

    let decision = validate_patch_commit_with_trace(&patch, &trace)
        .expect_err("mismatched trace roots should be rejected");
    assert!(decision.to_string().contains("trace.symbol.symbol_id"));
}

#[test]
fn rejects_mismatched_trace_root_file_during_replay() {
    let dir = temporary_dir();
    let caller = dir.join("caller.py");

    fs::write(
        &caller,
        "def top_level(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 2\n",
        None,
    )
    .unwrap();
    let mut trace = trace_symbol_graph(&dir, "top_level", TraceDirection::Both).unwrap();
    trace.symbol.file_path = dir.join("other.py").to_string_lossy().replace('\\', "/");
    trace.symbol.evidence_key = format!(
        "{}|{}|{}|{}|{}..{}|{}",
        trace.symbol.symbol_id,
        trace.symbol.file_path,
        trace.symbol.node_kind,
        trace.symbol.origin_type,
        trace.symbol.byte_range.0,
        trace.symbol.byte_range.1,
        trace.symbol.signature.as_deref().unwrap_or("")
    );
    trace.evidence_keys.symbol = trace.symbol.evidence_key.clone();

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("mismatched trace root files should be rejected");
    assert!(replay.to_string().contains("trace.symbol.file_path"));
}

#[test]
fn rejects_blank_patch_replay_selected_evidence_keys() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let mut patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    patch.validation.commit_gate.evidence_invariants[0].selected_evidence_key =
        Some("   ".to_string());

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("blank selected evidence keys should be rejected");
    assert!(replay.to_string().contains("selected_evidence_key"));
}

#[test]
fn rejects_tampered_syntax_error_details_during_replay() {
    let dir = temporary_dir();
    let caller = dir.join("caller.c");

    fs::write(
        &caller,
        "int orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let mut patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return value + 1\n}\n",
        None,
    )
    .unwrap();
    assert!(!patch.applied);
    assert_eq!(patch.validation.syntax_errors.len(), 1);

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    patch.validation.syntax_errors[0].message = "manually tampered".to_string();

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("tampered syntax error details should be rejected");
    assert!(
        replay
            .to_string()
            .contains("patch.validation.syntax_errors")
    );
}

#[test]
fn rejects_blank_patch_updated_source_during_replay() {
    let dir = temporary_dir();
    let caller = dir.join("caller.py");

    fs::write(
        &caller,
        "def top_level(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let mut patch = patch_ast_node_from_path(
        &caller,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 2\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "top_level", TraceDirection::Both).unwrap();
    patch.updated_source = "   ".to_string();

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("blank patch sources should be rejected before replay");
    assert!(replay.to_string().contains("patch.updated_source"));
}

#[test]
fn rejects_duplicate_trace_evidence_entries_during_replay() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let mut trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    trace.callees.push(trace.callees[0].clone());
    trace
        .evidence_keys
        .callees
        .push(trace.evidence_keys.callees[0].clone());

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("duplicate trace evidence entries should be rejected");
    assert!(replay.to_string().contains("trace.callees[1].evidence_key"));
}

#[test]
fn rejects_tampered_replay_match_counts() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    let mut replay = replay_patch_evidence_against_trace(&patch, &trace).unwrap();
    assert!(replay.matched_items > 0);
    replay.matched_items = 0;

    let error = validate_trace_patch_evidence_replay_result(&replay)
        .expect_err("tampered replay match counts should be rejected");
    assert!(error.to_string().contains("matched_items"));
}

#[test]
fn rejects_non_root_trace_symbol_origin_type_during_replay() {
    let dir = temporary_dir();
    let caller = dir.join("caller.py");

    fs::write(
        &caller,
        "def top_level(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 2\n",
        None,
    )
    .unwrap();
    let mut trace = trace_symbol_graph(&dir, "top_level", TraceDirection::Both).unwrap();
    trace.symbol.origin_type = "callee".to_string();
    trace.symbol.evidence_key = format!(
        "{}|{}|{}|{}|{}..{}|{}",
        trace.symbol.symbol_id,
        trace.symbol.file_path,
        trace.symbol.node_kind,
        trace.symbol.origin_type,
        trace.symbol.byte_range.0,
        trace.symbol.byte_range.1,
        trace.symbol.signature.as_deref().unwrap_or("")
    );
    trace.evidence_keys.symbol = trace.symbol.evidence_key.clone();

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("non-root trace symbol origin types should be rejected");
    assert!(replay.to_string().contains("trace.symbol.origin_type"));
}

#[test]
fn rejects_tampered_resolved_identifier_summary_during_replay() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let mut patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    patch.validation.resolved_identifiers.clear();

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("tampered resolved identifier summaries should be rejected");
    assert!(replay.to_string().contains("resolved_identifiers"));
}

#[test]
fn rejects_unsupported_binding_decision_status_during_replay() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let mut patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    patch.validation.binding_decisions[0].status = "mystery".to_string();

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("unsupported binding decision statuses should be rejected");
    assert!(replay.to_string().contains("binding_decisions[0].status"));
}

#[test]
fn rejects_resolved_binding_decision_without_selected_symbol_id_during_replay() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let mut patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    patch.validation.binding_decisions[0].selected_symbol_id = None;

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("resolved binding decisions must retain their selected symbol id");
    assert!(
        replay
            .to_string()
            .contains("binding_decisions[0].selected_symbol_id")
    );
}

#[test]
fn rejects_inconsistent_patch_gate_status_during_replay() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let mut patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    patch.validation.commit_gate.status = "allowed_with_bypass".to_string();
    patch.bypass_applied = true;

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("inconsistent patch gate status should be rejected");
    assert!(replay.to_string().contains("bypass_reason"));
}

#[test]
fn rejects_inconsistent_patch_applied_flags_during_replay() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let mut patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    patch.applied = false;

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("inconsistent patch applied flag should be rejected");
    assert!(replay.to_string().contains("patch.applied"));
}

#[test]
fn rejects_tampered_patch_commit_gate_reason_during_replay() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let mut patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    patch.validation.commit_gate.reason = "manually overridden".to_string();

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("tampered patch gate reasons should be rejected");
    assert!(replay.to_string().contains("commit_gate.reason"));
}

#[test]
fn rejects_tampered_patch_commit_gate_invariants_during_replay() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let mut patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    patch.validation.commit_gate.evidence_invariants[0].status = "blocked".to_string();

    let replay = replay_patch_evidence_against_trace(&patch, &trace)
        .expect_err("tampered patch gate evidence invariants should be rejected");
    assert!(
        replay
            .to_string()
            .contains("commit_gate.evidence_invariants")
    );
}

#[test]
fn validates_patch_with_trace_context_in_one_call() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = validate_patch_with_trace_context_from_path(
        &dir,
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
        TraceDirection::Both,
    )
    .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.trace_target, result.patch.resolved_symbol_id);
    assert!(result.trace.is_some());
    assert!(result.trace_validation.is_some());
    assert!(result.trace_error.is_none());
    assert!(
        result
            .trace_validation
            .as_ref()
            .is_some_and(|decision| decision.allowed)
    );
}

#[test]
fn validates_trace_context_with_unsaved_source_file() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let result = validate_patch_with_trace_context(
            &dir,
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
            "orchestrate",
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
            None,
            TraceDirection::Both,
        )
        .unwrap();

    assert!(result.patch.applied);
    assert!(result.trace_error.is_none());
    let trace = result.trace.as_ref().expect("trace should be available");
    assert_eq!(trace.symbol.semantic_path, "orchestrate");
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    assert!(
        result
            .trace_validation
            .as_ref()
            .is_some_and(|decision| decision.allowed)
    );
    assert!(!caller.exists());
}

#[test]
fn keeps_trace_error_when_context_patch_has_syntax_errors() {
    let dir = temporary_dir();
    let caller = dir.join("caller.c");

    fs::write(
        &caller,
        "int orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = validate_patch_with_trace_context_from_path(
        &dir,
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value)\n",
        None,
        TraceDirection::Both,
    )
    .unwrap();

    assert!(!result.patch.applied);
    assert!(result.trace.is_none());
    assert!(result.trace_validation.is_none());
    assert_eq!(
        result.trace_error.as_deref(),
        Some("trace skipped because patch validation reported syntax errors")
    );
}

#[test]
fn validates_patch_with_graph_context_in_one_call() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let entry = dir.join("entry.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from caller import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let result = validate_patch_with_graph_context_from_path(
        &dir,
        &caller,
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        None,
        TraceDirection::Both,
        2,
        10,
    )
    .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.trace_target, result.patch.resolved_symbol_id);
    assert!(result.trace.is_some());
    assert!(result.neighborhood.is_some());
    assert!(result.trace_validation.is_some());
    assert!(result.trace_error.is_none());
    assert!(
        result
            .trace_validation
            .as_ref()
            .is_some_and(|decision| decision.allowed)
    );
    let neighborhood = result
        .neighborhood
        .as_ref()
        .expect("neighborhood should be available");
    assert_eq!(neighborhood.symbol.semantic_path, "orchestrate");
    assert!(
        neighborhood
            .nodes
            .iter()
            .any(|node| node.symbol.semantic_path == "helper")
    );
    assert!(
        neighborhood
            .nodes
            .iter()
            .any(|node| node.symbol.semantic_path == "entrypoint")
    );
}

#[test]
fn graph_context_accepts_unsaved_source_and_keeps_skip_reason() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let success = validate_patch_with_graph_context(
            &dir,
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
            "orchestrate",
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
            None,
            TraceDirection::Both,
            2,
            10,
        )
        .unwrap();

    assert!(success.patch.applied);
    assert!(success.trace.is_some());
    assert!(success.neighborhood.is_some());
    assert!(success.trace_error.is_none());
    assert!(!caller.exists());

    let rejected = validate_patch_with_graph_context(
        &dir,
        &caller,
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    return missing_helper(value)\n",
        None,
        TraceDirection::Both,
        2,
        10,
    )
    .unwrap();

    assert!(!rejected.patch.applied);
    assert!(rejected.trace.is_none());
    assert!(rejected.neighborhood.is_none());
    assert!(rejected.trace_validation.is_none());
    assert_eq!(
        rejected.trace_error.as_deref(),
        Some("trace skipped because patch validation rejected the patch")
    );
}

#[test]
fn validates_patch_with_neighborhood_context_in_one_call() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let entry = dir.join("entry.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from caller import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let result = validate_patch_with_neighborhood_context_from_path(
        &dir,
        &caller,
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        None,
        TraceDirection::Both,
        2,
        10,
    )
    .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.trace_target, result.patch.resolved_symbol_id);
    assert!(result.trace.is_some());
    assert!(result.neighborhood_context.is_some());
    assert!(result.trace_validation.is_some());
    assert!(result.trace_error.is_none());
    assert!(
        result
            .trace_validation
            .as_ref()
            .is_some_and(|decision| decision.allowed)
    );
    let neighborhood_context = result
        .neighborhood_context
        .as_ref()
        .expect("neighborhood context should be available");
    assert_eq!(
        neighborhood_context.neighborhood.symbol.semantic_path,
        "orchestrate"
    );
    assert_eq!(
        neighborhood_context.reads.len(),
        neighborhood_context.neighborhood.nodes.len()
    );
    assert!(
        neighborhood_context
            .reads
            .iter()
            .any(|read| read.symbol.semantic_path == "helper")
    );
    assert!(
        neighborhood_context
            .reads
            .iter()
            .any(|read| read.symbol.semantic_path == "entrypoint")
    );
}

#[test]
fn neighborhood_context_accepts_unsaved_source_and_keeps_skip_reason() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let success = validate_patch_with_neighborhood_context(
            &dir,
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
            "orchestrate",
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
            None,
            TraceDirection::Both,
            2,
            10,
        )
        .unwrap();

    assert!(success.patch.applied);
    assert!(success.trace.is_some());
    assert!(success.neighborhood_context.is_some());
    assert!(success.trace_error.is_none());
    assert!(!caller.exists());

    let rejected = validate_patch_with_neighborhood_context(
        &dir,
        &caller,
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    return missing_helper(value)\n",
        None,
        TraceDirection::Both,
        2,
        10,
    )
    .unwrap();

    assert!(!rejected.patch.applied);
    assert!(rejected.trace.is_none());
    assert!(rejected.neighborhood_context.is_none());
    assert!(rejected.trace_validation.is_none());
    assert_eq!(
        rejected.trace_error.as_deref(),
        Some("trace skipped because patch validation rejected the patch")
    );
}

#[test]
fn validates_patch_with_discovery_context_in_one_call() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let entry = dir.join("entry.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from caller import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let result = validate_patch_with_discovery_context_from_path(
        &dir,
        &caller,
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        None,
        TraceDirection::Both,
        2,
        10,
    )
    .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.trace_target, result.patch.resolved_symbol_id);
    assert!(result.trace.is_some());
    assert!(result.read.is_some());
    assert!(result.neighborhood_context.is_some());
    assert!(result.trace_validation.is_some());
    assert!(result.trace_error.is_none());
    assert!(
        result
            .trace_validation
            .as_ref()
            .is_some_and(|decision| decision.allowed)
    );
    let read = result.read.as_ref().expect("read should be available");
    assert_eq!(read.symbol.semantic_path, "orchestrate");
    assert!(read.source.contains("helper(value)"));
    let neighborhood_context = result
        .neighborhood_context
        .as_ref()
        .expect("neighborhood context should be available");
    assert_eq!(
        neighborhood_context.neighborhood.symbol.semantic_path,
        "orchestrate"
    );
    assert!(
        neighborhood_context
            .reads
            .iter()
            .any(|node_read| node_read.symbol.semantic_path == "helper")
    );
    assert!(
        neighborhood_context
            .reads
            .iter()
            .any(|node_read| node_read.symbol.semantic_path == "entrypoint")
    );
}

#[test]
fn discovery_context_accepts_unsaved_source_and_keeps_skip_reason() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let success = validate_patch_with_discovery_context(
            &dir,
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
            "orchestrate",
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
            None,
            TraceDirection::Both,
            2,
            10,
        )
        .unwrap();

    assert!(success.patch.applied);
    assert!(success.trace.is_some());
    assert!(success.read.is_some());
    assert!(success.neighborhood_context.is_some());
    assert!(success.trace_error.is_none());
    assert!(!caller.exists());

    let rejected = validate_patch_with_discovery_context(
        &dir,
        &caller,
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    return missing_helper(value)\n",
        None,
        TraceDirection::Both,
        2,
        10,
    )
    .unwrap();

    assert!(!rejected.patch.applied);
    assert!(rejected.trace.is_none());
    assert!(rejected.read.is_none());
    assert!(rejected.neighborhood_context.is_none());
    assert!(rejected.trace_validation.is_none());
    assert_eq!(
        rejected.trace_error.as_deref(),
        Some("trace skipped because patch validation rejected the patch")
    );
}

#[test]
fn rejects_tampered_trace_context_result_target_mismatch() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let mut result = validate_patch_with_trace_context_from_path(
        &dir,
        &caller,
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        None,
        TraceDirection::Both,
    )
    .unwrap();
    result.trace_target = "helper".to_string();

    let error = validate_trace_backed_patch_result(&result)
        .expect_err("tampered trace context targets should be rejected");
    assert!(error.to_string().contains("trace_target"));
}

#[test]
fn skips_trace_when_context_patch_is_rejected_by_patch_gate() {
    let dir = temporary_dir();
    let caller = dir.join("caller.py");

    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let result = validate_patch_with_trace_context_from_path(
        &dir,
        &caller,
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    return missing_helper(value)\n",
        None,
        TraceDirection::Both,
    )
    .unwrap();

    assert!(!result.patch.applied);
    assert_eq!(result.patch.validation.commit_gate.status, "rejected");
    assert!(result.trace.is_none());
    assert!(result.trace_validation.is_none());
    assert_eq!(
        result.trace_error.as_deref(),
        Some("trace skipped because patch validation rejected the patch")
    );
}
