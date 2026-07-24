use super::*;

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
fn exports_patch_validation_diagnostics_as_sarif() {
    let dir = temporary_dir();
    let caller = dir.join("caller.c");
    fs::write(
        &caller,
        "int orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return value + 1\n}\n",
        None,
    )
    .unwrap();
    let sarif = export_patch_diagnostics_sarif(&patch).unwrap();

    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(sarif["runs"][0]["columnKind"], "utf8CodeUnits");
    let results = sarif["runs"][0]["results"]
        .as_array()
        .expect("SARIF results should be an array");
    let artifact_uri = sarif_artifact_uri(&patch.file);
    assert!(results.iter().any(|result| {
        result["ruleId"]
            .as_str()
            .is_some_and(|id| id.starts_with("arborist.syntax."))
            && result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"] == artifact_uri
    }));
    assert!(
        results
            .iter()
            .any(|result| result["ruleId"] == "arborist.patch-gate")
    );
}

#[test]
fn exports_unresolved_binding_diagnostics_as_sarif() {
    let dir = temporary_dir();
    let caller = dir.join("caller.py");
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    return missing_helper(value)\n",
        None,
    )
    .unwrap();
    let sarif = export_patch_diagnostics_sarif(&patch).unwrap();
    let results = sarif["runs"][0]["results"]
        .as_array()
        .expect("SARIF results should be an array");

    assert!(results.iter().any(|result| {
        result["ruleId"] == "arborist.binding.unresolved"
            && result["level"] == "error"
            && result["message"]["text"]
                .as_str()
                .is_some_and(|message| message.contains("missing_helper"))
    }));
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
