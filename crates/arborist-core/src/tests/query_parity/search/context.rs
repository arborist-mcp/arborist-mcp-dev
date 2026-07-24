use super::*;

#[test]
fn searches_symbol_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    \"\"\"Increment a value.\"\"\"\n    return value + 1\n";
    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let live = search_symbols_context(&dir, "helper", 10).unwrap();
    assert_eq!(live.search.query, "helper");
    assert_eq!(live.search.indexed_files, 2);
    assert_eq!(live.search.total_matches, 1);
    assert_eq!(live.search.matches.len(), 1);
    assert_eq!(live.reads.len(), 1);
    assert_eq!(live.search.matches[0].semantic_path, "helper");
    assert_eq!(live.reads[0].symbol.semantic_path, "helper");
    assert_eq!(live.reads[0].source, helper_source.trim_end_matches('\n'));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_context_from_index(&db_path, "helper", 10).unwrap();
    assert_eq!(persisted.search.query, "helper");
    assert_eq!(persisted.search.indexed_files, 2);
    assert_eq!(persisted.search.total_matches, 1);
    assert_eq!(persisted.search.matches.len(), 1);
    assert_eq!(persisted.reads.len(), 1);
    assert_eq!(persisted.search.matches[0].semantic_path, "helper");
    assert_eq!(persisted.reads[0].symbol.semantic_path, "helper");
    assert_eq!(
        persisted.reads[0].source,
        helper_source.trim_end_matches('\n')
    );
}

#[test]
fn search_symbols_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_source = "def renamed_helper() -> int:\n    return 1\n";
    vfs.open_file(&helper, Some(renamed_source)).unwrap();

    let results = vfs
        .search_symbols_context(&dir, "renamed_helper", 10)
        .unwrap();
    assert_eq!(results.search.total_matches, 1);
    assert_eq!(results.search.matches.len(), 1);
    assert_eq!(results.reads.len(), 1);
    assert_eq!(results.search.matches[0].semantic_path, "renamed_helper");
    assert_eq!(results.reads[0].symbol.semantic_path, "renamed_helper");
    assert_eq!(
        results.reads[0].source,
        renamed_source.trim_end_matches('\n')
    );

    let old_name = vfs.search_symbols_context(&dir, "helper", 10).unwrap();
    assert_eq!(old_name.search.matches[0].semantic_path, "renamed_helper");
    assert_eq!(old_name.reads[0].symbol.semantic_path, "renamed_helper");
    assert!(
        !old_name
            .search
            .matches
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}
