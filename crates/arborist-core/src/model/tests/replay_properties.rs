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

/// A replay item whose status, match flag, scope, and selected key follow the
/// checker's pairing rules.
fn consistent_item_strategy(
    status: &'static str,
) -> impl Strategy<Value = TracePatchEvidenceReplayItem> {
    (nonblank_strategy(), 0usize..2usize).prop_map(move |(name, candidate_count)| {
        let (matched_in_trace, trace_match_scope, selected_evidence_key) = match status {
            "matched" => (true, "callers".to_string(), Some("key".to_string())),
            "missing" => (false, "none".to_string(), Some("key".to_string())),
            "blocked" => (false, "none".to_string(), None),
            _ => (false, "none".to_string(), None),
        };
        let candidate_evidence_keys: Vec<String> = (0..candidate_count)
            .map(|position| format!("key{position}"))
            .collect();
        TracePatchEvidenceReplayItem {
            name,
            status: status.to_string(),
            selected_evidence_key,
            matched_in_trace,
            trace_match_scope,
            candidate_evidence_keys,
        }
    })
}

/// A replay result whose summary counts and consistent flag are derived from
/// the item statuses.
fn consistent_result_strategy() -> impl Strategy<Value = TracePatchEvidenceReplayResult> {
    (
        prop::collection::vec(consistent_item_strategy("matched"), 0..=2),
        prop::collection::vec(consistent_item_strategy("missing"), 0..=2),
        prop::collection::vec(consistent_item_strategy("blocked"), 0..=2),
        prop::collection::vec(consistent_item_strategy("failed"), 0..=2),
    )
        .prop_map(|(mut matched, mut missing, mut blocked, failed)| {
            let items: Vec<_> = {
                let mut all = Vec::new();
                all.append(&mut matched);
                all.append(&mut missing);
                all.append(&mut blocked);
                all.append(&mut failed.clone());
                all
            };
            let matched_items = items.iter().filter(|i| i.status == "matched").count();
            let blocked_items = items.iter().filter(|i| i.status == "blocked").count();
            let consistent = items
                .iter()
                .all(|i| matches!(i.status.as_str(), "matched" | "blocked"));
            TracePatchEvidenceReplayResult {
                items,
                matched_items,
                blocked_items,
                consistent,
            }
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A replay result with derived summary fields validates.
    #[test]
    fn consistent_replay_results_validate(result in consistent_result_strategy()) {
        prop_assert!(result.validate_public_output().is_ok());
    }

    /// Summary count drift must be rejected.
    #[test]
    fn replay_results_reject_count_drift(
        mut result in consistent_result_strategy(),
        drift in 1usize..=3usize,
    ) {
        result.matched_items += drift;
        prop_assert!(result.validate_public_output().is_err());
    }

    /// The consistent flag must mirror whether every item is matched/blocked;
    /// flipping it on a matched/blocked-only result is rejected.
    #[test]
    fn replay_results_reject_inconsistent_flag(
        mut result in consistent_result_strategy(),
    ) {
        // Rebuild the item list to contain only matched and blocked items so
        // the original flag is true, then flip it.
        let kept: Vec<_> = result
            .items
            .iter()
            .filter(|i| matches!(i.status.as_str(), "matched" | "blocked"))
            .cloned()
            .collect();
        prop_assume!(!kept.is_empty());
        let matched_items = kept.iter().filter(|i| i.status == "matched").count();
        let blocked_items = kept.iter().filter(|i| i.status == "blocked").count();
        result.items = kept;
        result.matched_items = matched_items;
        result.blocked_items = blocked_items;
        result.consistent = true;
        prop_assert!(result.validate_public_output().is_ok());

        result.consistent = false;
        prop_assert!(result.validate_public_output().is_err());
    }

    /// A matched item claiming scope `none` must be rejected.
    #[test]
    fn replay_results_reject_matched_with_none_scope(mut result in consistent_result_strategy()) {
        let matched_index = result.items.iter().position(|i| i.status == "matched");
        prop_assume!(matched_index.is_some());
        let index = matched_index.unwrap();
        result.items[index].trace_match_scope = "none".to_string();
        prop_assert!(result.validate_public_output().is_err());
    }
}
