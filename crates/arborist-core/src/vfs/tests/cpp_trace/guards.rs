use super::*;

#[test]
fn trace_patch_context_uses_unsaved_workspace_overrides() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");
    let consumer_path = workspace.join("consumer.py");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &consumer_path,
        "def consume(value: int) -> int:\n    return value\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &consumer_path,
        Some(
            "from caller import orchestrate\n\n\ndef consume(value: int) -> int:\n    return orchestrate(value)\n",
        ),
    )
    .unwrap();

    let result = vfs
        .validate_patch_with_trace_context(
            &workspace,
            &caller_path,
            "orchestrate",
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
            None,
            TraceDirection::Both,
        )
        .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.trace_target, result.patch.resolved_symbol_id);
    assert!(result.trace_error.is_none());
    assert_eq!(
        result
            .trace_validation
            .as_ref()
            .map(|validation| validation.allowed),
        Some(true)
    );

    let trace = result.trace.expect("trace result should be present");
    assert!(
        trace
            .callees
            .iter()
            .find(|symbol| symbol.semantic_path == "helper")
            .is_some()
    );
    assert!(
        trace
            .callers
            .iter()
            .find(|symbol| symbol.semantic_path == "consume")
            .is_some()
    );

    let consumer_snapshot = vfs.read_file(&consumer_path).unwrap();
    assert!(consumer_snapshot.dirty);
    assert!(
        consumer_snapshot
            .source
            .contains("return orchestrate(value)")
    );
    let consumer_disk = fs::read_to_string(&consumer_path).unwrap();
    assert!(consumer_disk.contains("return value"));
    assert!(!consumer_disk.contains("return orchestrate(value)"));
}

#[test]
fn trace_patch_context_rejects_unresolved_crlf_patch_bindings() {
    let workspace = temp_workspace();
    let caller_path = workspace.join("caller.py");
    let original_source = "def orchestrate(value: int) -> int:\r\n    return value + 1\r\n";

    fs::write(&caller_path, original_source).unwrap();

    let mut vfs = VirtualFileSystem::new();
    let result = vfs
        .validate_patch_with_trace_context(
            &workspace,
            &caller_path,
            "orchestrate",
            "def orchestrate(value: int) -> int:\n    return missing_helper(value)\n",
            None,
            TraceDirection::Both,
        )
        .unwrap();

    assert!(!result.patch.applied);
    assert_eq!(result.patch.validation.commit_gate.status, "rejected");
    assert_eq!(
        result.patch.validation.unresolved_identifiers,
        vec!["missing_helper"]
    );
    assert!(result.trace.is_none());
    assert!(result.trace_validation.is_none());
    assert_eq!(
        result.trace_error.as_deref(),
        Some("trace skipped because patch validation rejected the patch")
    );

    let snapshot = vfs.read_file(&caller_path).unwrap();
    assert_eq!(snapshot.source, original_source);
    assert!(!snapshot.dirty);
}

#[test]
fn trace_symbol_graph_ignores_virtual_files_in_skipped_dirs() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let venv_path = workspace.join("VENV").join("installed.py");

    fs::create_dir_all(venv_path.parent().unwrap()).unwrap();
    fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&venv_path, Some("def installed() -> int:\n    return 2\n"))
        .unwrap();

    assert!(
        vfs.trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
            .is_ok()
    );
    assert!(
        vfs.trace_symbol_graph(&workspace, "installed", TraceDirection::Both)
            .is_err()
    );
}

#[test]
fn trace_symbol_graph_ignores_virtual_files_in_sibling_workspace_prefix() {
    let dir = temp_workspace();
    let workspace = dir.join("project");
    let sibling = dir.join("project-extra");
    let helper_path = workspace.join("helper.py");
    let sibling_path = sibling.join("installed.py");

    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&sibling).unwrap();
    fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &sibling_path,
        Some("def installed() -> int:\n    return 2\n"),
    )
    .unwrap();

    assert!(
        vfs.trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
            .is_ok()
    );
    assert!(
        vfs.trace_symbol_graph(&workspace, "installed", TraceDirection::Both)
            .is_err()
    );
}
