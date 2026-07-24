use super::*;

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
