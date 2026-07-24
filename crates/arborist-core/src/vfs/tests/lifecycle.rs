use super::*;

#[test]
fn path_aliases_share_one_virtual_entry() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let alias_dir = file.parent().unwrap().join("child");
    fs::create_dir_all(&alias_dir).unwrap();
    let alias = alias_dir.join("..").join("buffer.py");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&alias).unwrap();
    assert!(!snapshot.file.contains("/../"));
    let digit_offset = snapshot.source.rfind('1').unwrap();

    vfs.apply_edit(&file, digit_offset, digit_offset + 1, "2")
        .unwrap();

    let statuses = vfs.virtual_file_statuses(false).unwrap();
    assert_eq!(statuses.len(), 1);
    assert!(statuses[0].dirty);

    let aliased_snapshot = vfs.read_file(&alias).unwrap();
    assert!(aliased_snapshot.source.contains("return 2"));

    let committed = vfs.commit_file(&alias).unwrap();
    assert!(!committed.dirty);
    assert!(fs::read_to_string(&file).unwrap().contains("return 2"));
}

#[test]
fn discards_virtual_changes() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&file).unwrap();
    let digit_offset = snapshot.source.rfind('1').unwrap();
    vfs.apply_edit(&file, digit_offset, digit_offset + 1, "9")
        .unwrap();
    let discarded = vfs.discard_file(&file).unwrap();

    assert!(!discarded.dirty);
    assert!(discarded.source.contains("return 1"));
}

#[test]
fn discarding_unchanged_file_is_idempotent() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let first = vfs.discard_file(&file).unwrap();
    let second = vfs.discard_file(&file).unwrap();

    assert_eq!(first, initial);
    assert_eq!(second, initial);
}

#[test]
fn discard_refreshes_from_current_disk_source() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    vfs.open_file(&file, Some("def value() -> int:\n    return 9\n"))
        .unwrap();
    fs::write(&file, "def value() -> int:\n    return 2\n").unwrap();
    let discarded = vfs.discard_file(&file).unwrap();

    assert!(!discarded.dirty);
    assert!(discarded.source.contains("return 2"));
    assert_eq!(discarded.disk_source, discarded.source);
}

#[test]
fn refreshes_clean_file_deleted_on_disk_as_empty() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert_eq!(snapshot.version, 0);

    fs::remove_file(&file).unwrap();
    let refreshed = vfs.read_file(&file).unwrap();

    assert!(!refreshed.dirty);
    assert_eq!(refreshed.source, "");
    assert_eq!(refreshed.disk_source, "");
    assert_eq!(refreshed.version, 1);
}

#[test]
fn commit_refreshes_clean_file_changed_on_disk() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert_eq!(snapshot.version, 0);

    fs::write(&file, "def value() -> int:\n    return 2\n").unwrap();
    let committed = vfs.commit_file(&file).unwrap();

    assert!(!committed.dirty);
    assert!(committed.source.contains("return 2"));
    assert_eq!(committed.disk_source, committed.source);
    assert_eq!(committed.version, 1);
}

#[test]
fn opens_with_virtual_source_and_lists_dirty_files() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs
        .open_file(&file, Some("def value() -> int:\n    return 7\n"))
        .unwrap();
    assert!(snapshot.dirty);
    assert!(snapshot.source.contains("return 7"));
    assert!(snapshot.disk_source.contains("return 1"));

    let dirty_files = vfs.virtual_file_statuses(true).unwrap();
    assert_eq!(dirty_files.len(), 1);
    assert_eq!(dirty_files[0].file, snapshot.file);
    assert!(dirty_files[0].dirty);
}

#[test]
fn open_with_source_refreshes_disk_baseline() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let initial = vfs.read_file(&file).unwrap();
    assert!(!initial.dirty);

    fs::write(&file, "def value() -> int:\n    return 2\n").unwrap();
    let reopened = vfs
        .open_file(&file, Some("def value() -> int:\n    return 2\n"))
        .unwrap();

    assert!(!reopened.dirty);
    assert!(reopened.source.contains("return 2"));
    assert_eq!(reopened.disk_source, reopened.source);
}

