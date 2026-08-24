use std::sync::LazyLock;

use proptest::prelude::*;

use crate::symbol_index_model::symbol_base_name_ref;

/// Characters valid in identifier-like segments (no `::` or `.` separators,
/// no whitespace or control characters) so generated semantic paths exercise
/// real-world nesting without producing ambiguous separators.
static SEGMENT_CHARACTERS: LazyLock<Vec<char>> =
    LazyLock::new(|| vec!['a', 'z', '0', '9', '_', 'A', 'Z']);

fn segment_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*SEGMENT_CHARACTERS), 1..=12)
        .prop_map(String::from_iter)
}

fn path_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(segment_strategy(), 1..=5).prop_map(|segments| segments.join("::"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn symbol_base_name_extracts_the_last_segment(
        prefix_segments in prop::collection::vec(segment_strategy(), 0..=3),
        base_name in segment_strategy(),
    ) {
        let mut segments = prefix_segments;
        let base = base_name.clone();
        segments.push(base_name.clone());

        let semantic_path = segments.join("::");
        prop_assert_eq!(symbol_base_name_ref(&semantic_path), base.as_str());
    }

    #[test]
    fn symbol_base_name_is_idempotent(
        semantic_path in path_strategy(),
    ) {
        let base = symbol_base_name_ref(&semantic_path);
        prop_assert_eq!(symbol_base_name_ref(base), base);
    }

    #[test]
    fn symbol_base_name_is_always_a_nonempty_substring_of_input(
        semantic_path in path_strategy(),
    ) {
        let base = symbol_base_name_ref(&semantic_path);
        prop_assert!(!base.is_empty(), "base name must not be empty for {semantic_path:?}");
        prop_assert!(
            semantic_path.ends_with(base),
            "base name {base:?} must be a suffix of the input {semantic_path:?}"
        );
    }

    #[test]
    fn symbol_base_name_strips_method_suffix_after_dot(
        namespace_segments in prop::collection::vec(segment_strategy(), 1..=3),
        type_name in segment_strategy(),
        method_name in segment_strategy(),
    ) {
        let mut parts = namespace_segments;
        parts.push(type_name.clone());
        let joined = parts.join("::");
        let semantic_path = format!("{joined}.{}", method_name);

        // The dot-suffix form extracts the method name, not the type.
        prop_assert_eq!(symbol_base_name_ref(&semantic_path), method_name.as_str());
    }
}
