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

/// A list result with position-suffixed unique ids, aligned match-free reads,
/// and consistent total/truncated flags.
fn aligned_list_context_strategy() -> impl Strategy<Value = SymbolListContextResult> {
    (
        prop::collection::vec(workspace_summary_strategy(), 0..=4),
        1usize..8usize,
    )
        .prop_map(|(summaries, indexed_files)| {
            let symbols: Vec<_> = summaries
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
            let reads: Vec<_> = symbols.iter().map(|s| read_for(s, indexed_files)).collect();
            SymbolListContextResult {
                list: SymbolListResult {
                    indexed_files,
                    total_symbols: symbols.len(),
                    truncated: false,
                    symbols,
                },
                reads,
            }
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// An aligned list context validates.
    #[test]
    fn aligned_list_contexts_validate(context in aligned_list_context_strategy()) {
        prop_assert!(context.validate_public_output().is_ok());
    }

    /// Inconsistent truncation flags must be rejected.
    #[test]
    fn lists_reject_inconsistent_truncation(
        mut context in aligned_list_context_strategy(),
        extra_total in 1usize..=5usize,
    ) {
        prop_assume!(!context.list.symbols.is_empty());
        // total > len with truncated=false is rejected.
        context.list.total_symbols += extra_total;
        prop_assert!(context.validate_public_output().is_err());
    }

    /// Misaligned reads must be rejected: wrong length or any single-field
    /// mismatch with the corresponding symbol.
    #[test]
    fn list_contexts_reject_misaligned_reads(
        mut context in aligned_list_context_strategy(),
        which in 0usize..4,
    ) {
        prop_assume!(!context.reads.is_empty());
        match which {
            // Wrong length.
            0 => {
                context.reads.pop();
            }
            1 => context.reads[0].indexed_files += 1,
            2 => context.reads[0].symbol.symbol_id = "other".to_string(),
            // byte_range follows the same direct comparison path.
            _ => context.reads[0].symbol.byte_range = (99, 100),
        }
        prop_assert!(context.validate_public_output().is_err());
    }

    /// Duplicated evidence keys must be rejected.
    #[test]
    fn lists_reject_duplicate_evidence_keys(mut context in aligned_list_context_strategy()) {
        prop_assume!(context.list.symbols.len() >= 2);
        let duplicate = context.list.symbols[0].clone();
        context.list.symbols[1] = duplicate;
        prop_assert!(context.validate_public_output().is_err());
    }
}