#[test]
fn list_virtual_files_refreshes_clean_disk_changes() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    vfs.read_file(&file).unwrap();
    fs::write(&file, "def value(\n").unwrap();
    let statuses = vfs.virtual_file_statuses(false).unwrap();

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].version, 1);
    assert!(statuses[0].syntax_error_count > 0);
    assert!(!statuses[0].dirty);
}

#[test]
fn closes_virtual_file_without_persisting_changes() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    vfs.open_file(&file, Some("def value() -> int:\n    return 8\n"))
        .unwrap();
    let snapshot = vfs.close_file(&file, false).unwrap();

    assert!(!snapshot.dirty);
    assert!(snapshot.source.contains("return 1"));
    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
    assert!(fs::read_to_string(&file).unwrap().contains("return 1"));
}

#[test]
fn commits_refresh_registered_symbol_index() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");
    let db_path = workspace.join("symbols.db");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return leaf(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 2);
    assert_eq!(stats.reused_files, 0);
    assert_eq!(vfs.registered_symbol_indexes().len(), 1);

    vfs.patch_node(
        &helper_path,
        "helper",
        "def helper(value: int) -> int:\n    return branch(value)\n",
        None,
    )
    .unwrap();
    vfs.commit_file(&helper_path).unwrap();

    let trace = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "branch")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "leaf")
    );

    assert!(vfs.unregister_symbol_index(&workspace).unwrap());
    assert!(vfs.registered_symbol_indexes().is_empty());
}

#[test]
fn commits_new_file_refresh_registered_symbol_index() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");
    let db_path = workspace.join("symbols.db");

    fs::write(
        &caller_path,
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 1);

    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(initial_trace.callees.is_empty());

    vfs.open_file(
        &helper_path,
        Some("def helper(value: int) -> int:\n    return value + 1\n"),
    )
    .unwrap();
    vfs.commit_file(&helper_path).unwrap();

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        updated_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn refreshes_registered_symbol_index_after_external_disk_change() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");
    let db_path = workspace.join("symbols.db");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return leaf(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.register_symbol_index(&workspace, &db_path).unwrap();

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return branch(value)\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();

    let stats = vfs
        .refresh_registered_symbol_indexes(20_000, None, None)
        .unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].indexed_files, 2);
    assert_eq!(stats[0].rebuilt_files, 1);
    assert_eq!(stats[0].reused_files, 1);

    let trace = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "branch")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "leaf")
    );
}

#[test]
fn commits_clean_deleted_file_refresh_registered_symbol_index() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");
    let db_path = workspace.join("symbols.db");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 2);
    vfs.read_file(&helper_path).unwrap();

    fs::remove_file(&helper_path).unwrap();
    let committed = vfs.commit_file(&helper_path).unwrap();

    assert_eq!(committed.source, "");
    assert!(!committed.dirty);
    assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_err());
    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(updated_trace.callees.is_empty());
}

#[test]
fn commits_skip_registered_index_refresh_for_ignored_dirs() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let venv_path = workspace.join("VENV").join("installed.py");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(venv_path.parent().unwrap()).unwrap();
    fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 1);

    vfs.open_file(&venv_path, Some("def installed() -> int:\n    return 2\n"))
        .unwrap();
    vfs.commit_file(&venv_path).unwrap();

    assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_ok());
    assert!(trace_symbol_graph_from_index(&db_path, "installed", TraceDirection::Both).is_err());
}

#[test]
fn commit_skips_registered_index_refresh_for_sibling_workspace_prefix() {
    let dir = temp_workspace();
    let workspace = dir.join("project");
    let sibling = dir.join("project-extra");
    let helper_path = workspace.join("helper.py");
    let sibling_path = sibling.join("installed.py");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&sibling).unwrap();
    fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 1);

    vfs.open_file(
        &sibling_path,
        Some("def installed() -> int:\n    return 2\n"),
    )
    .unwrap();
    vfs.commit_file(&sibling_path).unwrap();

    assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_ok());
    assert!(trace_symbol_graph_from_index(&db_path, "installed", TraceDirection::Both).is_err());
}
