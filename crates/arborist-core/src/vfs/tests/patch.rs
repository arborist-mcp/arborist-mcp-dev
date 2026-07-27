#[test]
fn read_context_validation_rejects_zero_timeout_before_virtual_file_work() {
    let mut vfs = VirtualFileSystem::new();
    let path = Path::new("");
    let position = Position { row: 0, column: 0 };
    let replacement = "def target():
    return 2
";
    let errors = [
        vfs.validate_patch_with_neighborhood_context_with_timeout(
            path,
            path,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("virtual neighborhood context should reject zero timeout"),
        vfs.validate_patch_with_neighborhood_context_at_position_with_timeout(
            path,
            path,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("virtual position neighborhood context should reject zero timeout"),
        vfs.validate_patch_with_discovery_context_with_timeout(
            path,
            path,
            "target",
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("virtual discovery context should reject zero timeout"),
        vfs.validate_patch_with_discovery_context_at_position_with_timeout(
            path,
            path,
            &position,
            replacement,
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("virtual position discovery context should reject zero timeout"),
    ];

    for error in errors {
        assert!(
            error
                .to_string()
                .contains("invalid trace timeout_ms: value must be greater than zero")
        );
    }
}

#[test]
fn graph_context_validation_rejects_zero_timeout_before_virtual_file_work() {
    let mut vfs = VirtualFileSystem::new();
    let path = Path::new("");
    let position = Position { row: 0, column: 0 };
    let errors = [
        vfs.validate_patch_with_graph_context_with_timeout(
            path,
            path,
            "target",
            "def target():
    return 2
",
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("virtual graph context should reject zero timeout"),
        vfs.validate_patch_with_graph_context_at_position_with_timeout(
            path,
            path,
            &position,
            "def target():
    return 2
",
            None,
            TraceDirection::Both,
            2,
            10,
            Some(0),
        )
        .expect_err("virtual position graph context should reject zero timeout"),
    ];

    for error in errors {
        assert!(
            error
                .to_string()
                .contains("invalid trace timeout_ms: value must be greater than zero")
        );
    }
}

#[test]
fn trace_context_validation_rejects_zero_timeout_before_virtual_file_work() {
    let mut vfs = VirtualFileSystem::new();
    let path = Path::new("");
    let position = Position { row: 0, column: 0 };
    let errors = [
        vfs.validate_patch_with_trace_context_with_timeout(
            path,
            path,
            "target",
            "def target():
    return 2
",
            None,
            TraceDirection::Both,
            Some(0),
        )
        .expect_err("virtual trace context should reject zero timeout"),
        vfs.validate_patch_with_trace_context_at_position_with_timeout(
            path,
            path,
            &position,
            "def target():
    return 2
",
            None,
            TraceDirection::Both,
            Some(0),
        )
        .expect_err("virtual position trace context should reject zero timeout"),
    ];

    for error in errors {
        assert!(
            error
                .to_string()
                .contains("invalid trace timeout_ms: value must be greater than zero")
        );
    }
}

use super::*;

#[test]
fn patches_virtual_symbol_without_immediate_commit() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let result = vfs
        .patch_node(&file, "value", "def value() -> int:\n    return 3\n", None)
        .unwrap();

    assert!(result.applied);
    let snapshot = vfs.read_file(&file).unwrap();
    assert!(snapshot.dirty);
    assert!(snapshot.source.contains("return 3"));
    assert!(fs::read_to_string(&file).unwrap().contains("return 1"));
}

#[test]
fn patches_virtual_symbol_at_position_without_immediate_commit() {
    let file = temp_file(
        "def decorator(func):\n    return func\n\n@decorator\ndef value() -> int:\n    return 1\n",
    );
    let mut vfs = VirtualFileSystem::new();

    let result = vfs
        .patch_node_at_position(
            &file,
            &Position { row: 3, column: 1 },
            "def value() -> int:\n    return 3\n",
            None,
        )
        .unwrap();

    assert!(!result.applied);
    assert_eq!(result.resolved_path, "value");
    assert!(
        result
            .validation
            .syntax_errors
            .iter()
            .any(|issue| issue.kind == "decorator_guard")
    );
    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert!(snapshot.source.contains("@decorator"));
    assert!(snapshot.source.contains("return 1"));
    assert!(fs::read_to_string(&file).unwrap().contains("@decorator"));
}

#[test]
fn rejects_blank_virtual_patch_without_dirtying_buffer() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let error = vfs
        .patch_node(&file, "value", " \t", None)
        .expect_err("blank virtual patch replacements should be rejected");

    assert!(error.to_string().contains("new_code"));
    assert!(error.to_string().contains("blank"));
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
}

#[test]
fn rejects_blank_virtual_patch_bypass_without_dirtying_buffer() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let error = vfs
        .patch_node(
            &file,
            "value",
            "def value() -> int:\n    return 2\n",
            Some(" \t"),
        )
        .expect_err("blank virtual patch bypass reasons should be rejected");

    assert!(error.to_string().contains("bypass_reason"));
    assert!(error.to_string().contains("blank"));
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
}

#[test]
fn rolls_back_invalid_virtual_patch() {
    let file = temp_file(
        "def helper(value: int) -> int:\n    return value + 1\n\ndef value() -> int:\n    return helper(1)\n",
    );
    let mut vfs = VirtualFileSystem::new();

    let result = vfs
        .patch_node(
            &file,
            "value",
            "def value() -> int:\n    return missing_helper(1)\n",
            None,
        )
        .unwrap();

    assert!(!result.applied);
    assert_eq!(
        result.validation.unresolved_identifiers,
        vec!["missing_helper"]
    );

    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert!(snapshot.source.contains("return helper(1)"));
}

#[test]
fn rolls_back_virtual_patch_when_validation_errors() {
    let workspace = temp_workspace();
    let file = workspace.join("sample.c");
    let bad_include = workspace.join("bad.txt");
    fs::write(&bad_include, "int helper(void);\n").unwrap();
    fs::write(
        &file,
        "#include \"bad.txt\"\n\nint value(void) {\n    return 1;\n}\n",
    )
    .unwrap();
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let error = vfs
        .patch_node(
            &file,
            "value",
            "int value(void) {\n    return helper();\n}\n",
            None,
        )
        .expect_err("validation errors should reject the virtual patch");

    assert!(
        error
            .to_string()
            .contains("failed to validate virtual patch")
    );
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
}
