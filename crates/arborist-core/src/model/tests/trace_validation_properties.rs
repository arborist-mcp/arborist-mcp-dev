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

/// A replay result whose items all share the given status so the derived
/// replay status is deterministic. Item fields follow the per-item pairing
/// rules for each status.
fn replay_with_status(
    status: &'static str,
) -> impl Strategy<Value = TracePatchEvidenceReplayResult> {
    (nonblank_strategy(), 1usize..=2usize).prop_map(move |(name, count)| {
        let (matched_in_trace, trace_match_scope, selected_evidence_key) = match status {
            "matched" => (true, "callers".to_string(), Some("key".to_string())),
            "missing" => (false, "none".to_string(), Some("key".to_string())),
            _ => (false, "none".to_string(), None),
        };
        let items: Vec<_> = (0..count)
            .map(|position| TracePatchEvidenceReplayItem {
                name: format!("{name}{position}"),
                status: status.to_string(),
                selected_evidence_key: selected_evidence_key.clone(),
                matched_in_trace,
                trace_match_scope: trace_match_scope.clone(),
                candidate_evidence_keys: Vec::new(),
            })
            .collect();
        TracePatchEvidenceReplayResult {
            matched_items: if status == "matched" { count } else { 0 },
            blocked_items: if status == "blocked" { count } else { 0 },
            consistent: matches!(status, "matched" | "blocked"),
            items,
        }
    })
}

/// An allowed result whose replay evidence is fully matched.
fn allowed_result_strategy() -> impl Strategy<Value = PatchTraceValidationResult> {
    (replay_with_status("matched"), nonblank_strategy()).prop_map(|(replay, reason)| {
        PatchTraceValidationResult {
            allowed: true,
            status: "allowed".to_string(),
            reason,
            patch_gate_status: "allowed".to_string(),
            replay_status: "matched".to_string(),
            replay,
        }
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A fully allowed result with matched evidence validates.
    #[test]
    fn allowed_results_validate(result in allowed_result_strategy()) {
        prop_assert!(result.validate_public_output().is_ok());
    }

    /// A gate-rejected result validates regardless of replay evidence.
    #[test]
    fn gate_rejected_results_validate(
        reason in nonblank_strategy(),
        replay in replay_with_status("missing"),
    ) {
        let result = PatchTraceValidationResult {
            allowed: false,
            status: "rejected_by_patch_gate".to_string(),
            reason,
            patch_gate_status: "rejected".to_string(),
            // The checker only requires a non-rejected gate here; the replay
            // summary must still agree with its own items.
            replay_status: summarize(&replay),
            replay,
        };
        prop_assert!(result.validate_public_output().is_ok());
    }

    /// Flipping `allowed` on an allowed result must be rejected.
    #[test]
    fn results_reject_disallowed_allowed(mut result in allowed_result_strategy()) {
        result.allowed = false;
        prop_assert!(result.validate_public_output().is_err());
    }

    /// A rejected_by_patch_gate result claiming an allowed gate must be
    /// rejected.
    #[test]
    fn results_reject_gate_mismatch(mut result in allowed_result_strategy()) {
        result.status = "rejected_by_patch_gate".to_string();
        prop_assert!(result.validate_public_output().is_err());
    }
}

/// Mirror of the internal summarizer used to build consistent fixtures.
fn summarize(replay: &TracePatchEvidenceReplayResult) -> String {
    if replay.items.iter().any(|i| i.status == "failed") {
        return "failed".to_string();
    }
    if replay.items.iter().any(|i| i.status == "missing") {
        return "missing".to_string();
    }
    if replay.items.iter().any(|i| i.status == "blocked") {
        return "blocked".to_string();
    }
    "matched".to_string()
}
