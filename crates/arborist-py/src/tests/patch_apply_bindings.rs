use super::*;

#[test]
fn patch_preview_bindings_forward_zero_timeout_before_path_work() {
    prepare_python();

    let core = ArboristCore::new();
    let source = Some("def target():\n    return 1\n".to_string());
    let errors = [
        core.preview_patch_ast_node_json_impl(
            "",
            "target",
            "def target():\n    return 2\n",
            source.clone(),
            None,
            Some(0),
        )
        .expect_err("semantic preview should reject zero timeout"),
        core.preview_patch_ast_node_at_position_json_impl(
            "",
            0,
            4,
            "def target():\n    return 2\n",
            source,
            None,
            Some(0),
        )
        .expect_err("position preview should reject zero timeout"),
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
fn patch_bindings_forward_zero_timeout_before_path_or_vfs_work() {
    prepare_python();

    let core = ArboristCore::new();
    let source = Some("def target():\n    return 1\n".to_string());
    let replacement = "def target():\n    return 2\n";
    let errors = [
        core.patch_ast_node_json_impl("", "target", replacement, source.clone(), None, Some(0))
            .expect_err("source semantic patch should reject zero timeout"),
        core.patch_ast_node_json_impl("", "target", replacement, None, None, Some(0))
            .expect_err("committed semantic patch should reject zero timeout"),
        core.patch_ast_node_at_position_json_impl("", 0, 4, replacement, source, None, Some(0))
            .expect_err("source position patch should reject zero timeout"),
        core.patch_ast_node_at_position_json_impl("", 0, 4, replacement, None, None, Some(0))
            .expect_err("committed position patch should reject zero timeout"),
        core.patch_virtual_ast_node_json_impl("", "target", replacement, None, Some(0))
            .expect_err("virtual semantic patch should reject zero timeout"),
        core.patch_virtual_ast_node_at_position_json_impl("", 0, 4, replacement, None, Some(0))
            .expect_err("virtual position patch should reject zero timeout"),
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
fn committed_patch_binding_writes_with_shared_timeout_budget() {
    prepare_python();

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let workspace =
        std::env::temp_dir().join(format!("arborist-py-patch-{}-{suffix}", std::process::id()));
    fs::create_dir_all(&workspace).unwrap();
    let file = workspace.join("sample.py");
    fs::write(&file, "def target():\n    return 1\n").unwrap();
    let core = ArboristCore::new();

    let result: Value = serde_json::from_str(
        &core
            .patch_ast_node_json_impl(
                &file.to_string_lossy(),
                "target",
                "def target():\n    return 2\n",
                None,
                None,
                Some(MAX_PATCH_TIMEOUT_MS),
            )
            .expect("committed patch binding should succeed"),
    )
    .expect("committed patch result should be valid JSON");

    assert_eq!(result["applied"], true);
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        result["updated_source"].as_str().unwrap()
    );
    fs::remove_dir_all(workspace).unwrap();
}
#[test]
fn virtual_file_and_source_preview_dispatch_preserve_results() {
    prepare_python();

    let core = ArboristCore::new();
    let source = "def target() -> int:\n    return 1\n";
    let replacement = "def target() -> int:\n    return 2\n";
    let expected_virtual_source = "def target() -> int:\n    return 2\n\n";
    let opened: Value = serde_json::from_str(
        &core
            .open_virtual_file_json_impl("memory.py", Some(source.to_string()), None)
            .expect("virtual file should open"),
    )
    .expect("virtual file result should be valid JSON");
    let patched: Value = serde_json::from_str(
        &core
            .patch_virtual_ast_node_json_impl(
                "memory.py",
                "target",
                replacement,
                None,
                Some(MAX_PATCH_TIMEOUT_MS),
            )
            .expect("virtual patch should apply"),
    )
    .expect("virtual patch result should be valid JSON");
    let read: Value = serde_json::from_str(
        &core
            .read_virtual_file_json_impl("memory.py", None)
            .expect("virtual file should be readable"),
    )
    .expect("virtual file read should be valid JSON");
    let preview: Value = serde_json::from_str(
        &core
            .preview_patch_ast_node_json_impl(
                "memory.py",
                "target",
                replacement,
                Some(source.to_string()),
                None,
                None,
            )
            .expect("source-backed patch preview should succeed"),
    )
    .expect("patch preview result should be valid JSON");

    assert_eq!(opened["source"], source);
    assert_eq!(patched["applied"], true);
    assert_eq!(read["source"], expected_virtual_source);
    assert_eq!(read["dirty"], true);
    assert_eq!(preview["changed"], true);
    assert_eq!(preview["patch"]["applied"], true);
    assert_eq!(preview["patch"]["updated_source"], expected_virtual_source);
}
