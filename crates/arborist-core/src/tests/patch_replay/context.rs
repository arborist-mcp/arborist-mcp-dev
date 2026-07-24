use super::*;
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
    let impact = result.impact.as_ref().expect("impact should be available");
    assert_eq!(impact.affected_symbol_count, 1);
    assert!(impact.added_callers.is_empty());
    assert!(impact.removed_callers.is_empty());
    assert_eq!(impact.added_callees.len(), 1);
    assert_eq!(impact.added_callees[0].semantic_path, "helper");
    assert!(impact.removed_callees.is_empty());
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
fn trace_patch_impact_ignores_unchanged_callees_with_shifted_byte_ranges() {
    let dir = temporary_dir();
    let caller = dir.join("caller.py");
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return helper(value)\n\n\ndef helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let result = validate_patch_with_trace_context_from_path(
        &dir,
        &caller,
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    # Keep the existing dependency while changing this symbol's size.\n    return helper(value)\n",
        None,
        TraceDirection::Both,
    )
    .unwrap();

    let impact = result.impact.expect("impact should be available");
    assert!(impact.added_callers.is_empty());
    assert!(impact.removed_callers.is_empty());
    assert!(impact.added_callees.is_empty());
    assert!(impact.removed_callees.is_empty());
    assert_eq!(impact.affected_symbol_count, 0);
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
