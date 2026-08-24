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

fn read_for(symbol: &SymbolSummary, indexed_files: usize) -> SymbolReadResult {
    SymbolReadResult {
        indexed_files,
        symbol: symbol.clone(),
        source: "def value():\n    return 1\n".to_string(),
        start_point: Position { row: 0, column: 0 },
        end_point: Position { row: 1, column: 14 },
    }
}

/// A search result with position-suffixed unique ids and aligned details.
fn aligned_search_result_strategy() -> impl Strategy<Value = SymbolSearchResult> {
    (
        nonblank_strategy(),
        prop::collection::vec(workspace_summary_strategy(), 0..=4),
        1usize..8usize,
    )
        .prop_map(|(query, summaries, indexed_files)| {
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
                indexed_files,
                total_matches: matches.len(),
                truncated: false,
                matches,
                match_details,
            }
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A search context whose reads mirror every match validates.
    #[test]
    fn aligned_search_contexts_validate(search in aligned_search_result_strategy()) {
        prop_assume!(!search.matches.is_empty());
        let reads: Vec<_> = search
            .matches
            .iter()
            .map(|m| read_for(m, search.indexed_files))
            .collect();
        let context = SymbolSearchContextResult { search, reads };
        prop_assert!(context.validate_public_output().is_ok());
    }

    /// Misaligned reads must be rejected: wrong length or any single-field
    /// mismatch with the corresponding match.
    #[test]
    fn search_contexts_reject_misaligned_reads(
        search in aligned_search_result_strategy(),
        which in 0usize..3,
    ) {
        prop_assume!(!search.matches.is_empty());
        let reads: Vec<_> = search
            .matches
            .iter()
            .map(|m| read_for(m, search.indexed_files))
            .collect();
        let mut context = SymbolSearchContextResult { search, reads };

        match which {
            // Wrong length.
            0 => {
                context.reads.pop();
            }
            1 => context.reads[0].indexed_files += 1,
            // semantic_path follows the same direct comparison path.
            _ => context.reads[0].symbol.semantic_path = "other".to_string(),
        }
        prop_assert!(context.validate_public_output().is_err());
    }
}
