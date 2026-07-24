use super::*;

#[test]
fn lists_symbols_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

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

    let live = list_symbols(&dir, 10).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.total_symbols, 2);
    assert!(!live.truncated);
    assert_eq!(live.symbols.len(), 2);
    assert_eq!(live.symbols[0].semantic_path, "orchestrate");
    assert_eq!(live.symbols[1].semantic_path, "helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = list_symbols_from_index(&db_path, 10).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.total_symbols, 2);
    assert!(!persisted.truncated);
    assert_eq!(persisted.symbols.len(), 2);
    assert_eq!(persisted.symbols[1].semantic_path, "helper");
}

#[test]
fn list_symbols_filters_and_honors_limit() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let helper_types = dir.join("helper_types.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(
        &helper_types,
        "class Helper:\n    pass\n\ndef helper_factory() -> Helper:\n    return Helper()\n",
    )
    .unwrap();

    let live = list_symbols_filtered(&dir, 1, Some("types"), Some("class_definition")).unwrap();
    assert_eq!(live.total_symbols, 1);
    assert!(!live.truncated);
    assert_eq!(live.symbols.len(), 1);
    assert_eq!(live.symbols[0].semantic_path, "Helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        list_symbols_from_index_filtered(&db_path, 1, Some("helper"), Some("function_definition"))
            .unwrap();
    assert_eq!(persisted.total_symbols, 2);
    assert!(persisted.truncated);
    assert_eq!(persisted.symbols.len(), 1);
    assert_eq!(persisted.symbols[0].semantic_path, "helper");
}

#[test]
fn list_symbols_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&helper, Some("class RenamedHelper:\n    pass\n"))
        .unwrap();

    let listed = vfs
        .list_symbols_filtered(&dir, 10, Some("helper.py"), Some("class_definition"))
        .unwrap();
    assert_eq!(listed.total_symbols, 1);
    assert_eq!(listed.symbols.len(), 1);
    assert_eq!(listed.symbols[0].semantic_path, "RenamedHelper");
    assert_eq!(listed.symbols[0].node_kind, "class_definition");
}

#[test]
fn lists_symbol_context_in_live_workspace_and_persisted_index() {
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

    let live = list_symbols_context(&dir, 10).unwrap();
    assert_eq!(live.list.indexed_files, 2);
    assert_eq!(live.list.total_symbols, 2);
    assert_eq!(live.list.symbols.len(), 2);
    assert_eq!(live.reads.len(), 2);
    assert_eq!(live.list.symbols[0].semantic_path, "orchestrate");
    assert_eq!(live.reads[0].symbol.semantic_path, "orchestrate");
    assert_eq!(live.list.symbols[1].semantic_path, "helper");
    assert_eq!(live.reads[1].symbol.semantic_path, "helper");
    assert_eq!(live.reads[1].source, helper_source.trim_end_matches('\n'));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = list_symbols_context_from_index(&db_path, 10).unwrap();
    assert_eq!(persisted.list.indexed_files, 2);
    assert_eq!(persisted.list.total_symbols, 2);
    assert_eq!(persisted.list.symbols.len(), 2);
    assert_eq!(persisted.reads.len(), 2);
    assert_eq!(persisted.list.symbols[0].semantic_path, "orchestrate");
    assert_eq!(persisted.reads[0].symbol.semantic_path, "orchestrate");
    assert_eq!(persisted.list.symbols[1].semantic_path, "helper");
    assert_eq!(persisted.reads[1].symbol.semantic_path, "helper");
    assert_eq!(
        persisted.reads[1].source,
        helper_source.trim_end_matches('\n')
    );
}

#[test]
fn list_symbols_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_source = "class RenamedHelper:\n    pass\n";
    vfs.open_file(&helper, Some(renamed_source)).unwrap();

    let listed = vfs
        .list_symbols_context_filtered(&dir, 10, Some("helper.py"), Some("class_definition"))
        .unwrap();
    assert_eq!(listed.list.total_symbols, 1);
    assert_eq!(listed.list.symbols.len(), 1);
    assert_eq!(listed.reads.len(), 1);
    assert_eq!(listed.list.symbols[0].semantic_path, "RenamedHelper");
    assert_eq!(listed.reads[0].symbol.semantic_path, "RenamedHelper");
    assert_eq!(
        listed.reads[0].source,
        renamed_source.trim_end_matches('\n')
    );
}

