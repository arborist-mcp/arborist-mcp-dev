use super::*;
use crate::api_patch_validation::{
    export_patch_diagnostics_sarif_with_deadline,
    replay_patch_evidence_against_trace_with_deadline,
    validate_patch_commit_with_trace_with_deadline,
};
use crate::deadline::CooperativeDeadline;
use crate::{PatchAstNodeResult, TraceSymbolGraphResult};

fn valid_patch_and_trace() -> (PatchAstNodeResult, TraceSymbolGraphResult) {
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
        "def orchestrate(value: int) -> int:\n    return value + 2\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    (patch, trace)
}

#[test]
fn patch_analysis_timeouts_validate_bounds_before_analysis() {
    let (patch, trace) = valid_patch_and_trace();

    let errors = [
        replay_patch_evidence_against_trace_with_timeout(&patch, &trace, Some(0))
            .expect_err("zero replay timeout should fail"),
        validate_patch_commit_with_trace_with_timeout(&patch, &trace, Some(0))
            .expect_err("zero trace validation timeout should fail"),
        export_patch_diagnostics_sarif_with_timeout(&patch, Some(0))
            .expect_err("zero SARIF timeout should fail"),
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

    let excessive = replay_patch_evidence_against_trace_with_timeout(
        &patch,
        &trace,
        Some(MAX_PATCH_ANALYSIS_TIMEOUT_MS + 1),
    )
    .expect_err("excessive replay timeout should fail");
    assert!(
        excessive
            .to_string()
            .contains(&format!("must not exceed {MAX_PATCH_ANALYSIS_TIMEOUT_MS}"))
    );
}

#[test]
fn timed_patch_analysis_preserves_legacy_results() {
    let (patch, trace) = valid_patch_and_trace();

    let replay = replay_patch_evidence_against_trace(&patch, &trace).unwrap();
    let timed_replay = replay_patch_evidence_against_trace_with_timeout(
        &patch,
        &trace,
        Some(MAX_PATCH_ANALYSIS_TIMEOUT_MS),
    )
    .unwrap();
    assert_eq!(timed_replay, replay);

    let validation = validate_patch_commit_with_trace(&patch, &trace).unwrap();
    let timed_validation = validate_patch_commit_with_trace_with_timeout(
        &patch,
        &trace,
        Some(MAX_PATCH_ANALYSIS_TIMEOUT_MS),
    )
    .unwrap();
    assert_eq!(timed_validation, validation);

    let sarif = export_patch_diagnostics_sarif(&patch).unwrap();
    let timed_sarif =
        export_patch_diagnostics_sarif_with_timeout(&patch, Some(MAX_PATCH_ANALYSIS_TIMEOUT_MS))
            .unwrap();
    assert_eq!(timed_sarif, sarif);
}

#[test]
fn expired_patch_analysis_deadlines_stop_before_payload_work() {
    let (patch, trace) = valid_patch_and_trace();

    let cases = [
        (
            replay_patch_evidence_against_trace_with_deadline(
                &patch,
                &trace,
                &CooperativeDeadline::expired_for_tests(1, "patch evidence replay"),
            )
            .expect_err("expired replay deadline should fail"),
            "patch evidence replay",
        ),
        (
            validate_patch_commit_with_trace_with_deadline(
                &patch,
                &trace,
                &CooperativeDeadline::expired_for_tests(1, "patch trace validation"),
            )
            .expect_err("expired trace validation deadline should fail"),
            "patch trace validation",
        ),
        (
            export_patch_diagnostics_sarif_with_deadline(
                &patch,
                &CooperativeDeadline::expired_for_tests(1, "SARIF export"),
            )
            .expect_err("expired SARIF deadline should fail"),
            "SARIF export",
        ),
    ];

    for (error, operation) in cases {
        let message = error.to_string();
        assert!(message.contains(&format!("{operation} timeout exceeded")));
        assert!(message.contains("during validating patch payload"));
    }
}
