use super::*;

#[test]
fn rebuilds_and_reads_persisted_symbol_index() {
    let workspace_root = Path::new("../../tests/fixtures");
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");

    let stats = rebuild_symbol_index(workspace_root, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 4);
    assert!(stats.indexed_symbols >= 3);
    assert_eq!(stats.reused_files, 0);

    let repeat_stats = rebuild_symbol_index(workspace_root, &db_path).unwrap();
    assert_eq!(repeat_stats.indexed_files, 4);
    assert_eq!(repeat_stats.rebuilt_files, 0);
    assert_eq!(repeat_stats.reused_files, 4);

    let trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.parameters, vec!["value: int".to_string()]);
    assert_eq!(trace.symbol.return_type.as_deref(), Some("int"));
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.parameters == vec!["value: int".to_string()])
    );
}

#[test]
fn refreshes_changed_added_and_deleted_workspace_files_incrementally() {
    let dir = temporary_dir();
    let changed = dir.join("changed.py");
    let deleted = dir.join("deleted.py");
    let added = dir.join("added.py");
    let db_path = dir.join("symbols.db");
    fs::write(&changed, "def before() -> int:\n    return 1\n").unwrap();
    fs::write(&deleted, "def removed() -> int:\n    return 2\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    fs::write(&changed, "def after() -> int:\n    return 3\n").unwrap();
    fs::remove_file(&deleted).unwrap();
    fs::write(&added, "def created() -> int:\n    return 4\n").unwrap();

    let stats = refresh_symbol_index(&dir, &db_path).unwrap();

    assert_eq!(stats.indexed_files, 2);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 0);
    assert_eq!(
        search_symbols_from_index(&db_path, "after", 10)
            .unwrap()
            .matches
            .len(),
        1
    );
    assert_eq!(
        search_symbols_from_index(&db_path, "created", 10)
            .unwrap()
            .matches
            .len(),
        1
    );
    assert!(
        search_symbols_from_index(&db_path, "before", 10)
            .unwrap()
            .matches
            .is_empty()
    );
    assert!(
        search_symbols_from_index(&db_path, "removed", 10)
            .unwrap()
            .matches
            .is_empty()
    );
}

#[test]
fn rebuild_symbol_index_normalizes_workspace_and_db_paths() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let nested = workspace.join("child");
    let helper = workspace.join("helper.py");
    let caller = workspace.join("caller.py");

    fs::create_dir_all(&nested).unwrap();
    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let workspace_with_segments = nested.join("..");
    let db_path_with_segments = nested.join("..").join("symbols.db");
    let stats = rebuild_symbol_index(&workspace_with_segments, &db_path_with_segments).unwrap();

    assert_eq!(stats.indexed_files, 2);
    assert!(!stats.db_path.contains("/../"));

    let trace =
        trace_symbol_graph_from_index(&db_path_with_segments, "orchestrate", TraceDirection::Both)
            .unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "helper");
    assert!(!trace.symbol.file_path.contains("/../"));
}
