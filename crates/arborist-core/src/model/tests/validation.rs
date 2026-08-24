use std::sync::LazyLock;

use super::*;
use proptest::prelude::*;

/// Characters valid in identifier-like strings (no whitespace or control
/// characters) so generated values exercise real-world non-blank inputs.
static IDENTIFIER_CHARACTERS: LazyLock<Vec<char>> =
    LazyLock::new(|| vec!['a', 'z', '0', '9', '_', 'A', 'Z', '.']);

fn nonblank_string_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*IDENTIFIER_CHARACTERS), 1..=12)
        .prop_map(String::from_iter)
}

fn blank_string_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\s]{1,8}").expect("valid blank-string regex")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn ensure_nonblank_accepts_arbitrary_nonblank_values(
        value in nonblank_string_strategy(),
    ) {
        prop_assert!(ensure_nonblank(&value, "field").is_ok());
    }

    #[test]
    fn ensure_nonblank_rejects_whitespace_only_values(
        value in nonblank_string_strategy(),
        leading_whitespace in r"[\s]{0,4}",
        trailing_whitespace in r"[\s]{0,4}",
    ) {
        // Wrapped non-blank values still pass.
        let wrapped = format!("{leading_whitespace}{value}{trailing_whitespace}");
        prop_assert!(ensure_nonblank(&wrapped, "field").is_ok());

        // Stripping the non-blank core leaves only whitespace, which fails
        // whether the remainder is empty or not.
        let remainder = format!("{leading_whitespace}{trailing_whitespace}");
        let error = ensure_nonblank(&remainder, "field")
            .expect_err("whitespace-only values must be rejected");
        assert!(error.to_string().contains("must not be blank"));
    }

    #[test]
    fn ensure_nonblank_strings_pinpoints_the_first_blank_entry(
        entries in prop::collection::vec(nonblank_string_strategy(), 1..=4),
        blank_index in 0usize..=4,
        blank in blank_string_strategy(),
    ) {
        let blank_index = blank_index % (entries.len() + 1);
        let mut values: Vec<String> = Vec::new();
        for (position, entry) in entries.iter().enumerate() {
            if position == blank_index {
                values.push(blank.clone());
            }
            values.push(entry.clone());
        }
        if blank_index == entries.len() {
            values.push(blank);
        }

        let error = ensure_nonblank_strings(&values, "field")
            .expect_err("a list containing a blank value must be rejected");
        assert!(error.to_string().contains(&format!("field[{blank_index}]")));
    }

    #[test]
    fn ensure_unique_strings_accepts_distinct_and_rejects_duplicates(
        entries in prop::collection::vec(nonblank_string_strategy(), 1..=6),
        duplicate_index in 0usize..=5,
    ) {
        // Deduplicated lists always pass.
        let mut distinct: Vec<String> = Vec::new();
        for entry in &entries {
            if !distinct.contains(entry) {
                distinct.push(entry.clone());
            }
        }
        prop_assert!(ensure_unique_strings(&distinct, "field").is_ok());

        // Appending a copy of one entry fails at its second occurrence.
        let duplicate_index = duplicate_index % distinct.len();
        let mut with_duplicate = distinct.clone();
        let duplicated_value = with_duplicate[duplicate_index].clone();
        with_duplicate.push(duplicated_value);
        let expected_index = with_duplicate.len() - 1;

        let error = ensure_unique_strings(&with_duplicate, "field")
            .expect_err("duplicate values must be rejected");
        assert!(error.to_string().contains(&format!("field[{expected_index}]")));
    }
}
