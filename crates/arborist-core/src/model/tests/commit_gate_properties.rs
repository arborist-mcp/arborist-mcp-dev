use std::sync::LazyLock;

use super::*;
use proptest::prelude::*;

/// Characters valid in identifier-like non-blank strings.
static IDENTIFIER_CHARACTERS: LazyLock<Vec<char>> =
    LazyLock::new(|| vec!['a', 'z', '0', '9', '_', 'A', 'Z']);

fn nonblank_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*IDENTIFIER_CHARACTERS), 1..=12)
        .prop_map(String::from_iter)
}

/// A gate report whose status, allowed flag, bypass reason, and blocker set
/// follow the checker's pairing rules for the given status.
fn consistent_gate_strategy(
    status: &'static str,
) -> impl Strategy<Value = (PatchCommitGateReport, bool, bool)> {
    (nonblank_strategy(), 0usize..2usize).prop_map(move |(reason, syntax_errors)| {
        let error_count = if status == "allowed" {
            0
        } else {
            syntax_errors + 1
        };
        let gate = PatchCommitGateReport {
            status: status.to_string(),
            allowed: status != "rejected",
            reason,
            bypass_reason: (status == "allowed_with_bypass").then(|| "user override".to_string()),
            blocking_decisions: Vec::new(),
            evidence_invariants: Vec::new(),
            syntax_error_count: error_count,
        };
        let applied = status != "rejected";
        let bypass_applied = status == "allowed_with_bypass";
        (gate, applied, bypass_applied)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Each consistent gate pairing validates against the matching patch
    /// flags and its own syntax-error count.
    #[test]
    fn consistent_gates_validate(
        allowed in consistent_gate_strategy("allowed"),
        bypass in consistent_gate_strategy("allowed_with_bypass"),
        rejected in consistent_gate_strategy("rejected"),
    ) {
        let (gate, applied, bypass_applied) = allowed;
        prop_assert!(gate.validate_trace_replay_input(applied, bypass_applied, gate.syntax_error_count).is_ok());

        let (gate, applied, bypass_applied) = bypass;
        prop_assert!(gate.validate_trace_replay_input(applied, bypass_applied, gate.syntax_error_count).is_ok());

        let (gate, applied, bypass_applied) = rejected;
        prop_assert!(gate.validate_trace_replay_input(applied, bypass_applied, gate.syntax_error_count).is_ok());
    }

    /// An allowed gate reporting a bypass reason must be rejected.
    #[test]
    fn gates_reject_allowed_with_bypass_reason(
        (mut gate, applied, bypass_applied) in consistent_gate_strategy("allowed"),
    ) {
        gate.bypass_reason = Some("stray".to_string());
        prop_assert!(gate.validate_trace_replay_input(applied, bypass_applied, gate.syntax_error_count).is_err());
    }

    /// An allowed gate with a syntax-error blocker must be rejected.
    #[test]
    fn gates_reject_allowed_with_syntax_errors(
        (mut gate, applied, bypass_applied) in consistent_gate_strategy("allowed"),
        extra in 1usize..=3usize,
    ) {
        gate.syntax_error_count += extra;
        prop_assert!(gate.validate_trace_replay_input(applied, bypass_applied, gate.syntax_error_count).is_err());
    }

    /// A bypass-applied patch whose gate is not allowed_with_bypass must be
    /// rejected.
    #[test]
    fn gates_reject_bypass_flag_mismatch(
        (gate, applied, _) in consistent_gate_strategy("allowed"),
    ) {
        prop_assert!(gate.validate_trace_replay_input(applied, false, gate.syntax_error_count).is_ok());
        prop_assert!(gate.validate_trace_replay_input(applied, true, gate.syntax_error_count).is_err());
    }
}
