use std::sync::LazyLock;

use super::*;
use proptest::prelude::*;

/// Characters valid in non-blank capture/source text.
static IDENTIFIER_CHARACTERS: LazyLock<Vec<char>> =
    LazyLock::new(|| vec!['a', 'z', '0', '9', '_', 'A', 'Z']);

fn nonblank_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*IDENTIFIER_CHARACTERS), 1..=12)
        .prop_map(String::from_iter)
}

/// A correctly ordered (start, end) point pair for a capture.
fn ordered_positions_strategy() -> impl Strategy<Value = (Position, Position)> {
    (0usize..6usize, 0usize..24usize).prop_flat_map(|(row, column)| {
        (row..=row + 2, column..=column + 12).prop_map(move |(end_row, end_col)| {
            (
                Position { row, column },
                Position {
                    row: end_row,
                    column: end_col,
                },
            )
        })
    })
}

/// A well-formed capture whose owner and scope fields follow the checker's
/// pairing rules depending on the flags.
fn capture_strategy(
    with_owner: bool,
    with_scope: bool,
) -> impl Strategy<Value = QueryCaptureResult> {
    (
        nonblank_strategy(),
        nonblank_strategy(),
        0usize..32usize,
        0usize..16usize,
        ordered_positions_strategy(),
    )
        .prop_map(
            move |(capture_name, node_kind, start_byte, span, (sp, ep))| QueryCaptureResult {
                capture_name,
                node_kind,
                text: "captured".to_string(),
                owner_symbol_id: with_owner.then_some("owner".to_string()),
                owner_semantic_path: with_owner.then_some("owner::symbol".to_string()),
                owner_scope_path: with_scope.then_some("scope".to_string()),
                start_byte,
                end_byte: start_byte + span,
                start_point: sp,
                end_point: ep,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any well-formed capture -- with or without owner and scope metadata --
    /// must validate through the public checker.
    #[test]
    fn well_formed_captures_validate(
        without_owner in capture_strategy(false, false),
        with_owner in capture_strategy(true, false),
        with_scope in capture_strategy(true, true),
    ) {
        prop_assert!(without_owner.validate_public_output(0).is_ok());
        prop_assert!(with_owner.validate_public_output(0).is_ok());
        prop_assert!(with_scope.validate_public_output(0).is_ok());
    }

    /// Reversing the byte range must be rejected.
    #[test]
    fn captures_reject_reversed_byte_range(mut capture in capture_strategy(true, false)) {
        let start = capture.start_byte;
        capture.start_byte = capture.end_byte + 1;
        capture.end_byte = start;
        prop_assert!(capture.validate_public_output(0).is_err());
    }

    /// Placing the start point after the end point must be rejected.
    #[test]
    fn captures_reject_start_point_after_end_point(mut capture in capture_strategy(false, false)) {
        std::mem::swap(&mut capture.start_point, &mut capture.end_point);
        prop_assume!(point_is_after(&capture.start_point, &capture.end_point));
        prop_assert!(capture.validate_public_output(0).is_err());
    }

    /// Owner fields must be present together, and scope path requires an owner.
    #[test]
    fn captures_reject_inconsistent_owner_and_scope(
        consistent in capture_strategy(true, true),
        ownernone in capture_strategy(false, false),
    ) {
        // Fully consistent: accept.
        prop_assert!(consistent.validate_public_output(0).is_ok());

        // Drop only the semantic path: partial owner fields.
        let mut partial = consistent;
        partial.owner_semantic_path = None;
        prop_assert!(partial.validate_public_output(0).is_err());

        // Owner symbol id alone.
        let mut symbol_only = ownernone.clone();
        symbol_only.owner_symbol_id = Some("owner".to_string());
        prop_assert!(symbol_only.validate_public_output(0).is_err());

        // Owner semantic path alone.
        let mut semantic_only = ownernone.clone();
        semantic_only.owner_semantic_path = Some("owner::symbol".to_string());
        prop_assert!(semantic_only.validate_public_output(0).is_err());

        // Scope path without any owner.
        let mut scope_only = ownernone.clone();
        scope_only.owner_scope_path = Some("scope".to_string());
        prop_assert!(scope_only.validate_public_output(0).is_err());
    }
}
