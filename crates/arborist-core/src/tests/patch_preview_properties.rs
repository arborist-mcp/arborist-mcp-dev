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

fn nonempty_line_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(
        prop::collection::vec(prop::sample::select(&*LINE_CHARACTERS), 1..=16)
            .prop_map(String::from_iter),
        min..=max,
    )
}

fn changed_multi_line_sources()
-> impl Strategy<Value = (Vec<String>, Vec<String>, Vec<String>, Vec<String>)> {
    (prefix_lines_strategy(), suffix_lines_strategy()).prop_flat_map(|(prefix, suffix)| {
        (
            Just(prefix),
            Just(suffix),
            nonempty_line_vec_strategy(1, 3),
            nonempty_line_vec_strategy(1, 3),
        )
    })
}

fn insertion_sources() -> impl Strategy<Value = (Vec<String>, Vec<String>, Vec<String>)> {
    (prefix_lines_strategy(), suffix_lines_strategy()).prop_flat_map(|(prefix, suffix)| {
        (Just(prefix), Just(suffix), nonempty_line_vec_strategy(1, 4))
    })
}

fn parse_hunk(diff: &str) -> (u32, u32, u32, u32, Vec<&str>, Vec<&str>) {
    let mut lines = diff.lines();
    assert_eq!(lines.next(), Some("--- a/sample.py"));
    assert_eq!(lines.next(), Some("+++ b/sample.py"));
    let header = lines.next().expect("hunk header must be present");
    let header = header
        .strip_prefix("@@ -")
        .and_then(|rest| rest.strip_suffix(" @@"))
        .expect("hunk header must match @@ -o,n +m,k @@");
    let parts: Vec<&str> = header.split(' ').collect();
    assert_eq!(parts.len(), 2, "hunk header must have two ranges: {header}");
    let parse_range = |range: &str| -> (u32, u32) {
        let mut halves = range.splitn(2, ',');
        let start: u32 = halves.next().unwrap().parse().unwrap();
        let count: u32 = halves.next().map(|c| c.parse().unwrap()).unwrap_or(1);
        (start, count)
    };
    let (old_start, old_count) = parse_range(parts[0]);
    let (new_start, new_count) = parse_range(parts[1]);

    let mut removed = Vec::new();
    let mut added = Vec::new();
    for line in lines {
        if let Some(content) = line.strip_prefix('-') {
            removed.push(content);
        } else if let Some(content) = line.strip_prefix('+') {
            added.push(content);
        } else {
            panic!("unexpected diff line after hunk header: {line:?}");
        }
    }
    assert_eq!(
        removed.len() as u32,
        old_count,
        "hunk header old count {old_count} must match {removed:?}"
    );
    assert_eq!(
        added.len() as u32,
        new_count,
        "hunk header new count {new_count} must match {added:?}"
    );
    (old_start, old_count, new_start, new_count, removed, added)
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
    fn unified_diff_multi_line_hunk_counts_match_content(
        (prefix, suffix, old_middle, new_middle) in changed_multi_line_sources(),
    ) {
        prop_assume!(old_middle != new_middle);

        let old_source = assemble_source(&prefix, &old_middle.join("\n"), &suffix);
        let new_source = assemble_source(&prefix, &new_middle.join("\n"), &suffix);
        let diff = unified_diff(Path::new("sample.py"), &old_source, &new_source);

        let (old_start, _, new_start, _, removed, added) = parse_hunk(&diff);
        // Every removed line must come from the old middle; every added line
        // must go to the new middle. Prefix/suffix lines must never leak.
        for line in &removed {
            prop_assert!(
                old_middle.iter().any(|l| l == line),
                "removed {line:?} must belong to old_middle {old_middle:?}"
            );
        }
        for line in &added {
            prop_assert!(
                new_middle.iter().any(|l| l == line),
                "added {line:?} must belong to new_middle {new_middle:?}"
            );
        }
        // Hunk positions must point inside the source, not past it.
        prop_assert!(old_start >= 1);
        prop_assert!(new_start >= 1);
        prop_assert!(
            (old_start as usize - 1) <= old_source.lines().count(),
            "old_start {old_start} must not exceed {} old lines",
            old_source.lines().count()
        );
        prop_assert!(
            (new_start as usize - 1) <= new_source.lines().count(),
            "new_start {new_start} must not exceed {} new lines",
            new_source.lines().count()
        );
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

        let (_, _, _, _, forward_removed, forward_added) = parse_hunk(&forward);
        let (_, _, _, _, reverse_removed, reverse_added) = parse_hunk(&reverse);
        prop_assert_eq!(forward_removed, vec![old_middle.as_str()]);
        prop_assert_eq!(forward_added, vec![new_middle.as_str()]);
        prop_assert_eq!(reverse_removed, vec![new_middle.as_str()]);
        prop_assert_eq!(reverse_added, vec![old_middle.as_str()]);
    }

    #[test]
    fn unified_diff_pure_insertion_between_adjacent_blocks_reports_only_additions(
        (prefix, inserted, suffix) in insertion_sources(),
    ) {
        prop_assume!(
            !inserted.is_empty()
                && inserted.iter().all(|line| !line.is_empty())
                // Disjoint from surrounding blocks so the diff is unambiguously
                // an insertion rather than a replacement of matching lines.
                && inserted.iter().all(|line| !prefix.contains(line))
                && inserted.iter().all(|line| !suffix.contains(line))
        );

        // A unique placeholder cannot match prefix/suffix/inserted, so the
        // diff is unambiguously a replacement of placeholder by inserted lines.
        let placeholder = "\u{0}PLACEHOLDER\u{0}";
        prop_assume!(inserted.iter().all(|line| !line.contains(placeholder)));
        let old_source = assemble_source(&prefix, placeholder, &suffix);
        let new_source = assemble_source(&prefix, &inserted.join("\n"), &suffix);
        let diff = unified_diff(Path::new("sample.py"), &old_source, &new_source);

        let (_, _, _, _, removed, _) = parse_hunk(&diff);
        // Exactly the placeholder line is replaced by the inserted block.
        prop_assert_eq!(removed.len(), 1);
        prop_assert_eq!(removed[0], placeholder);

        // All inserted content appears among additions (possibly merged with
        // adjacent matching lines).
        for line in &inserted {
            let (_, _, _, _, _, added) = parse_hunk(&diff);
            prop_assert!(
                added.contains(&line.as_str()),
                "inserted {line:?} must appear in additions"
            );
        }
    }

    #[test]
    fn unified_diff_pure_deletion_reports_only_removals(
        (prefix, deleted, suffix) in insertion_sources(),
    ) {
        prop_assume!(
            !deleted.is_empty()
                && deleted.iter().all(|line| !line.is_empty())
                // Disjoint from surrounding blocks so the diff is unambiguously
                // a deletion rather than a replacement with matching lines.
                && deleted.iter().all(|line| !prefix.contains(line))
                && deleted.iter().all(|line| !suffix.contains(line))
        );

        let placeholder = "\u{0}PLACEHOLDER\u{0}";
        prop_assume!(deleted.iter().all(|line| !line.contains(placeholder)));
        let old_source = assemble_source(&prefix, &deleted.join("\n"), &suffix);
        let new_source = assemble_source(&prefix, placeholder, &suffix);
        let diff = unified_diff(Path::new("sample.py"), &old_source, &new_source);

        let (_, _, _, _, removed, added) = parse_hunk(&diff);
        // Exactly the placeholder line replaces the deleted block.
        prop_assert_eq!(added.len(), 1);
        prop_assert_eq!(added[0], placeholder);

        // All deleted content appears among removals.
        for line in &deleted {
            prop_assert!(
                removed.contains(&line.as_str()),
                "deleted {line:?} must appear in removals"
            );
        }
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
