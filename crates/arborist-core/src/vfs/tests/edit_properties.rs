use super::*;
use std::sync::LazyLock;

use proptest::prelude::*;

static SOURCE_CHARACTERS: LazyLock<Vec<char>> = LazyLock::new(|| {
    vec![
        'a', 'z', '0', '9', ' ', '_', '\n', '\t', 'é', '茅', '中', '🙂', '🎉',
    ]
});

fn source_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*SOURCE_CHARACTERS), 0..=48)
        .prop_map(String::from_iter)
}

fn replacement_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*SOURCE_CHARACTERS), 0..=16)
        .prop_map(String::from_iter)
}

prop_compose! {
    fn byte_edit_strategy()(source in source_strategy())(
        start in 0..=source.len(),
        end in 0..=source.len(),
        replacement in replacement_strategy(),
        source in Just(source),
    ) -> (String, usize, usize, String) {
        let (start, end) = if start <= end { (start, end) } else { (end, start) };
        (source, start, end, replacement)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn apply_edit_replaces_exactly_the_requested_range(
        (source, start, end, replacement) in byte_edit_strategy(),
    ) {
        prop_assume!(
            source.is_char_boundary(start) && source.is_char_boundary(end)
        );

        let file = temp_file(&source);
        let mut vfs = VirtualFileSystem::new();
        vfs.read_file(&file).unwrap();

        vfs.apply_edit(&file, start, end, &replacement).unwrap();
        let snapshot = vfs.read_file(&file).unwrap();

        let expected = format!(
            "{}{replacement}{}",
            &source[..start],
            &source[end..],
        );
        prop_assert_eq!(snapshot.source, expected);
    }

    #[test]
    fn empty_range_edit_preserves_source(
        source in source_strategy(),
        anchor in 0usize..=1,
    ) {
        let start = anchor * source.len();

        let file = temp_file(&source);
        let mut vfs = VirtualFileSystem::new();
        vfs.read_file(&file).unwrap();

        vfs.apply_edit(&file, start, start, "").unwrap();
        let snapshot = vfs.read_file(&file).unwrap();

        prop_assert_eq!(snapshot.source, source);
    }

    #[test]
    fn out_of_bounds_edits_are_rejected_without_dirtying_the_buffer(
        source in source_strategy(),
        replacement in replacement_strategy(),
        overshoot in 1usize..8,
    ) {
        let file = temp_file(&source);
        let mut vfs = VirtualFileSystem::new();
        let initial = vfs.read_file(&file).unwrap();

        let offset = source.len() + overshoot;
        let error = vfs
            .apply_edit(&file, offset, offset, &replacement)
            .expect_err("out-of-bounds edits should be rejected");

        prop_assert!(error.to_string().contains("out of bounds"));
        let snapshot = vfs.read_file(&file).unwrap();
        prop_assert_eq!(snapshot.source, initial.source);
        prop_assert_eq!(snapshot.version, initial.version);
        prop_assert_eq!(snapshot.dirty, initial.dirty);
    }

    #[test]
    fn sequential_insertions_compose_in_application_order(
        source in source_strategy(),
        first in replacement_strategy(),
        second in replacement_strategy(),
    ) {
        let file = temp_file(&source);
        let mut vfs = VirtualFileSystem::new();
        vfs.read_file(&file).unwrap();

        vfs.apply_edit(&file, 0, 0, &first).unwrap();
        vfs.apply_edit(&file, 0, 0, &second).unwrap();
        let snapshot = vfs.read_file(&file).unwrap();

        prop_assert_eq!(snapshot.source, format!("{second}{first}{source}"));
    }
}
