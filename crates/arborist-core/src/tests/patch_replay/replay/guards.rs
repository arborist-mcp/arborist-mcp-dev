use super::*;

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

    let export_error = export_patch_diagnostics_sarif(&patch)
        .expect_err("SARIF export must reject tampered syntax diagnostics");
    assert!(
        export_error
            .to_string()
            .contains("patch.validation.syntax_errors")
    );

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
