use std::path::Path;

use crate::symbol_trace::TraceQueryDeadline;

use super::{
    patch_ast_node_with_trace_deadline, sarif_artifact_uri,
    validate_patch_with_discovery_context_at_position_from_index_path_with_timeout,
    validate_patch_with_discovery_context_at_position_from_index_with_timeout,
    validate_patch_with_discovery_context_at_position_from_path_with_timeout,
    validate_patch_with_discovery_context_at_position_with_timeout,
    validate_patch_with_discovery_context_from_index_path_with_timeout,
    validate_patch_with_discovery_context_from_index_with_timeout,
    validate_patch_with_discovery_context_from_path_with_timeout,
    validate_patch_with_discovery_context_with_timeout,
    validate_patch_with_graph_context_at_position_from_index_path_with_timeout,
    validate_patch_with_graph_context_at_position_from_index_with_timeout,
    validate_patch_with_graph_context_at_position_from_path_with_timeout,
    validate_patch_with_graph_context_at_position_with_deadline,
    validate_patch_with_graph_context_at_position_with_timeout,
    validate_patch_with_graph_context_from_index_path_with_timeout,
    validate_patch_with_graph_context_from_index_with_timeout,
    validate_patch_with_graph_context_from_path_with_timeout,
    validate_patch_with_graph_context_with_deadline,
    validate_patch_with_graph_context_with_timeout,
    validate_patch_with_neighborhood_context_at_position_from_index_path_with_timeout,
    validate_patch_with_neighborhood_context_at_position_from_index_with_timeout,
    validate_patch_with_neighborhood_context_at_position_from_path_with_timeout,
    validate_patch_with_neighborhood_context_at_position_with_deadline,
    validate_patch_with_neighborhood_context_at_position_with_timeout,
    validate_patch_with_neighborhood_context_from_index_path_with_timeout,
    validate_patch_with_neighborhood_context_from_index_with_timeout,
    validate_patch_with_neighborhood_context_from_path_with_timeout,
    validate_patch_with_neighborhood_context_with_deadline,
    validate_patch_with_neighborhood_context_with_timeout,
    validate_patch_with_trace_context_at_position_from_index_path_with_timeout,
    validate_patch_with_trace_context_at_position_from_index_with_timeout,
    validate_patch_with_trace_context_at_position_from_path_with_timeout,
    validate_patch_with_trace_context_at_position_with_deadline,
    validate_patch_with_trace_context_at_position_with_timeout,
    validate_patch_with_trace_context_from_index_path_with_timeout,
    validate_patch_with_trace_context_from_index_with_timeout,
    validate_patch_with_trace_context_from_path_with_timeout,
    validate_patch_with_trace_context_with_deadline,
    validate_patch_with_trace_context_with_timeout,
};
use crate::model::{Position, TraceDirection};

#[test]
fn sarif_artifact_uris_normalize_windows_paths_and_escape_components() {
    assert_eq!(
        sarif_artifact_uri("E:\\workspace\\a b\\naive-\u{00E9}.c"),
        "file:///E:/workspace/a%20b/naive-%C3%A9.c"
    );
    assert_eq!(sarif_artifact_uri("/tmp/a b.c"), "file:///tmp/a%20b.c");
    assert_eq!(
        sarif_artifact_uri(r"\\server\share\a b.c"),
        "file://server/share/a%20b.c"
    );
}

