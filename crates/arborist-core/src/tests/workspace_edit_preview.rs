use std::fs;

use super::support::temporary_dir;
use crate::{Position, PositionEdit, WorkspacePositionEdits, preview_workspace_position_edits};

#[test]
fn previews_multiple_position_edit_files_without_writing_to_disk() {
    let dir = temporary_dir();
    let first = dir.join("first.py");
    let second = dir.join("second.py");
    fs::write(&first, "def first() -> int:\n    return 1\n").unwrap();
    fs::write(&second, "def second() -> int:\n    return 2\n").unwrap();

    let result = preview_workspace_position_edits(&[
        WorkspacePositionEdits {
            file_path: first.display().to_string(),
            source: None,
            edits: vec![PositionEdit {
                start: Position { row: 1, column: 11 },
                end: Position { row: 1, column: 12 },
                new_text: "10".to_string(),
            }],
        },
        WorkspacePositionEdits {
            file_path: second.display().to_string(),
            source: None,
            edits: vec![PositionEdit {
                start: Position { row: 1, column: 11 },
                end: Position { row: 1, column: 12 },
                new_text: "20".to_string(),
            }],
        },
    ])
    .unwrap();

    assert!(result.changed);
    assert_eq!(result.files.len(), 2);
    assert!(result.files.iter().all(|file| file.changed));
    assert!(
        result
            .files
            .iter()
            .all(|file| file.validation.syntax_errors.is_empty())
    );
    assert!(result.files[0].source.contains("return 10"));
    assert!(result.files[1].source.contains("return 20"));
    assert!(
        result
            .files
            .iter()
            .all(|file| !file.unified_diff.is_empty())
    );
    assert_eq!(
        fs::read_to_string(&first).unwrap(),
        "def first() -> int:\n    return 1\n"
    );
    assert_eq!(
        fs::read_to_string(&second).unwrap(),
        "def second() -> int:\n    return 2\n"
    );
}

#[test]
fn rejects_invalid_position_edits_without_writing_any_file() {
    let dir = temporary_dir();
    let first = dir.join("first.py");
    let second = dir.join("second.py");
    fs::write(&first, "def first() -> int:\n    return 1\n").unwrap();
    fs::write(&second, "def second() -> int:\n    return 2\n").unwrap();

    let error = preview_workspace_position_edits(&[
        WorkspacePositionEdits {
            file_path: first.display().to_string(),
            source: None,
            edits: vec![PositionEdit {
                start: Position { row: 1, column: 11 },
                end: Position { row: 1, column: 12 },
                new_text: "10".to_string(),
            }],
        },
        WorkspacePositionEdits {
            file_path: second.display().to_string(),
            source: None,
            edits: vec![PositionEdit {
                start: Position { row: 9, column: 0 },
                end: Position { row: 9, column: 1 },
                new_text: "20".to_string(),
            }],
        },
    ])
    .expect_err("out-of-range edits should be rejected");

    assert!(error.to_string().contains("position edit at index 0"));
    assert_eq!(
        fs::read_to_string(&first).unwrap(),
        "def first() -> int:\n    return 1\n"
    );
    assert_eq!(
        fs::read_to_string(&second).unwrap(),
        "def second() -> int:\n    return 2\n"
    );
}
