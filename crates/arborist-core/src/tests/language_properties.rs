use std::path::PathBuf;
use std::sync::LazyLock;

use proptest::prelude::*;

use crate::language::{
    normalize_absolute_path, offset_for_position, point_for_offset, position_from,
};
use crate::model::Position;

/// Characters chosen so generated sources exercise ASCII, multi-byte UTF-8,
/// line breaks, and astral-plane code points in every shrinking case.
static SOURCE_CHARACTERS: LazyLock<Vec<char>> = LazyLock::new(|| {
    vec![
        'a', 'z', '0', '9', ' ', '_', '\n', '\t', 'é', '茅', '中', '🙂', '🎉',
    ]
});

fn source_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*SOURCE_CHARACTERS), 0..=64)
        .prop_map(String::from_iter)
}

prop_compose! {
    fn boundary_offset_strategy()(source in source_strategy())(
        offset in 0..=source.len(),
        source in Just(source),
    ) -> (String, usize) {
        (source, offset)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn byte_position_helpers_round_trip_arbitrary_boundary_offsets(
        (source, offset) in boundary_offset_strategy(),
    ) {
        prop_assume!(source.is_char_boundary(offset));

        let point = point_for_offset(&source, offset).unwrap();
        let position = position_from(point);

        prop_assert_eq!(offset_for_position(&source, &position).unwrap(), offset);
    }

    #[test]
    fn byte_position_helpers_reject_arbitrary_positions_inside_characters(
        source in source_strategy(),
        row in 0usize..4,
        column in 1usize..8,
    ) {
        prop_assume!(row <= source.lines().count());

        let position = Position { row, column };
        match offset_for_position(&source, &position) {
            Ok(offset) => prop_assert!(source.is_char_boundary(offset)),
            Err(error) => prop_assert!(
                error
                    .to_string()
                    .contains("does not align to a UTF-8 character boundary")
                    || error.to_string().contains("out of bounds")
                    || error.to_string().contains("beyond"),
                "unexpected error: {error}"
            ),
        }
    }

    #[test]
    fn point_for_offset_rejects_out_of_bounds_offsets(
        source in source_strategy(),
        overshoot in 1usize..16,
    ) {
        let offset = source.len() + overshoot;

        let error =
            point_for_offset(&source, offset).expect_err("out-of-bounds offsets should be rejected");
        prop_assert!(error.to_string().contains("out of bounds"));
    }

    #[test]
    fn normalize_absolute_path_is_idempotent_for_arbitrary_relative_paths(
        relative in prop::collection::vec("[a-z_]{1,8}", 1..=5)
            .prop_map(|segments| segments.iter().fold(PathBuf::new(), |path, segment| path.join(segment))),
    ) {
        let absolute = std::env::current_dir().unwrap().join(&relative);
        let normalized = normalize_absolute_path(&absolute).unwrap();

        prop_assert!(normalized.is_absolute());
        prop_assert_eq!(normalize_absolute_path(&normalized).unwrap(), normalized);
    }

    #[test]
    fn point_for_offset_matches_reference_line_counting(
        (source, offset) in boundary_offset_strategy(),
    ) {
        prop_assume!(source.is_char_boundary(offset));

        let point = point_for_offset(&source, offset).unwrap();
        let prefix = &source.as_bytes()[..offset];
        let expected_row = prefix.iter().filter(|byte| **byte == b'\n').count();
        let expected_column = prefix
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(prefix.len(), |newline| prefix.len() - newline - 1);

        prop_assert_eq!(point.row as usize, expected_row);
        prop_assert_eq!(point.column as usize, expected_column);
    }
}
