#[test]
fn rejects_zero_timeout_before_workspace_edit_preview_work() {
    let error = preview_workspace_position_edits_with_timeout(&[], Some(0))
        .expect_err("zero timeout should be rejected before input validation");

    assert!(
        error
            .to_string()
            .contains("invalid workspace edit preview timeout_ms: value must be greater than zero")
    );
}

#[test]
fn rejects_excessive_workspace_edit_preview_timeout() {
    let error = preview_workspace_position_edits_with_timeout(
        &[],
        Some(crate::MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1),
    )
    .expect_err("oversized timeout should be rejected before input validation");

    assert!(
        error
            .to_string()
            .contains("invalid workspace edit preview timeout_ms: value must not exceed")
    );
}

use std::fs;

use super::support::temporary_dir;
use crate::{
    MAX_POSITION_EDIT_NEW_TEXT_BYTES, MAX_POSITION_EDIT_TEXT_BYTES, MAX_POSITION_EDITS,
    MAX_WORKSPACE_EDIT_PREVIEW_FILES, Position, PositionEdit, WorkspacePositionEdits,
    preview_workspace_position_edits, preview_workspace_position_edits_with_timeout,
};

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

#[test]
fn previews_unsaved_source_without_reading_or_writing_disk() {
    let dir = temporary_dir();
    let missing = dir.join("unsaved.py");
    let result = preview_workspace_position_edits(&[WorkspacePositionEdits {
        file_path: missing.display().to_string(),
        source: Some("value = 1\n".to_string()),
        edits: vec![PositionEdit {
            start: Position { row: 0, column: 8 },
            end: Position { row: 0, column: 9 },
            new_text: "2".to_string(),
        }],
    }])
    .unwrap();

    assert!(result.changed);
    assert_eq!(result.files[0].source, "value = 2\n");
    assert!(!missing.exists());
}

#[test]
fn rejects_too_many_position_edits_before_reading_source() {
    let dir = temporary_dir();
    let missing = dir.join("missing.py");
    let edit = PositionEdit {
        start: Position { row: 0, column: 0 },
        end: Position { row: 0, column: 0 },
        new_text: String::new(),
    };
    let edits = vec![edit; MAX_POSITION_EDITS + 1];

    let error = preview_workspace_position_edits(&[WorkspacePositionEdits {
        file_path: missing.display().to_string(),
        source: None,
        edits,
    }])
    .expect_err("too many position edits should be rejected");

    assert!(error.to_string().contains("workspace_edits[0].edits"));
    assert!(error.to_string().contains(&MAX_POSITION_EDITS.to_string()));
    assert!(!missing.exists());
}

#[test]
fn rejects_position_edit_text_budget_before_reading_source() {
    let dir = temporary_dir();
    let missing = dir.join("missing.py");
    let error = preview_workspace_position_edits(&[WorkspacePositionEdits {
        file_path: missing.display().to_string(),
        source: None,
        edits: vec![
            PositionEdit {
                start: Position { row: 0, column: 0 },
                end: Position { row: 0, column: 0 },
                new_text: "x".repeat(MAX_POSITION_EDIT_NEW_TEXT_BYTES),
            };
            (MAX_POSITION_EDIT_TEXT_BYTES / MAX_POSITION_EDIT_NEW_TEXT_BYTES) + 1
        ],
    }])
    .expect_err("replacement text beyond the batch budget should be rejected");

    assert!(error.to_string().contains("workspace_edits[0].edits"));
    assert!(
        error
            .to_string()
            .contains(&MAX_POSITION_EDIT_TEXT_BYTES.to_string())
    );
    assert!(!missing.exists());
}

#[test]
fn rejects_too_many_workspace_preview_files_before_reading_source() {
    let dir = temporary_dir();
    let missing = dir.join("missing.py");
    let request = WorkspacePositionEdits {
        file_path: missing.display().to_string(),
        source: None,
        edits: Vec::new(),
    };

    let error =
        preview_workspace_position_edits(&vec![request; MAX_WORKSPACE_EDIT_PREVIEW_FILES + 1])
            .expect_err("too many workspace preview files should be rejected");

    assert!(
        error
            .to_string()
            .contains("workspace edit preview accepts at most")
    );
    assert!(
        error
            .to_string()
            .contains(&MAX_WORKSPACE_EDIT_PREVIEW_FILES.to_string())
    );
    assert!(!missing.exists());
}

#[test]
fn applies_sequential_position_edits_against_updated_utf8_source() {
    let dir = temporary_dir();
    let missing = dir.join("sequential.py");
    let result = preview_workspace_position_edits(&[WorkspacePositionEdits {
        file_path: missing.display().to_string(),
        source: Some("value = \"é\"\n".to_string()),
        edits: vec![
            PositionEdit {
                start: Position { row: 0, column: 9 },
                end: Position { row: 0, column: 11 },
                new_text: "éé".to_string(),
            },
            PositionEdit {
                start: Position { row: 0, column: 11 },
                end: Position { row: 0, column: 13 },
                new_text: "x".to_string(),
            },
        ],
    }])
    .unwrap();

    assert_eq!(result.files[0].source, "value = \"éx\"\n");
    assert!(result.files[0].changed);
    assert!(!missing.exists());
}
