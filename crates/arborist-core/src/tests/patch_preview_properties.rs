use std::path::Path;
use std::sync::LazyLock;

use proptest::prelude::*;

use crate::patching::unified_diff;

/// Newline-free characters so generated lines exercise ASCII, multi-byte
/// UTF-8, tabs, and astral-plane code points without embedding line breaks.
static LINE_CHARACTERS: LazyLock<Vec<char>> = LazyLock::new(|| {
    vec![
        'a', 'z', '0', '9', ' ', '_', '\t', 'é', '茅', '中', '🙂', '🎉',
    ]
});

fn line_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*LINE_CHARACTERS), 0..=16)
        .prop_map(String::from_iter)
}

fn prefix_lines_strategy() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(line_strategy(), 0..=3)
}

fn suffix_lines_strategy() -> impl Strategy<Value = Vec<String>> {
    // Keep at least one suffix line so an empty changed middle remains a real
    // interior line instead of disappearing into a trailing newline.
    prop::collection::vec(line_strategy(), 1..=2)
}

fn assemble_source(prefix: &[String], middle: &str, suffix: &[String]) -> String {
    let mut source = prefix.join("\n");
    if !prefix.is_empty() {
        source.push('\n');
    }
    source.push_str(middle);
    if !suffix.is_empty() {
        source.push('\n');
        source.push_str(&suffix.join("\n"));
    }
    source
}

fn changed_line_sources() -> impl Strategy<Value = (Vec<String>, Vec<String>, String, String)> {
    (prefix_lines_strategy(), suffix_lines_strategy()).prop_flat_map(|(prefix, suffix)| {
        (Just(prefix), Just(suffix), line_strategy(), line_strategy())
    })
}

fn changed_lines(diff: &str) -> (Vec<&str>, Vec<&str>) {
    let mut removed = Vec::new();
    let mut added = Vec::new();
    for line in diff.lines().skip(3) {
        if let Some(content) = line.strip_prefix('-') {
            removed.push(content);
        } else if let Some(content) = line.strip_prefix('+') {
            added.push(content);
        } else {
            panic!("unexpected diff line after hunk header: {line:?}");
        }
    }
    (removed, added)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn unified_diff_is_empty_for_identical_arbitrary_sources(
        source in prop::collection::vec(prop::sample::select(&*LINE_CHARACTERS), 0..=48)
            .prop_map(String::from_iter),
    ) {
        let diff = unified_diff(Path::new("sample.py"), &source, &source);

        prop_assert_eq!(diff, "");
    }

    #[test]
    fn unified_diff_reports_exactly_the_changed_interior_line(
        (prefix, suffix, old_middle, new_middle) in changed_line_sources(),
    ) {
        let old_source = assemble_source(&prefix, &old_middle, &suffix);
        let new_source = assemble_source(&prefix, &new_middle, &suffix);
        let start = prefix.len() + 1;
        let expected = if old_middle == new_middle {
            String::new()
        } else {
            format!(
                "--- a/sample.py\n+++ b/sample.py\n@@ -{start},1 +{start},1 @@\n-{old_middle}\n+{new_middle}\n"
            )
        };

        let diff = unified_diff(Path::new("sample.py"), &old_source, &new_source);

        prop_assert_eq!(diff, expected);
    }

    #[test]
    fn unified_diff_swap_swaps_removals_and_additions(
        (prefix, suffix, old_middle, new_middle) in changed_line_sources(),
    ) {
        prop_assume!(old_middle != new_middle);

        let old_source = assemble_source(&prefix, &old_middle, &suffix);
        let new_source = assemble_source(&prefix, &new_middle, &suffix);
        let forward = unified_diff(Path::new("sample.py"), &old_source, &new_source);
        let reverse = unified_diff(Path::new("sample.py"), &new_source, &old_source);

        let (forward_removed, forward_added) = changed_lines(&forward);
        let (reverse_removed, reverse_added) = changed_lines(&reverse);
        prop_assert_eq!(forward_removed, vec![old_middle.as_str()]);
        prop_assert_eq!(forward_added, vec![new_middle.as_str()]);
        prop_assert_eq!(reverse_removed, vec![new_middle.as_str()]);
        prop_assert_eq!(reverse_added, vec![old_middle.as_str()]);
    }

    #[test]
    fn unified_diff_ignores_a_trailing_newline_addition(
        source in prop::collection::vec(prop::sample::select(&*LINE_CHARACTERS), 1..=32)
            .prop_map(String::from_iter),
        trailing_newline in prop::sample::select(vec!["\n", "\r\n"]),
    ) {
        // An empty source plus a trailing newline legitimately produces a
        // one-line addition; only non-empty sources should be unchanged.
        prop_assume!(!source.contains('\n') && !source.is_empty());
        let diff = unified_diff(
            Path::new("sample.py"),
            &source,
            &format!("{source}{trailing_newline}"),
        );

        prop_assert_eq!(diff, "");
    }
}