#[test]
fn lists_symbol_neighborhood_context_in_live_workspace_and_persisted_index() {
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

    let live = list_symbols_neighborhood_context(&dir, 10, TraceDirection::Callers, 2, 10).unwrap();
    assert_eq!(live.list.indexed_files, 3);
    assert_eq!(live.list.total_symbols, 3);
    assert_eq!(live.list.symbols.len(), 3);
    assert_eq!(live.contexts.len(), 3);
    assert_eq!(live.list.symbols[0].semantic_path, "orchestrate");
    assert_eq!(
        live.contexts[0].neighborhood.symbol.semantic_path,
        "orchestrate"
    );
    assert_eq!(live.contexts[0].reads.len(), 2);
    assert_eq!(
        live.contexts[0].reads[0].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(live.contexts[1].neighborhood.symbol.semantic_path, "helper");
    assert_eq!(live.contexts[1].reads.len(), 3);
    assert_eq!(
        live.contexts[1].reads[0].source,
        helper_source.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[1].reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[1].reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[2].neighborhood.symbol.semantic_path,
        "entrypoint"
    );
    assert_eq!(live.contexts[2].reads.len(), 1);
    assert_eq!(
        live.contexts[2].reads[0].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        list_symbols_neighborhood_context_from_index(&db_path, 10, TraceDirection::Callers, 2, 10)
            .unwrap();
    assert_eq!(persisted.list.indexed_files, 3);
    assert_eq!(persisted.list.total_symbols, 3);
    assert_eq!(persisted.contexts.len(), 3);
    assert_eq!(
        persisted.contexts[0].neighborhood.symbol.semantic_path,
        "orchestrate"
    );
    assert_eq!(
        persisted.contexts[1].neighborhood.symbol.semantic_path,
        "helper"
    );
    assert_eq!(
        persisted.contexts[2].neighborhood.symbol.semantic_path,
        "entrypoint"
    );
    assert_eq!(persisted.contexts[1].reads.len(), 3);
    assert_eq!(persisted.contexts[1].reads[0].symbol.symbol_id, "helper");
    assert_eq!(
        persisted.contexts[1].reads[1].symbol.symbol_id,
        "orchestrate"
    );
    assert_eq!(
        persisted.contexts[1].reads[2].symbol.symbol_id,
        "entrypoint"
    );
}

#[test]
fn list_symbols_neighborhood_context_uses_dirty_vfs_overrides() {
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

    let listed = vfs
        .list_symbols_neighborhood_context(&dir, 10, TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(listed.list.total_symbols, 2);
    assert_eq!(listed.contexts.len(), 2);
    assert_eq!(listed.list.symbols[1].semantic_path, "renamed_helper");
    assert_eq!(
        listed.contexts[1].neighborhood.symbol.semantic_path,
        "renamed_helper"
    );
    assert_eq!(listed.contexts[1].reads.len(), 2);
    assert_eq!(
        listed.contexts[1].reads[0].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        listed.contexts[1].reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
}

#[test]
fn lists_symbol_discovery_context_in_live_workspace_and_persisted_index() {
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

    let live = list_symbols_discovery_context(&dir, 10, TraceDirection::Callers, 2, 10).unwrap();
    assert_eq!(live.list.indexed_files, 3);
    assert_eq!(live.list.total_symbols, 3);
    assert_eq!(live.list.symbols.len(), 3);
    assert_eq!(live.reads.len(), 3);
    assert_eq!(live.contexts.len(), 3);
    assert_eq!(live.list.symbols[0].semantic_path, "orchestrate");
    assert_eq!(live.reads[0].symbol.semantic_path, "orchestrate");
    assert_eq!(
        live.reads[0].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(live.contexts[1].neighborhood.symbol.semantic_path, "helper");
    assert_eq!(live.contexts[1].reads.len(), 3);
    assert_eq!(
        live.contexts[1].reads[0].source,
        helper_source.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[2].reads[0].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        list_symbols_discovery_context_from_index(&db_path, 10, TraceDirection::Callers, 2, 10)
            .unwrap();
    assert_eq!(persisted.list.indexed_files, 3);
    assert_eq!(persisted.list.total_symbols, 3);
    assert_eq!(persisted.reads.len(), 3);
    assert_eq!(persisted.contexts.len(), 3);
    assert_eq!(persisted.reads[1].symbol.symbol_id, "helper");
    assert_eq!(persisted.contexts[1].reads[0].symbol.symbol_id, "helper");
    assert_eq!(
        persisted.contexts[1].reads[1].symbol.symbol_id,
        "orchestrate"
    );
    assert_eq!(
        persisted.contexts[1].reads[2].symbol.symbol_id,
        "entrypoint"
    );
}

#[test]
fn list_symbols_discovery_context_uses_dirty_vfs_overrides() {
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

    let listed = vfs
        .list_symbols_discovery_context(&dir, 10, TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(listed.list.total_symbols, 2);
    assert_eq!(listed.reads.len(), 2);
    assert_eq!(listed.contexts.len(), 2);
    assert_eq!(listed.reads[1].symbol.semantic_path, "renamed_helper");
    assert_eq!(
        listed.reads[1].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        listed.contexts[1].reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
}
