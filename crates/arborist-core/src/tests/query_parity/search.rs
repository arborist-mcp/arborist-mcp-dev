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

#[test]
fn searches_symbol_neighborhood_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    let orchestrator_symbol = "def orchestrate(value: int) -> int:\n    return helper(value)\n";
    let entry_symbol = "def entrypoint(value: int) -> int:\n    return orchestrate(value)\n";

    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let live =
        search_symbols_neighborhood_context(&dir, "helper", 10, TraceDirection::Callers, 2, 10)
            .unwrap();
    assert_eq!(live.search.query, "helper");
    assert_eq!(live.search.indexed_files, 3);
    assert_eq!(live.search.total_matches, 1);
    assert_eq!(live.search.matches.len(), 1);
    assert_eq!(live.contexts.len(), 1);
    assert_eq!(live.contexts[0].neighborhood.nodes.len(), 3);
    assert_eq!(live.contexts[0].reads.len(), 3);
    assert_eq!(live.contexts[0].reads[0].symbol.semantic_path, "helper");
    assert_eq!(
        live.contexts[0].reads[0].source,
        helper_source.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[0].reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[0].reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_neighborhood_context_from_index(
        &db_path,
        "helper",
        10,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.search.query, "helper");
    assert_eq!(persisted.search.indexed_files, 3);
    assert_eq!(persisted.search.total_matches, 1);
    assert_eq!(persisted.contexts.len(), 1);
    assert_eq!(persisted.contexts[0].neighborhood.nodes.len(), 3);
    assert_eq!(persisted.contexts[0].reads[0].symbol.symbol_id, "helper");
    assert_eq!(
        persisted.contexts[0].reads[1].symbol.symbol_id,
        "orchestrate"
    );
    assert_eq!(
        persisted.contexts[0].reads[2].symbol.symbol_id,
        "entrypoint"
    );
}

#[test]
fn search_symbols_neighborhood_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_orchestrator = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    let renamed_orchestrator_symbol =
        "def orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&orchestrator, Some(renamed_orchestrator))
        .unwrap();

    let results = vfs
        .search_symbols_neighborhood_context(
            &dir,
            "renamed_helper",
            10,
            TraceDirection::Callers,
            2,
            10,
        )
        .unwrap();
    assert_eq!(results.search.total_matches, 1);
    assert_eq!(results.contexts.len(), 1);
    assert_eq!(
        results.contexts[0].neighborhood.symbol.semantic_path,
        "renamed_helper"
    );
    assert_eq!(results.contexts[0].reads.len(), 2);
    assert_eq!(
        results.contexts[0].reads[0].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        results.contexts[0].reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
}

#[test]
fn searches_symbol_discovery_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    let orchestrator_symbol = "def orchestrate(value: int) -> int:\n    return helper(value)\n";
    let entry_symbol = "def entrypoint(value: int) -> int:\n    return orchestrate(value)\n";

    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let live = search_symbols_discovery_context(&dir, "helper", 10, TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(live.search.query, "helper");
    assert_eq!(live.search.indexed_files, 3);
    assert_eq!(live.search.total_matches, 1);
    assert_eq!(live.search.matches.len(), 1);
    assert_eq!(live.reads.len(), 1);
    assert_eq!(live.contexts.len(), 1);
    assert_eq!(live.reads[0].symbol.semantic_path, "helper");
    assert_eq!(live.reads[0].source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.contexts[0].neighborhood.nodes.len(), 3);
    assert_eq!(
        live.contexts[0].reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[0].reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_discovery_context_from_index(
        &db_path,
        "helper",
        10,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.search.query, "helper");
    assert_eq!(persisted.search.indexed_files, 3);
    assert_eq!(persisted.search.total_matches, 1);
    assert_eq!(persisted.reads.len(), 1);
    assert_eq!(persisted.contexts.len(), 1);
    assert_eq!(persisted.reads[0].symbol.symbol_id, "helper");
    assert_eq!(persisted.contexts[0].reads[0].symbol.symbol_id, "helper");
    assert_eq!(
        persisted.contexts[0].reads[1].symbol.symbol_id,
        "orchestrate"
    );
    assert_eq!(
        persisted.contexts[0].reads[2].symbol.symbol_id,
        "entrypoint"
    );
}

#[test]
fn search_symbols_discovery_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_orchestrator = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    let renamed_orchestrator_symbol =
        "def orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&orchestrator, Some(renamed_orchestrator))
        .unwrap();

    let results = vfs
        .search_symbols_discovery_context(
            &dir,
            "renamed_helper",
            10,
            TraceDirection::Callers,
            2,
            10,
        )
        .unwrap();
    assert_eq!(results.search.total_matches, 1);
    assert_eq!(results.reads.len(), 1);
    assert_eq!(results.contexts.len(), 1);
    assert_eq!(results.reads[0].symbol.semantic_path, "renamed_helper");
    assert_eq!(
        results.reads[0].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        results.contexts[0].reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
}
