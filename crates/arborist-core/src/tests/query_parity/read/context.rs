use super::*;

#[test]
fn reads_symbol_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let live = read_symbol_context(&dir, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.read.indexed_files, 2);
    assert_eq!(live.trace.indexed_files, 2);
    assert_eq!(live.read.symbol.semantic_path, "helper");
    assert_eq!(live.trace.symbol.semantic_path, "helper");
    assert_eq!(live.read.source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.trace.callers.len(), 1);
    assert_eq!(live.trace.callers[0].semantic_path, "orchestrate");
    assert!(live.trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        read_symbol_context_from_index(&db_path, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.read.indexed_files, 2);
    assert_eq!(persisted.trace.indexed_files, 2);
    assert_eq!(persisted.read.symbol.symbol_id, "helper");
    assert_eq!(persisted.trace.symbol.symbol_id, "helper");
    assert_eq!(persisted.read.source, helper_source.trim_end_matches('\n'));
    assert_eq!(persisted.trace.callers.len(), 1);
    assert_eq!(persisted.trace.callers[0].semantic_path, "orchestrate");
}

#[test]
fn read_symbol_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_caller = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&caller, Some(renamed_caller)).unwrap();

    let result = vfs
        .read_symbol_context(&dir, "renamed_helper", TraceDirection::Callers)
        .unwrap();
    assert_eq!(result.read.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.trace.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.read.source, renamed_helper.trim_end_matches('\n'));
    assert_eq!(result.trace.callers.len(), 1);
    assert_eq!(result.trace.callers[0].semantic_path, "orchestrate");
}

#[test]
fn read_symbol_context_at_position_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_caller = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&caller, Some(renamed_caller)).unwrap();

    let result = vfs
        .read_symbol_context_at_position(
            &dir,
            &helper,
            &Position { row: 0, column: 5 },
            TraceDirection::Callers,
        )
        .unwrap();
    assert_eq!(result.read.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.trace.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.read.source, renamed_helper.trim_end_matches('\n'));
    assert_eq!(result.trace.callers.len(), 1);
    assert_eq!(result.trace.callers[0].semantic_path, "orchestrate");
}
