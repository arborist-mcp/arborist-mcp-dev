use super::*;

#[test]
fn virtual_commit_binding_forwards_timeout_and_preserves_legacy_default() {
    prepare_python();

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!(
        "arborist-py-virtual-commit-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&workspace).unwrap();
    let file = workspace.join("sample.py");
    fs::write(&file, "value = 1\n").unwrap();
    let file_path = file.to_string_lossy();
    let core = ArboristCore::new();

    let zero = core
        .commit_virtual_file_json_impl(&file_path, Some(0))
        .expect_err("zero timeout should reach core validation");
    assert!(
        zero.to_string()
            .contains("invalid virtual file commit timeout_ms: value must be greater than zero")
    );

    core.open_virtual_file_json_impl(&file_path, Some("value = 2\n".to_string()), None)
        .expect("virtual file should open with a dirty source");
    let timed: Value = serde_json::from_str(
        &core
            .commit_virtual_file_json_impl(&file_path, Some(MAX_VIRTUAL_FILE_COMMIT_TIMEOUT_MS))
            .expect("timed virtual commit should succeed"),
    )
    .expect("timed virtual commit result should be valid JSON");
    assert_eq!(timed["dirty"], false);
    assert_eq!(fs::read_to_string(&file).unwrap(), "value = 2\n");

    core.open_virtual_file_json_impl(&file_path, Some("value = 3\n".to_string()), None)
        .expect("virtual file should reopen with another dirty source");
    let legacy: Value = serde_json::from_str(
        &core
            .commit_virtual_file_json_impl(&file_path, None)
            .expect("commit without a timeout should remain supported"),
    )
    .expect("legacy virtual commit result should be valid JSON");
    assert_eq!(legacy["dirty"], false);
    assert_eq!(fs::read_to_string(&file).unwrap(), "value = 3\n");

    fs::remove_dir_all(workspace).unwrap();
}
#[test]
fn virtual_discard_and_close_bindings_forward_timeouts_and_preserve_defaults() {
    prepare_python();

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!(
        "arborist-py-virtual-lifecycle-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&workspace).unwrap();
    let file = workspace.join("sample.py");
    fs::write(&file, "value = 1\n").unwrap();
    let file_path = file.to_string_lossy();
    let core = ArboristCore::new();

    let discard_zero = core
        .discard_virtual_file_json_impl(&file_path, Some(0))
        .expect_err("zero discard timeout should reach core validation");
    assert!(
        discard_zero
            .to_string()
            .contains("invalid virtual file discard timeout_ms: value must be greater than zero")
    );
    let close_zero = core
        .close_virtual_file_json_impl(&file_path, true, Some(0))
        .expect_err("zero close timeout should reach core validation");
    assert!(
        close_zero
            .to_string()
            .contains("invalid virtual file close timeout_ms: value must be greater than zero")
    );

    core.open_virtual_file_json_impl(&file_path, Some("value = 2\n".to_string()), None)
        .expect("virtual file should open with a dirty source");
    let discarded: Value = serde_json::from_str(
        &core
            .discard_virtual_file_json_impl(&file_path, Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS))
            .expect("timed virtual discard should succeed"),
    )
    .expect("timed virtual discard result should be valid JSON");
    assert_eq!(discarded["dirty"], false);
    assert_eq!(discarded["source"], "value = 1\n");
    assert_eq!(fs::read_to_string(&file).unwrap(), "value = 1\n");

    core.open_virtual_file_json_impl(&file_path, Some("value = 3\n".to_string()), None)
        .expect("virtual file should reopen with a dirty source");
    let persisted: Value = serde_json::from_str(
        &core
            .close_virtual_file_json_impl(
                &file_path,
                true,
                Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS),
            )
            .expect("timed persisted close should succeed"),
    )
    .expect("timed persisted close result should be valid JSON");
    assert_eq!(persisted["dirty"], false);
    assert_eq!(fs::read_to_string(&file).unwrap(), "value = 3\n");

    core.open_virtual_file_json_impl(&file_path, Some("value = 4\n".to_string()), None)
        .expect("virtual file should reopen for legacy close");
    let legacy: Value = serde_json::from_str(
        &core
            .close_virtual_file_json_impl(&file_path, false, None)
            .expect("close without a timeout should remain supported"),
    )
    .expect("legacy close result should be valid JSON");
    assert_eq!(legacy["dirty"], false);
    assert_eq!(legacy["source"], "value = 3\n");
    assert_eq!(fs::read_to_string(&file).unwrap(), "value = 3\n");

    let statuses: Value = serde_json::from_str(
        &core
            .list_virtual_files_json_impl(false, None)
            .expect("virtual file status list should serialize"),
    )
    .expect("virtual file status list should be valid JSON");
    assert_eq!(statuses, Value::Array(Vec::new()));

    fs::remove_dir_all(workspace).unwrap();
}
#[test]
fn virtual_read_bindings_forward_timeouts_and_preserve_defaults() {
    prepare_python();

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!(
        "arborist-py-virtual-read-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&workspace).unwrap();
    let file = workspace.join("sample.py");
    fs::write(&file, "value = 1\n").unwrap();
    let file_path = file.to_string_lossy();
    let core = ArboristCore::new();

    let open_zero = core
        .open_virtual_file_json_impl(&file_path, None, Some(0))
        .expect_err("zero open timeout should reach core validation");
    assert!(
        open_zero
            .to_string()
            .contains("invalid virtual file open timeout_ms: value must be greater than zero")
    );
    let read_zero = core
        .read_virtual_file_json_impl(&file_path, Some(0))
        .expect_err("zero read timeout should reach core validation");
    assert!(
        read_zero
            .to_string()
            .contains("invalid virtual file read timeout_ms: value must be greater than zero")
    );
    let list_zero = core
        .list_virtual_files_json_impl(false, Some(0))
        .expect_err("zero listing timeout should reach core validation");
    assert!(
        list_zero
            .to_string()
            .contains("invalid virtual file listing timeout_ms: value must be greater than zero")
    );

    let opened: Value = serde_json::from_str(
        &core
            .open_virtual_file_json_impl(
                &file_path,
                Some("value = 2\n".to_string()),
                Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS),
            )
            .expect("timed virtual open should succeed"),
    )
    .expect("timed virtual open result should be valid JSON");
    assert_eq!(opened["dirty"], true);
    assert_eq!(opened["source"], "value = 2\n");

    let statuses: Value = serde_json::from_str(
        &core
            .list_virtual_files_json_impl(true, Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS))
            .expect("timed virtual listing should succeed"),
    )
    .expect("timed virtual listing result should be valid JSON");
    assert_eq!(statuses.as_array().unwrap().len(), 1);
    assert_eq!(statuses[0]["dirty"], true);

    core.discard_virtual_file_json_impl(&file_path, None)
        .expect("discard should restore the disk source");
    fs::write(&file, "value = 3\n").unwrap();
    let read: Value = serde_json::from_str(
        &core
            .read_virtual_file_json_impl(&file_path, Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS))
            .expect("timed virtual read should succeed"),
    )
    .expect("timed virtual read result should be valid JSON");
    assert_eq!(read["dirty"], false);
    assert_eq!(read["source"], "value = 3\n");

    let legacy: Value = serde_json::from_str(
        &core
            .list_virtual_files_json_impl(false, None)
            .expect("listing without a timeout should remain supported"),
    )
    .expect("legacy virtual listing result should be valid JSON");
    assert_eq!(legacy.as_array().unwrap().len(), 1);

    fs::remove_dir_all(workspace).unwrap();
}
#[test]
fn virtual_edit_bindings_forward_timeouts_and_preserve_defaults() {
    prepare_python();

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!(
        "arborist-py-virtual-edit-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&workspace).unwrap();
    let file = workspace.join("sample.py");
    fs::write(&file, "value = 12\n").unwrap();
    let file_path = file.to_string_lossy();
    let core = ArboristCore::new();

    let byte_zero = core
        .apply_buffer_edit_json_impl(&file_path, 8, 9, "3", Some(0))
        .expect_err("zero byte-edit timeout should reach core validation");
    assert!(
        byte_zero
            .to_string()
            .contains("invalid virtual buffer edit timeout_ms: value must be greater than zero")
    );
    let position_zero = core
        .apply_position_edits_json_impl(&file_path, "[]", Some(0))
        .expect_err("zero position-edit timeout should reach core validation");
    assert!(
        position_zero
            .to_string()
            .contains("invalid virtual position edits timeout_ms: value must be greater than zero")
    );

    let byte: Value = serde_json::from_str(
        &core
            .apply_buffer_edit_json_impl(
                &file_path,
                8,
                9,
                "3",
                Some(MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS),
            )
            .expect("timed byte edit should succeed"),
    )
    .expect("timed byte-edit result should be valid JSON");
    assert_eq!(byte["source"], "value = 32\n");
    assert_eq!(byte["version"], 1);

    let position: Value = serde_json::from_str(
        &core
            .apply_position_edits_json_impl(
                &file_path,
                r#"[{"start":{"row":0,"column":9},"end":{"row":0,"column":10},"new_text":"4"}]"#,
                Some(MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS),
            )
            .expect("timed position edit should succeed"),
    )
    .expect("timed position-edit result should be valid JSON");
    assert_eq!(position["source"], "value = 34\n");
    assert_eq!(position["version"], 2);

    let legacy_byte: Value = serde_json::from_str(
        &core
            .apply_buffer_edit_json_impl(&file_path, 8, 10, "56", None)
            .expect("byte edit without a timeout should remain supported"),
    )
    .expect("legacy byte-edit result should be valid JSON");
    assert_eq!(legacy_byte["source"], "value = 56\n");

    let legacy_position: Value = serde_json::from_str(
        &core
            .apply_position_edits_json_impl(
                &file_path,
                r#"[{"start":{"row":0,"column":8},"end":{"row":0,"column":9},"new_text":"7"}]"#,
                None,
            )
            .expect("position edit without a timeout should remain supported"),
    )
    .expect("legacy position-edit result should be valid JSON");
    assert_eq!(legacy_position["source"], "value = 76\n");
    assert_eq!(legacy_position["version"], 4);

    fs::remove_dir_all(workspace).unwrap();
}
