use super::*;

#[test]
fn applies_incremental_edit_and_commits() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert_eq!(snapshot.version, 0);
    let digit_offset = snapshot.source.rfind('1').unwrap();

    let result = vfs
        .apply_edit(&file, digit_offset, digit_offset + 1, "2")
        .unwrap();
    assert!(result.incremental_parse);
    assert!(result.dirty);
    assert_eq!(result.version, 1);
    assert!(result.source.contains("return 2"));

    let committed = vfs.commit_file(&file).unwrap();
    assert!(!committed.dirty);
    assert!(fs::read_to_string(&file).unwrap().contains("return 2"));
}

#[test]
fn generated_position_edits_preserve_order_and_vfs_idempotence() {
    for (left, right, updated_left, updated_right) in generated_edit_cases() {
        let source = format!("def value() -> str:\n    return \"{left}:{right}\"\n");
        let file = temp_file(&source);
        let mut vfs = VirtualFileSystem::new();

        let left_start = source.find(left).unwrap();
        let after_left = source.replacen(left, updated_left, 1);
        let right_start = after_left.rfind(right).unwrap();
        let expected = after_left.replacen(right, updated_right, 1);
        let edits = [
            PositionEdit {
                start: position_at(&source, left_start),
                end: position_at(&source, left_start + left.len()),
                new_text: updated_left.to_string(),
            },
            PositionEdit {
                start: position_at(&after_left, right_start),
                end: position_at(&after_left, right_start + right.len()),
                new_text: updated_right.to_string(),
            },
        ];

        let edited = vfs.apply_position_edits(&file, &edits).unwrap();
        assert_eq!(edited.source, expected, "edit order failed for {source:?}");
        assert!(edited.dirty);

        let committed = vfs.commit_file(&file).unwrap();
        let repeated_commit = vfs.commit_file(&file).unwrap();
        assert_eq!(committed, repeated_commit, "commit must be idempotent");
        assert_eq!(fs::read_to_string(&file).unwrap(), expected);

        let revert_start = expected.find(updated_left).unwrap();
        vfs.apply_edit(&file, revert_start, revert_start + updated_left.len(), left)
            .unwrap();
        let discarded = vfs.discard_file(&file).unwrap();
        let repeated_discard = vfs.discard_file(&file).unwrap();
        assert_eq!(discarded, repeated_discard, "discard must be idempotent");
        assert_eq!(discarded.source, expected);
        assert!(!discarded.dirty);
    }
}

#[test]
fn rejects_byte_edit_inside_utf8_character() {
    let file = temp_file("def value() -> str:\n    return 'é'\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&file).unwrap();
    let character_start = snapshot.source.find('é').unwrap();
    let interior_byte = character_start + 1;
    let error = vfs
        .apply_edit(&file, interior_byte, interior_byte, "x")
        .expect_err("byte edits must not split UTF-8 characters");

    assert!(
        error
            .to_string()
            .contains("edit range must align to UTF-8 character boundaries")
    );
    let unchanged = vfs.read_file(&file).unwrap();
    assert!(!unchanged.dirty);
    assert_eq!(unchanged.source, snapshot.source);
}

#[test]
fn empty_position_edits_report_current_syntax_errors() {
    let file = temp_file("def value(\n");
    let mut vfs = VirtualFileSystem::new();

    let result = vfs.apply_position_edits(&file, &[]).unwrap();

    assert!(result.incremental_parse);
    assert!(!result.validation.syntax_errors.is_empty());
    assert!(result.validation.resolved_identifiers.is_empty());
    assert_eq!(result.validation.commit_gate.status, "not_evaluated");
}

#[test]
fn rolls_back_position_edits_when_later_edit_fails() {
    let file = temp_file("def value() -> int:\n    return 10\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let error = vfs
        .apply_position_edits(
            &file,
            &[
                PositionEdit {
                    start: Position { row: 1, column: 11 },
                    end: Position { row: 1, column: 13 },
                    new_text: "20".to_string(),
                },
                PositionEdit {
                    start: Position { row: 99, column: 0 },
                    end: Position { row: 99, column: 0 },
                    new_text: "# bad\n".to_string(),
                },
            ],
        )
        .expect_err("later edit failure should reject the whole batch");

    assert!(
        error
            .to_string()
            .contains("failed to apply position edit at index 1")
    );
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
}

#[test]
fn rolls_back_position_edits_when_later_edit_splits_utf8_character() {
    let file = temp_file("def value() -> str:\n    return 'é'\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let error = vfs
        .apply_position_edits(
            &file,
            &[
                PositionEdit {
                    start: Position { row: 0, column: 0 },
                    end: Position { row: 0, column: 0 },
                    new_text: "# staged\n".to_string(),
                },
                PositionEdit {
                    start: Position { row: 2, column: 13 },
                    end: Position { row: 2, column: 13 },
                    new_text: "x".to_string(),
                },
            ],
        )
        .expect_err("position edits must not split UTF-8 characters");

    assert!(
        error
            .to_string()
            .contains("failed to apply position edit at index 1")
    );
    let error_chain = format!("{error:#}");
    assert!(error_chain.contains("does not align to a UTF-8 character boundary"));
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
}