#[test]
fn trace_context_timeout_variants_reject_zero_before_path_or_patch_work() {
    let path = Path::new("");
    let position = Position { row: 0, column: 0 };
    let source = "def target():
    return 1
";
    let replacement = "def target():
    return 2
";
    let errors = [
        validate_patch_with_trace_context_with_timeout(
            path,
            path,
            source,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            Some(0),
        )
        .expect_err("workspace trace context should reject zero timeout"),
        validate_patch_with_trace_context_at_position_with_timeout(
            path,
            path,
            source,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            Some(0),
        )
        .expect_err("position trace context should reject zero timeout"),
        validate_patch_with_trace_context_from_path_with_timeout(
            path,
            path,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            Some(0),
        )
        .expect_err("path trace context should reject zero timeout"),
        validate_patch_with_trace_context_from_index_with_timeout(
            path,
            path,
            source,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            Some(0),
        )
        .expect_err("indexed trace context should reject zero timeout"),
        validate_patch_with_trace_context_at_position_from_path_with_timeout(
            path,
            path,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            Some(0),
        )
        .expect_err("position path trace context should reject zero timeout"),
        validate_patch_with_trace_context_from_index_path_with_timeout(
            path,
            path,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            Some(0),
        )
        .expect_err("indexed path trace context should reject zero timeout"),
        validate_patch_with_trace_context_at_position_from_index_with_timeout(
            path,
            path,
            source,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            Some(0),
        )
        .expect_err("indexed position trace context should reject zero timeout"),
        validate_patch_with_trace_context_at_position_from_index_path_with_timeout(
            path,
            path,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            Some(0),
        )
        .expect_err("indexed position path trace context should reject zero timeout"),
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
fn graph_context_timeout_variants_reject_zero_before_path_or_patch_work() {
    let path = Path::new("");
    let position = Position { row: 0, column: 0 };
    let source = "def target():
    return 1
";
    let replacement = "def target():
    return 2
";
    let errors = [
        validate_patch_with_graph_context_with_timeout(
            path,
            path,
            source,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("workspace graph context should reject zero timeout"),
        validate_patch_with_graph_context_at_position_with_timeout(
            path,
            path,
            source,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("position graph context should reject zero timeout"),
        validate_patch_with_graph_context_from_path_with_timeout(
            path,
            path,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("path graph context should reject zero timeout"),
        validate_patch_with_graph_context_from_index_with_timeout(
            path,
            path,
            source,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed graph context should reject zero timeout"),
        validate_patch_with_graph_context_at_position_from_path_with_timeout(
            path,
            path,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("position path graph context should reject zero timeout"),
        validate_patch_with_graph_context_from_index_path_with_timeout(
            path,
            path,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed path graph context should reject zero timeout"),
        validate_patch_with_graph_context_at_position_from_index_with_timeout(
            path,
            path,
            source,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed position graph context should reject zero timeout"),
        validate_patch_with_graph_context_at_position_from_index_path_with_timeout(
            path,
            path,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed position path graph context should reject zero timeout"),
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
fn neighborhood_context_timeout_variants_reject_zero_before_path_or_patch_work() {
    let path = Path::new("");
    let position = Position { row: 0, column: 0 };
    let source = "def target():
    return 1
";
    let replacement = "def target():
    return 2
";
    let errors = [
        validate_patch_with_neighborhood_context_with_timeout(
            path,
            path,
            source,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("workspace neighborhood context should reject zero timeout"),
        validate_patch_with_neighborhood_context_at_position_with_timeout(
            path,
            path,
            source,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("position neighborhood context should reject zero timeout"),
        validate_patch_with_neighborhood_context_from_path_with_timeout(
            path,
            path,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("path neighborhood context should reject zero timeout"),
        validate_patch_with_neighborhood_context_from_index_with_timeout(
            path,
            path,
            source,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed neighborhood context should reject zero timeout"),
        validate_patch_with_neighborhood_context_at_position_from_path_with_timeout(
            path,
            path,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("position path neighborhood context should reject zero timeout"),
        validate_patch_with_neighborhood_context_from_index_path_with_timeout(
            path,
            path,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed path neighborhood context should reject zero timeout"),
        validate_patch_with_neighborhood_context_at_position_from_index_with_timeout(
            path,
            path,
            source,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed position neighborhood context should reject zero timeout"),
        validate_patch_with_neighborhood_context_at_position_from_index_path_with_timeout(
            path,
            path,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed position path neighborhood context should reject zero timeout"),
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
fn discovery_context_timeout_variants_reject_zero_before_path_or_patch_work() {
    let path = Path::new("");
    let position = Position { row: 0, column: 0 };
    let source = "def target():
    return 1
";
    let replacement = "def target():
    return 2
";
    let errors = [
        validate_patch_with_discovery_context_with_timeout(
            path,
            path,
            source,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("workspace discovery context should reject zero timeout"),
        validate_patch_with_discovery_context_at_position_with_timeout(
            path,
            path,
            source,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("position discovery context should reject zero timeout"),
        validate_patch_with_discovery_context_from_path_with_timeout(
            path,
            path,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("path discovery context should reject zero timeout"),
        validate_patch_with_discovery_context_from_index_with_timeout(
            path,
            path,
            source,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed discovery context should reject zero timeout"),
        validate_patch_with_discovery_context_at_position_from_path_with_timeout(
            path,
            path,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("position path discovery context should reject zero timeout"),
        validate_patch_with_discovery_context_from_index_path_with_timeout(
            path,
            path,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed path discovery context should reject zero timeout"),
        validate_patch_with_discovery_context_at_position_from_index_with_timeout(
            path,
            path,
            source,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed position discovery context should reject zero timeout"),
        validate_patch_with_discovery_context_at_position_from_index_path_with_timeout(
            path,
            path,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("indexed position path discovery context should reject zero timeout"),
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
fn neighborhood_context_deadline_helpers_reject_expired_budget_before_patch_work() {
    let deadline = TraceQueryDeadline::expired_for_tests(1);
    let workspace_root = Path::new(".");
    let path = Path::new("missing.py");
    let position = Position { row: 0, column: 0 };

    let direct_error = validate_patch_with_neighborhood_context_with_deadline(
        workspace_root,
        path,
        "def target():\n    return 1\n",
        "target",
        "def target():\n    return 2\n",
        None,
        TraceDirection::Both,
        2,
        10,
        &deadline,
    )
    .expect_err("expired trace deadline must reject direct neighborhood patch validation");
    assert!(direct_error.to_string().contains("patch validation"));

    let position_error = validate_patch_with_neighborhood_context_at_position_with_deadline(
        workspace_root,
        path,
        "def target():\n    return 1\n",
        &position,
        "def target():\n    return 2\n",
        None,
        TraceDirection::Both,
        2,
        10,
        &deadline,
    )
    .expect_err("expired trace deadline must reject position neighborhood patch validation");
    assert!(
        position_error
            .to_string()
            .contains("patch position resolution")
    );
}

#[test]
fn graph_context_deadline_helpers_reject_expired_budget_before_patch_work() {
    let deadline = TraceQueryDeadline::expired_for_tests(1);
    let workspace_root = Path::new(".");
    let path = Path::new("missing.py");
    let position = Position { row: 0, column: 0 };

    let direct_error = validate_patch_with_graph_context_with_deadline(
        workspace_root,
        path,
        "def target():\n    return 1\n",
        "target",
        "def target():\n    return 2\n",
        None,
        TraceDirection::Both,
        2,
        10,
        &deadline,
    )
    .expect_err("expired trace deadline must reject direct graph-backed patch validation");
    assert!(direct_error.to_string().contains("patch validation"));

    let position_error = validate_patch_with_graph_context_at_position_with_deadline(
        workspace_root,
        path,
        "def target():\n    return 1\n",
        &position,
        "def target():\n    return 2\n",
        None,
        TraceDirection::Both,
        2,
        10,
        &deadline,
    )
    .expect_err("expired trace deadline must reject position graph-backed patch validation");
    assert!(
        position_error
            .to_string()
            .contains("patch position resolution")
    );
}

#[test]
fn trace_context_deadline_helpers_reject_expired_budget_before_patch_work() {
    let deadline = TraceQueryDeadline::expired_for_tests(1);
    let workspace_root = Path::new(".");
    let path = Path::new("missing.py");
    let position = Position { row: 0, column: 0 };

    let direct_error = validate_patch_with_trace_context_with_deadline(
        workspace_root,
        path,
        "def target():\n    return 1\n",
        "target",
        "def target():\n    return 2\n",
        None,
        TraceDirection::Both,
        &deadline,
    )
    .expect_err("expired trace deadline must reject direct trace-backed patch validation");
    assert!(direct_error.to_string().contains("patch validation"));

    let position_error = validate_patch_with_trace_context_at_position_with_deadline(
        workspace_root,
        path,
        "def target():\n    return 1\n",
        &position,
        "def target():\n    return 2\n",
        None,
        TraceDirection::Both,
        &deadline,
    )
    .expect_err("expired trace deadline must reject position trace-backed patch validation");
    assert!(
        position_error
            .to_string()
            .contains("patch position resolution")
    );
}

#[test]
fn patch_analysis_reuses_trace_deadline_for_patch_work() {
    let deadline = TraceQueryDeadline::expired_for_tests(1);

    let error = patch_ast_node_with_trace_deadline(
        &deadline,
        Path::new("missing.py"),
        "",
        "target",
        "replacement",
        None,
    )
    .expect_err("expired trace budgets must reject patch work before parsing or validation");

    assert!(error.to_string().contains("patch input validation"));
}
