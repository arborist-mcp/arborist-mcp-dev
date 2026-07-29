use super::*;

#[test]
fn workspace_edit_preview_binding_forwards_zero_timeout() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
        .preview_workspace_position_edits_json("[]", Some(0))
        .expect_err("zero timeout should reach workspace edit preview");

    assert!(
        error
            .to_string()
            .contains("invalid workspace edit preview timeout_ms: value must be greater than zero")
    );
}
#[test]
fn trace_context_patch_bindings_forward_zero_timeout_before_patch_work() {
    prepare_python();

    let core = ArboristCore::new();
    let errors = [
        core.validate_patch_with_trace_context_json_impl(
            ".",
            "missing.py",
            "missing",
            "def missing():\n    return 1\n",
            None,
            None,
            "both",
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach semantic trace-context validation"),
        core.validate_patch_with_trace_context_at_position_json_impl(
            ".",
            "missing.py",
            0,
            0,
            "def missing():\n    return 1\n",
            None,
            None,
            "both",
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach position trace-context validation"),
    ];

    for error in errors {
        assert!(
            error
                .to_string()
                .contains("invalid trace timeout_ms: value must be greater than zero")
        );
    }
}
#[test]
fn graph_context_patch_bindings_forward_zero_timeout_before_patch_work() {
    prepare_python();

    let core = ArboristCore::new();
    let errors = [
        core.validate_patch_with_graph_context_json_impl(
            ".",
            "missing.py",
            "missing",
            "def missing():
    return 1
",
            None,
            None,
            "both",
            NeighborhoodBounds::new(2, 64),
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach semantic graph-context validation"),
        core.validate_patch_with_graph_context_at_position_json_impl(
            ".",
            "missing.py",
            0,
            0,
            "def missing():
    return 1
",
            None,
            None,
            "both",
            NeighborhoodBounds::new(2, 64),
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach position graph-context validation"),
    ];

    for error in errors {
        assert!(
            error
                .to_string()
                .contains("invalid trace timeout_ms: value must be greater than zero")
        );
    }
}
#[test]
fn rich_context_patch_bindings_forward_zero_timeout_before_patch_work() {
    prepare_python();

    let core = ArboristCore::new();
    let bounds = NeighborhoodBounds::new(2, 64);
    let errors = [
        core.validate_patch_with_neighborhood_context_json_impl(
            ".",
            "missing.py",
            "missing",
            "def missing():
    return 1
",
            None,
            None,
            "both",
            bounds,
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach semantic neighborhood-context validation"),
        core.validate_patch_with_neighborhood_context_at_position_json_impl(
            ".",
            "missing.py",
            0,
            0,
            "def missing():
    return 1
",
            None,
            None,
            "both",
            bounds,
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach position neighborhood-context validation"),
        core.validate_patch_with_discovery_context_json_impl(
            ".",
            "missing.py",
            "missing",
            "def missing():
    return 1
",
            None,
            None,
            "both",
            bounds,
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach semantic discovery-context validation"),
        core.validate_patch_with_discovery_context_at_position_json_impl(
            ".",
            "missing.py",
            0,
            0,
            "def missing():
    return 1
",
            None,
            None,
            "both",
            bounds,
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach position discovery-context validation"),
    ];

    for error in errors {
        assert!(
            error
                .to_string()
                .contains("invalid trace timeout_ms: value must be greater than zero")
        );
    }
}
#[test]
fn offline_patch_analysis_bindings_forward_zero_timeout() {
    prepare_python();

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!(
        "arborist-py-offline-analysis-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&workspace).unwrap();
    let file = workspace.join("sample.py");
    fs::write(&file, "def target() -> int:\n    return 1\n").unwrap();

    let patch = patch_ast_node_from_path(
        &file,
        "target",
        "def target() -> int:\n    return 2\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&workspace, "target", TraceDirection::Both).unwrap();
    let patch_json = serde_json::to_string(&patch).unwrap();
    let trace_json = serde_json::to_string(&trace).unwrap();
    let core = ArboristCore::new();

    let errors = [
        core.replay_patch_evidence_against_trace_json_with_timeout_impl(
            &patch_json,
            &trace_json,
            Some(0),
        )
        .expect_err("zero replay timeout should reach core"),
        core.validate_patch_commit_with_trace_json_with_timeout_impl(
            &patch_json,
            &trace_json,
            Some(0),
        )
        .expect_err("zero trace validation timeout should reach core"),
        core.export_patch_diagnostics_sarif_json_with_timeout_impl(&patch_json, Some(0))
            .expect_err("zero SARIF timeout should reach core"),
    ];

    for (error, operation) in errors.into_iter().zip([
        "patch evidence replay",
        "patch trace validation",
        "SARIF export",
    ]) {
        assert!(
            error
                .to_string()
                .contains(&format!("invalid {operation} timeout_ms"))
        );
    }

    fs::remove_dir_all(workspace).unwrap();
}
