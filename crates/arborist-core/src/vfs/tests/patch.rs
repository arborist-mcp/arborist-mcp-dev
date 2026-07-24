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
