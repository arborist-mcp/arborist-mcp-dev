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

/// A workspace summary whose evidence key matches the derived identity so it
/// passes replay-input validation.
fn workspace_summary_strategy() -> impl Strategy<Value = SymbolSummary> {
    (nonblank_strategy(), nonblank_strategy(), 1usize..32usize).prop_map(
        |(symbol_id, file_path, end_byte)| {
            let node_kind = "function_definition".to_string();
            let origin_type = "workspace_symbol";
            let evidence_key =
                format!("{symbol_id}|{file_path}|{node_kind}|{origin_type}|0..{end_byte}|");
            let semantic_path = format!("path::{symbol_id}");
            SymbolSummary {
                symbol_id,
                semantic_path,
                scope_path: None,
                file_path,
                node_kind,
                origin_type: origin_type.to_string(),
                evidence_key,
                byte_range: (0, end_byte),
                signature: None,
                parameters: Vec::new(),
                return_type: None,
                docstring: None,
            }
        },
    )
}

/// Distinct summaries with position-suffixed ids so evidence keys stay
/// unique, plus aligned match details and consistent total/truncated flags.
fn aligned_search_strategy() -> impl Strategy<Value = SymbolSearchResult> {
    (
        nonblank_strategy(),
        prop::collection::vec(workspace_summary_strategy(), 0..=4),
    )
        .prop_map(|(query, summaries)| {
            let matches: Vec<_> = summaries
                .iter()
                .enumerate()
                .map(|(position, summary)| {
                    let mut item = summary.clone();
                    let suffixed_id = format!("{position}::{}", item.symbol_id);
                    item.symbol_id = suffixed_id.clone();
                    item.semantic_path = format!("path::{suffixed_id}");
                    item.evidence_key = format!(
                        "{}|{}|{}|{}|{}..{}|",
                        item.symbol_id,
                        item.file_path,
                        item.node_kind,
                        item.origin_type,
                        item.byte_range.0,
                        item.byte_range.1
                    );
                    item
                })
                .collect();
            let match_details: Vec<_> = matches
                .iter()
                .map(|item| SymbolSearchMatchDetail {
                    symbol_id: item.symbol_id.clone(),
                    score: 1000,
                    matched_fields: vec!["semantic_path".to_string()],
                })
                .collect();
            SymbolSearchResult {
                query,
                indexed_files: 1,
                total_matches: matches.len(),
                truncated: false,
                matches,
                match_details,
            }
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// An aligned search result validates.
    #[test]
    fn aligned_search_results_validate(result in aligned_search_strategy()) {
        prop_assert!(result.validate_public_output().is_ok());
    }

    /// Inconsistent truncation flags must be rejected: a non-truncated result
    /// cannot report more total matches than returned entries, while a
    /// truncated result must have at least one hidden entry.
    #[test]
    fn search_results_reject_inconsistent_truncation(
        result in aligned_search_strategy(),
        extra_total in 1usize..=5usize,
    ) {
        prop_assume!(!result.matches.is_empty());
        // total > len with truncated=false is rejected.
        let mut over_total = result.clone();
        over_total.total_matches += extra_total;
        prop_assert!(over_total.validate_public_output().is_err());

        // total == len with truncated=true is also rejected.
        let mut flagged = result;
        flagged.truncated = true;
        prop_assert!(flagged.validate_public_output().is_err());
    }

    /// Misaligned match details must be rejected.
    #[test]
    fn search_results_reject_misaligned_details(
        mut result in aligned_search_strategy(),
    ) {
        prop_assume!(!result.match_details.is_empty());
        let last = result.match_details.len() - 1;
        result.match_details[last].symbol_id = "other".to_string();
        prop_assert!(result.validate_public_output().is_err());
    }

    /// Duplicated evidence keys must be rejected.
    #[test]
    fn search_results_reject_duplicate_evidence_keys(
        mut result in aligned_search_strategy(),
    ) {
        prop_assume!(result.matches.len() >= 2);
        let duplicate = result.matches[0].clone();
        result.matches[1] = duplicate;
        prop_assert!(result.validate_public_output().is_err());
    }
}
