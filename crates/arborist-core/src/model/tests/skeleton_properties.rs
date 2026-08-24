use std::sync::LazyLock;

use super::*;
use proptest::prelude::*;

/// Characters valid in identifier-like strings, so generated semantic paths
/// and symbol IDs stay unambiguous and non-blank.
static IDENTIFIER_CHARACTERS: LazyLock<Vec<char>> =
    LazyLock::new(|| vec!['a', 'z', '0', '9', '_', 'A', 'Z']);

fn nonblank_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*IDENTIFIER_CHARACTERS), 1..=12)
        .prop_map(String::from_iter)
}

/// A valid symbol: every fabricable non-blank field set, byte range ordered,
/// node kind non-blank.
fn symbol_strategy() -> impl Strategy<Value = SemanticSkeletonSymbol> {
    (nonblank_strategy(), nonblank_strategy(), 0usize..16).prop_map(
        |(symbol_id, semantic_path, byte_span)| SemanticSkeletonSymbol {
            symbol_id,
            semantic_path,
            scope_path: None,
            node_kind: "function_definition".to_string(),
            byte_range: (0, byte_span),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
        },
    )
}

/// A skeleton whose paths and symbols are aligned by position and consistent:
/// `available_paths[i]` equals `available_symbols[i].semantic_path`.
fn aligned_skeleton_strategy() -> impl Strategy<Value = SemanticSkeleton> {
    let file = nonblank_strategy();
    let paths = prop::collection::vec(nonblank_strategy(), 0..=5);
    let symbols = prop::collection::vec(symbol_strategy(), 0..=5);
    (file, paths, symbols).prop_map(|(file, paths, symbols)| {
        // Rebuild symbols so each semantic_path matches its aligned path.
        let count = paths.len().min(symbols.len());
        let mut merged = Vec::with_capacity(count);
        for index in 0..count {
            let mut symbol = symbols[index].clone();
            symbol.semantic_path = paths[index].clone();
            merged.push(symbol);
        }
        SemanticSkeleton {
            file,
            skeleton: "def value() -> int:\n    return 1\n".to_string(),
            available_paths: paths[..count].to_vec(),
            available_symbols: merged,
        }
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any skeleton whose paths and symbols are positionally aligned and where
    /// each symbol is individually valid must validate.
    #[test]
    fn aligned_skeletons_always_validate(
        skeleton in aligned_skeleton_strategy(),
    ) {
        prop_assert!(skeleton.validate_public_output().is_ok());
    }

    /// A skeleton with more paths than symbols must be rejected, because the
    /// verification loop relies on exact alignment.
    #[test]
    fn skeletons_reject_more_paths_than_symbols(
        skeleton in aligned_skeleton_strategy(),
        extra in nonblank_strategy(),
    ) {
        let mut broken = skeleton;
        broken.available_paths.push(extra);
        prop_assert!(broken.validate_public_output().is_err());
    }

    /// A skeleton with more symbols than paths must be rejected.
    #[test]
    fn skeletons_reject_more_symbols_than_paths(
        skeleton in aligned_skeleton_strategy(),
        extra in symbol_strategy(),
    ) {
        let mut broken = skeleton;
        // Keep the extra symbol's semantic_path matching nothing; length
        // mismatch alone is the invariant.
        broken.available_symbols.push(extra);
        prop_assert!(broken.validate_public_output().is_err());
    }

    /// Replacing one symbol's path with a value that cannot equal any aligned
    /// path makes the skeleton inconsistent, so it must be rejected.
    #[test]
    fn skeletons_reject_misaligned_symbol_path(
        mut skeleton in aligned_skeleton_strategy(),
        index in 0usize..=4,
    ) {
        if skeleton.available_symbols.is_empty() {
            prop_assume!(false);
        }
        let index = index % skeleton.available_symbols.len();
        // Generation characters never include a separator, so this string is
        // disjoint from every generated path and guarantees a mismatch.
        skeleton.available_symbols[index].semantic_path = "mismatch::$aligned".to_string();
        prop_assert!(skeleton.validate_public_output().is_err());
    }
}
