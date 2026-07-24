use super::*;

#[test]
fn searches_symbols_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &helper,
            "def helper(value: int) -> int:\n    \"\"\"Increment a value.\"\"\"\n    return value + 1\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let live = search_symbols(&dir, "helper", 10).unwrap();
    assert_eq!(live.query, "helper");
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.total_matches, 1);
    assert!(!live.truncated);
    assert_eq!(live.matches.len(), 1);
    assert_eq!(live.matches[0].semantic_path, "helper");
    assert_eq!(live.match_details.len(), 1);
    assert_eq!(live.match_details[0].symbol_id, "helper");
    assert_eq!(live.match_details[0].score, 1000);
    assert!(
        live.match_details[0]
            .matched_fields
            .contains(&"base_name".to_string())
    );
    assert_eq!(live.matches[0].parameters, vec!["value: int".to_string()]);
    assert_eq!(live.matches[0].return_type.as_deref(), Some("int"));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_from_index(&db_path, "helper", 10).unwrap();
    assert_eq!(persisted.query, "helper");
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.total_matches, 1);
    assert!(!persisted.truncated);
    assert_eq!(persisted.matches.len(), 1);
    assert_eq!(persisted.matches[0].semantic_path, "helper");
    assert_eq!(persisted.match_details[0].symbol_id, "helper");
    assert_eq!(
        persisted.matches[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn search_symbols_prefers_exact_matches_and_honors_limit() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let helper_tools = dir.join("helper_tools.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(
        &helper_tools,
        "def helper_tool() -> int:\n    return 2\n\ndef helper_secondary() -> int:\n    return 3\n",
    )
    .unwrap();

    let live = search_symbols(&dir, "helper", 2).unwrap();
    assert_eq!(live.total_matches, 3);
    assert!(live.truncated);
    assert_eq!(live.matches.len(), 2);
    assert_eq!(live.matches[0].semantic_path, "helper");
    assert_eq!(live.match_details[0].score, 1000);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_from_index(&db_path, "helper", 1).unwrap();
    assert_eq!(persisted.total_matches, 3);
    assert!(persisted.truncated);
    assert_eq!(persisted.matches.len(), 1);
    assert_eq!(persisted.matches[0].semantic_path, "helper");
}

#[test]
fn search_symbols_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &helper,
        Some("def renamed_helper() -> int:\n    return 1\n"),
    )
    .unwrap();

    let results = vfs.search_symbols(&dir, "renamed_helper", 10).unwrap();
    assert_eq!(results.total_matches, 1);
    assert!(!results.truncated);
    assert_eq!(results.matches.len(), 1);
    assert_eq!(results.matches[0].semantic_path, "renamed_helper");
    assert_eq!(results.match_details[0].symbol_id, "renamed_helper");

    let old_name = vfs.search_symbols(&dir, "helper", 10).unwrap();
    assert_eq!(old_name.matches[0].semantic_path, "renamed_helper");
    assert!(
        !old_name
            .matches
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn search_symbols_filters_by_file_path_and_node_kind() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let helper_class = dir.join("helper_types.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(
        &helper_class,
        "class Helper:\n    pass\n\n\ndef helper_factory() -> Helper:\n    return Helper()\n",
    )
    .unwrap();

    let live = search_symbols_filtered(&dir, "helper", 10, Some("types"), Some("class_definition"))
        .unwrap();
    assert_eq!(live.total_matches, 1);
    assert_eq!(live.matches.len(), 1);
    assert_eq!(live.matches[0].semantic_path, "Helper");
    assert_eq!(live.matches[0].node_kind, "class_definition");
    assert!(live.matches[0].file_path.ends_with("helper_types.py"));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_from_index_filtered(
        &db_path,
        "helper",
        10,
        Some("types"),
        Some("class_definition"),
    )
    .unwrap();
    assert_eq!(persisted.total_matches, 1);
    assert_eq!(persisted.matches.len(), 1);
    assert_eq!(persisted.matches[0].semantic_path, "Helper");
    assert_eq!(persisted.matches[0].node_kind, "class_definition");
}

#[test]
fn search_symbols_filtered_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&db_path, "").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&helper, Some("class RenamedHelper:\n    pass\n"))
        .unwrap();

    let filtered = vfs
        .search_symbols_filtered(
            &dir,
            "helper",
            10,
            Some("helper.py"),
            Some("class_definition"),
        )
        .unwrap();
    assert_eq!(filtered.total_matches, 1);
    assert_eq!(filtered.matches.len(), 1);
    assert_eq!(filtered.matches[0].semantic_path, "RenamedHelper");
    assert_eq!(filtered.matches[0].node_kind, "class_definition");
}
