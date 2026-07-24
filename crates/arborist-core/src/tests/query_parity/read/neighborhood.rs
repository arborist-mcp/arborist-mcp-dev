use super::*;

#[test]
fn reads_symbol_neighborhood_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    let orchestrator_source = "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let orchestrator_symbol = "def orchestrate(value: int) -> int:\n    return helper(value)\n";
    let entry_source = "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n";
    let entry_symbol = "def entrypoint(value: int) -> int:\n    return orchestrate(value)\n";

    fs::write(&helper, helper_source).unwrap();
    fs::write(&orchestrator, orchestrator_source).unwrap();
    fs::write(&entry, entry_source).unwrap();

    let live =
        read_symbol_neighborhood_context(&dir, "helper", TraceDirection::Callers, 2, 10).unwrap();
    assert_eq!(live.neighborhood.indexed_files, 3);
    assert_eq!(live.neighborhood.nodes.len(), 3);
    assert_eq!(live.reads.len(), 3);
    assert_eq!(live.reads[0].symbol.semantic_path, "helper");
    assert_eq!(live.reads[0].source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.reads[1].symbol.semantic_path, "orchestrate");
    assert_eq!(
        live.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(live.reads[2].symbol.semantic_path, "entrypoint");
    assert_eq!(live.reads[2].source, entry_symbol.trim_end_matches('\n'));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = read_symbol_neighborhood_context_from_index(
        &db_path,
        "helper",
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.neighborhood.indexed_files, 3);
    assert_eq!(persisted.neighborhood.nodes.len(), 3);
    assert_eq!(persisted.reads.len(), 3);
    assert_eq!(persisted.reads[0].symbol.symbol_id, "helper");
    assert_eq!(persisted.reads[1].symbol.symbol_id, "orchestrate");
    assert_eq!(persisted.reads[2].symbol.symbol_id, "entrypoint");
    assert_eq!(
        persisted.reads[0].source,
        helper_source.trim_end_matches('\n')
    );
    assert_eq!(
        persisted.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        persisted.reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );
}

#[test]
fn read_symbol_neighborhood_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");

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
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
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

    let truncated = vfs
        .read_symbol_neighborhood_context(&dir, "renamed_helper", TraceDirection::Callers, 2, 2)
        .unwrap();
    assert_eq!(
        truncated.neighborhood.symbol.semantic_path,
        "renamed_helper"
    );
    assert_eq!(truncated.neighborhood.nodes.len(), 2);
    assert_eq!(truncated.reads.len(), 2);
    assert_eq!(
        truncated.reads[0].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        truncated.reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
    assert!(truncated.neighborhood.truncated);

    let full = vfs
        .read_symbol_neighborhood_context(&dir, "renamed_helper", TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(full.neighborhood.nodes.len(), 3);
    assert_eq!(full.reads.len(), 3);
    assert_eq!(full.reads[0].symbol.semantic_path, "renamed_helper");
    assert_eq!(full.reads[1].symbol.semantic_path, "orchestrate");
    assert_eq!(full.reads[2].symbol.semantic_path, "entrypoint");
    assert_eq!(full.reads[0].source, renamed_helper.trim_end_matches('\n'));
    assert_eq!(
        full.reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
    assert!(!full.neighborhood.truncated);
}
