use std::fs;
use std::path::Path;

use super::{
    MAX_PATCH_PREVIEW_TIMEOUT_MS, Position,
    preview_patch_ast_node_at_position_from_path_with_timeout,
    preview_patch_ast_node_at_position_with_timeout, preview_patch_ast_node_from_path_with_timeout,
    preview_patch_ast_node_with_timeout, temporary_dir,
};
use crate::language::MAX_SOURCE_FILE_BYTES;

const SOURCE: &str = "def sample():\n    return 1\n";
const REPLACEMENT: &str = "def sample():\n    return 2\n";

#[test]
fn patch_preview_rejects_zero_timeout_before_path_or_source_work() {
    let position = Position { row: 0, column: 4 };
    let errors = [
        preview_patch_ast_node_with_timeout(
            Path::new(""),
            SOURCE,
            "sample",
            REPLACEMENT,
            None,
            Some(0),
        )
        .expect_err("inline semantic preview should reject zero timeout"),
        preview_patch_ast_node_from_path_with_timeout(
            Path::new(""),
            "sample",
            REPLACEMENT,
            None,
            Some(0),
        )
        .expect_err("path semantic preview should reject zero timeout"),
        preview_patch_ast_node_at_position_with_timeout(
            Path::new(""),
            SOURCE,
            &position,
            REPLACEMENT,
            None,
            Some(0),
        )
        .expect_err("inline position preview should reject zero timeout"),
        preview_patch_ast_node_at_position_from_path_with_timeout(
            Path::new(""),
            &position,
            REPLACEMENT,
            None,
            Some(0),
        )
        .expect_err("path position preview should reject zero timeout"),
    ];

    for error in errors {
        assert!(
            error
                .to_string()
                .contains("invalid patch preview timeout_ms: value must be greater than zero")
        );
    }
}

#[test]
fn patch_preview_rejects_excessive_timeout() {
    let error = preview_patch_ast_node_with_timeout(
        Path::new("sample.py"),
        SOURCE,
        "sample",
        REPLACEMENT,
        None,
        Some(MAX_PATCH_PREVIEW_TIMEOUT_MS + 1),
    )
    .expect_err("excessive timeout should fail");

    assert!(
        error
            .to_string()
            .contains(&format!("must not exceed {MAX_PATCH_PREVIEW_TIMEOUT_MS}"))
    );
}

#[test]
fn patch_preview_timeout_variants_preserve_semantic_and_position_results() {
    let semantic = preview_patch_ast_node_with_timeout(
        Path::new("sample.py"),
        SOURCE,
        "sample",
        REPLACEMENT,
        None,
        Some(MAX_PATCH_PREVIEW_TIMEOUT_MS),
    )
    .expect("semantic preview should succeed");
    let position = preview_patch_ast_node_at_position_with_timeout(
        Path::new("sample.py"),
        SOURCE,
        &Position { row: 0, column: 4 },
        REPLACEMENT,
        None,
        Some(MAX_PATCH_PREVIEW_TIMEOUT_MS),
    )
    .expect("position preview should succeed");

    assert!(semantic.changed);
    assert!(semantic.patch.applied);
    assert_eq!(semantic, position);
}

#[test]
fn timed_path_patch_previews_read_without_writing() {
    let workspace = temporary_dir();
    let path = workspace.join("sample.py");
    fs::write(&path, SOURCE).expect("fixture should be written");

    let semantic = preview_patch_ast_node_from_path_with_timeout(
        &path,
        "sample",
        REPLACEMENT,
        None,
        Some(MAX_PATCH_PREVIEW_TIMEOUT_MS),
    )
    .expect("semantic path preview should succeed");
    let position = preview_patch_ast_node_at_position_from_path_with_timeout(
        &path,
        &Position { row: 0, column: 4 },
        REPLACEMENT,
        None,
        Some(MAX_PATCH_PREVIEW_TIMEOUT_MS),
    )
    .expect("position path preview should succeed");

    assert_eq!(semantic, position);
    assert_eq!(
        fs::read_to_string(&path).expect("fixture should remain"),
        SOURCE
    );

    fs::remove_dir_all(workspace).expect("fixture directory should be removed");
}

#[test]
fn oversized_path_patch_preview_fails_closed_before_patch_work() {
    let workspace = temporary_dir();
    let path = workspace.join("sample.py");
    fs::write(&path, SOURCE).expect("fixture should be written");
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("fixture should be opened for truncation")
        .set_len(MAX_SOURCE_FILE_BYTES + 1)
        .expect("fixture should be resized past the source limit");

    let error = preview_patch_ast_node_from_path_with_timeout(
        &path,
        "sample",
        REPLACEMENT,
        None,
        Some(MAX_PATCH_PREVIEW_TIMEOUT_MS),
    )
    .expect_err("path preview should reject oversized sources");

    let message = format!("{error:#}");
    assert!(
        message.contains("source text too large") || message.contains("source file too large"),
        "unexpected error: {message}"
    );

    fs::remove_dir_all(workspace).expect("fixture directory should be removed");
}

#[test]
fn timed_c_patch_preview_preserves_reference_validation() {
    let source = "int helper(void) { return 1; }\nint sample(void) { return helper(); }\n";
    let replacement = "int sample(void) { return helper() + 1; }";

    let preview = preview_patch_ast_node_with_timeout(
        Path::new("sample.c"),
        source,
        "sample",
        replacement,
        None,
        Some(MAX_PATCH_PREVIEW_TIMEOUT_MS),
    )
    .expect("timed C preview should succeed");

    assert!(preview.changed);
    assert!(preview.patch.applied);
    assert!(
        preview
            .patch
            .validation
            .resolved_identifiers
            .iter()
            .any(|binding| binding.name == "helper")
    );
}
