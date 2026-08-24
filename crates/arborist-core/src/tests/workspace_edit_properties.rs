use std::sync::LazyLock;

use proptest::prelude::*;

use crate::{Position, PositionEdit, WorkspacePositionEdits, preview_workspace_position_edits};

/// Newline-free characters so generated lines exercise ASCII, multi-byte
/// UTF-8, tabs, and astral-plane code points without embedding line breaks.
static LINE_CHARACTERS: LazyLock<Vec<char>> = LazyLock::new(|| {
    vec![
        'a', 'z', '0', '9', ' ', '_', '\t', 'é', '茅', '中', '🙂', '🎉',
    ]
});

fn line_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*LINE_CHARACTERS), 0..=12)
        .prop_map(String::from_iter)
}

fn nonempty_line_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*LINE_CHARACTERS), 1..=12)
        .prop_map(String::from_iter)
}

fn source_from_lines(lines: &[String]) -> String {
    lines.join("\n")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn workspace_preview_single_file_replacement_reports_exact_changed_lines(
        old_line in nonempty_line_strategy(),
        new_line in nonempty_line_strategy(),
        prefix in prop::collection::vec(line_strategy(), 0..=2),
        suffix in prop::collection::vec(nonempty_line_strategy(), 1..=2),
    ) {
        let mut old_lines = prefix.clone();
        old_lines.push(old_line.clone());
        old_lines.extend(suffix.iter().cloned());

        let mut new_lines = prefix.clone();
        new_lines.push(new_line.clone());
        new_lines.extend(suffix.iter().cloned());

        let old_source = source_from_lines(&old_lines);

        let row = prefix.len();
        let col = 0usize;
        let request = WorkspacePositionEdits {
            file_path: "sample.py".to_string(),
            source: Some(old_source.clone()),
            edits: vec![PositionEdit {
                start: Position { row, column: col },
                end: Position { row: row + 1, column: 0 },
                new_text: format!("{new_line}\n"),
            }],
        };

        let result =
            preview_workspace_position_edits(std::slice::from_ref(&request))
                .expect("single-file replacement preview should succeed");

        // Top-level changed flag must agree with the file-level flag.
        prop_assert_eq!(result.files.len(), 1);
        let file = &result.files[0];
        prop_assert_eq!(file.changed, result.changed);
        if old_line == new_line {
            prop_assert!(!file.changed, "identical replacement should not report changed");
        } else {
            prop_assert!(file.changed, "different replacement should report changed");

            // The diff must contain exactly one removed and one added line.
            let diff_lines: Vec<&str> = file.unified_diff.lines().collect();
            prop_assert!(diff_lines.len() >= 3);
            let body = &diff_lines[3..];
            let removed: Vec<&str> = body
                .iter()
                .filter_map(|l| l.strip_prefix('-'))
                .collect();
            let added: Vec<&str> = body
                .iter()
                .filter_map(|l| l.strip_prefix('+'))
                .collect();
            prop_assert_eq!(removed.len(), 1);
            prop_assert_eq!(added.len(), 1);
            prop_assert_eq!(removed[0], old_line.as_str());
            prop_assert_eq!(added[0], new_line.as_str());
        }
    }

    #[test]
    fn workspace_preview_multi_file_changed_flag_is_or_of_files(
        line_a in nonempty_line_strategy(),
        line_b in nonempty_line_strategy(),
        change_second in proptest::bool::ANY,
    ) {
        let base_a = format!("{line_a}\n");
        let base_b = format!("{line_b}\n");
        let replacement = format!("{line_a}X\n");

        // File A always changes; file B changes only when change_second=true.
        let new_a = replacement.clone();
        let new_b = if change_second {
            format!("{line_b}X\n")
        } else {
            base_b.clone()
        };

        let edit_a = WorkspacePositionEdits {
            file_path: "a.py".to_string(),
            source: Some(base_a.clone()),
            edits: vec![PositionEdit {
                start: Position { row: 0, column: 0 },
                end: Position { row: 1, column: 0 },
                new_text: new_a,
            }],
        };
        let edit_b = WorkspacePositionEdits {
            file_path: "b.py".to_string(),
            source: Some(base_b.clone()),
            edits: vec![PositionEdit {
                start: Position { row: 0, column: 0 },
                end: Position { row: 1, column: 0 },
                new_text: new_b,
            }],
        };

        let result = preview_workspace_position_edits(&[edit_a, edit_b])
            .expect("multi-file workspace preview should succeed");

        prop_assert_eq!(result.files.len(), 2);
        let any_changed = result.files.iter().any(|f| f.changed);
        prop_assert_eq!(result.changed, any_changed);

        // At most one of these two is unchanged; at least one is changed
        // because the replacement differs from the original.
        let unchanged_count = result.files.iter().filter(|f| !f.changed).count();
        prop_assert!(unchanged_count <= 1, "at most one file should be unchanged");
    }

    #[test]
    fn workspace_preview_diff_never_leaks_prefix_suffix_content(
        prefix in prop::collection::vec(nonempty_line_strategy(), 1..=3),
        old_middle in nonempty_line_strategy(),
        new_middle in nonempty_line_strategy(),
        suffix in prop::collection::vec(nonempty_line_strategy(), 1..=3),
    ) {
        prop_assume!(old_middle != new_middle);
        prop_assume!(
            !prefix.contains(&old_middle) && !prefix.contains(&new_middle)
                && !suffix.contains(&old_middle) && !suffix.contains(&new_middle)
        );

        let mut old_lines = prefix.clone();
        old_lines.push(old_middle.clone());
        old_lines.extend(suffix.iter().cloned());
        let mut new_lines = prefix.clone();
        new_lines.push(new_middle.clone());
        new_lines.extend(suffix.iter().cloned());

        let old_source = source_from_lines(&old_lines);
        let row = prefix.len();
        let request = WorkspacePositionEdits {
            file_path: "sample.py".to_string(),
            source: Some(old_source),
            edits: vec![PositionEdit {
                start: Position { row, column: 0 },
                end: Position { row: row + 1, column: 0 },
                new_text: format!("{new_middle}\n"),
            }],
        };

        let result =
            preview_workspace_position_edits(std::slice::from_ref(&request))
                .expect("leak-free preview should succeed");

        let file = &result.files[0];
        prop_assert!(file.changed);
        let diff_lines: Vec<&str> = file.unified_diff.lines().collect();
        let body = &diff_lines[3..];
        for line in body {
            let content = line
                .strip_prefix('-')
                .or_else(|| line.strip_prefix('+'))
                .unwrap_or_else(|| panic!("unexpected diff line: {line:?}"));
            prop_assert!(
                content != prefix.first().map(String::as_str).unwrap_or_default()
                    && content != suffix.last().map(String::as_str).unwrap_or_default(),
                "prefix/suffix content leaked into diff: {content:?}"
            );
        }
    }
}
