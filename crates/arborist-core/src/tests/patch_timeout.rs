use std::fs;
use std::path::Path;

use super::{
    MAX_PATCH_TIMEOUT_MS, Position, patch_ast_node_at_position_from_path_with_timeout,
    patch_ast_node_at_position_with_timeout, patch_ast_node_from_path_with_timeout,
    patch_ast_node_with_timeout, temporary_dir,
};

const SOURCE: &str = "def sample():\n    return 1\n";
const REPLACEMENT: &str = "def sample():\n    return 2\n";

#[test]
fn patch_rejects_zero_timeout_before_path_or_source_work() {
    let position = Position { row: 0, column: 4 };
    let errors = [
        patch_ast_node_with_timeout(Path::new(""), SOURCE, "sample", REPLACEMENT, None, Some(0))
            .expect_err("inline semantic patch should reject zero timeout"),
        patch_ast_node_from_path_with_timeout(Path::new(""), "sample", REPLACEMENT, None, Some(0))
            .expect_err("path semantic patch should reject zero timeout"),
        patch_ast_node_at_position_with_timeout(
            Path::new(""),
            SOURCE,
            &position,
            REPLACEMENT,
            None,
            Some(0),
        )
        .expect_err("inline position patch should reject zero timeout"),
        patch_ast_node_at_position_from_path_with_timeout(
            Path::new(""),
            &position,
            REPLACEMENT,
            None,
            Some(0),
        )
        .expect_err("path position patch should reject zero timeout"),
    ];

    for error in errors {
        assert!(
            error
                .to_string()
                .contains("invalid patch timeout_ms: value must be greater than zero")
        );
    }
}

#[test]
fn patch_rejects_excessive_timeout() {
    let error = patch_ast_node_with_timeout(
        Path::new("sample.py"),
        SOURCE,
        "sample",
        REPLACEMENT,
        None,
        Some(MAX_PATCH_TIMEOUT_MS + 1),
    )
    .expect_err("excessive timeout should fail");

    assert!(
        error
            .to_string()
            .contains(&format!("must not exceed {MAX_PATCH_TIMEOUT_MS}"))
    );
}

#[test]
fn timed_inline_patch_variants_preserve_semantic_and_position_results() {
    let semantic = patch_ast_node_with_timeout(
        Path::new("sample.py"),
        SOURCE,
        "sample",
        REPLACEMENT,
        None,
        Some(MAX_PATCH_TIMEOUT_MS),
    )
    .expect("semantic patch should succeed");
    let position = patch_ast_node_at_position_with_timeout(
        Path::new("sample.py"),
        SOURCE,
        &Position { row: 0, column: 4 },
        REPLACEMENT,
        None,
        Some(MAX_PATCH_TIMEOUT_MS),
    )
    .expect("position patch should succeed");

    assert!(semantic.applied);
    assert_eq!(semantic, position);
}

#[test]
fn timed_path_patch_variants_write_python_source() {
    let workspace = temporary_dir();
    let semantic_path = workspace.join("semantic.py");
    let position_path = workspace.join("position.py");
    fs::write(&semantic_path, SOURCE).expect("semantic fixture should be written");
    fs::write(&position_path, SOURCE).expect("position fixture should be written");

    let semantic = patch_ast_node_from_path_with_timeout(
        &semantic_path,
        "sample",
        REPLACEMENT,
        None,
        Some(MAX_PATCH_TIMEOUT_MS),
    )
    .expect("semantic path patch should succeed");
    let position = patch_ast_node_at_position_from_path_with_timeout(
        &position_path,
        &Position { row: 0, column: 4 },
        REPLACEMENT,
        None,
        Some(MAX_PATCH_TIMEOUT_MS),
    )
    .expect("position path patch should succeed");

    assert_eq!(semantic.target_path, position.target_path);
    assert_eq!(semantic.resolved_path, position.resolved_path);
    assert_eq!(semantic.resolved_symbol_id, position.resolved_symbol_id);
    assert_eq!(semantic.updated_source, position.updated_source);
    assert_eq!(semantic.validation, position.validation);
    assert_eq!(
        fs::read_to_string(&semantic_path).expect("semantic fixture should remain"),
        semantic.updated_source
    );
    assert_eq!(
        fs::read_to_string(&position_path).expect("position fixture should remain"),
        position.updated_source
    );

    fs::remove_dir_all(workspace).expect("fixture directory should be removed");
}

#[test]
fn timed_blocked_path_patch_does_not_write_source() {
    let workspace = temporary_dir();
    let path = workspace.join("sample.py");
    fs::write(&path, SOURCE).expect("fixture should be written");

    let result = patch_ast_node_from_path_with_timeout(
        &path,
        "sample",
        "def sample(:\n    return 2\n",
        None,
        Some(MAX_PATCH_TIMEOUT_MS),
    )
    .expect("invalid replacement should return a blocked result");

    assert!(!result.applied);
    assert_eq!(
        fs::read_to_string(&path).expect("fixture should remain"),
        SOURCE
    );

    fs::remove_dir_all(workspace).expect("fixture directory should be removed");
}

#[test]
fn timed_c_patch_preserves_reference_validation() {
    let source = "int helper(void) { return 1; }\nint sample(void) { return helper(); }\n";
    let replacement = "int sample(void) { return helper() + 1; }";

    let result = patch_ast_node_with_timeout(
        Path::new("sample.c"),
        source,
        "sample",
        replacement,
        None,
        Some(MAX_PATCH_TIMEOUT_MS),
    )
    .expect("timed C patch should succeed");

    assert!(result.applied);
    assert!(
        result
            .validation
            .resolved_identifiers
            .iter()
            .any(|binding| binding.name == "helper")
    );
}
