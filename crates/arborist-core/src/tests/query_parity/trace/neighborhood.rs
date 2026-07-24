use super::*;

#[test]
fn traces_symbol_neighborhood_at_position_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

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

    let position = Position { row: 0, column: 5 };
    let live = trace_symbol_neighborhood_at_position(
        &dir,
        &helper,
        &position,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.nodes.len(), 3);
    assert_eq!(live.nodes[1].symbol.semantic_path, "orchestrate");
    assert_eq!(live.nodes[2].symbol.semantic_path, "entrypoint");
    assert_eq!(live.edges.len(), 2);
    assert!(!live.truncated);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_neighborhood_at_position_from_index(
        &db_path,
        &helper,
        &position,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.nodes.len(), 3);
    assert_eq!(persisted.edges.len(), 2);
    assert!(!persisted.truncated);
}

#[test]
fn traces_symbol_neighborhood_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

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

    let live = trace_symbol_neighborhood(&dir, "helper", TraceDirection::Callers, 2, 10).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.nodes.len(), 3);
    assert_eq!(live.nodes[0].symbol.semantic_path, "helper");
    assert_eq!(live.nodes[0].depth, 0);
    assert_eq!(live.nodes[1].symbol.semantic_path, "orchestrate");
    assert_eq!(live.nodes[1].depth, 1);
    assert_eq!(live.nodes[2].symbol.semantic_path, "entrypoint");
    assert_eq!(live.nodes[2].depth, 2);
    assert_eq!(live.edges.len(), 2);
    assert_eq!(live.edges[0].from_symbol_id, "orchestrate");
    assert_eq!(live.edges[0].to_symbol_id, "helper");
    assert_eq!(live.edges[1].from_symbol_id, "entrypoint");
    assert_eq!(live.edges[1].to_symbol_id, "orchestrate");
    assert!(!live.truncated);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_neighborhood_from_index(&db_path, "helper", TraceDirection::Callers, 2, 10)
            .unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.nodes.len(), 3);
    assert_eq!(persisted.edges.len(), 2);
    assert_eq!(persisted.nodes[2].symbol.semantic_path, "entrypoint");
    assert!(!persisted.truncated);
}

#[test]
fn trace_symbol_neighborhood_respects_max_nodes_and_uses_dirty_vfs_overrides() {
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
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&orchestrator, Some(renamed_orchestrator))
        .unwrap();

    let truncated = vfs
        .trace_symbol_neighborhood(&dir, "renamed_helper", TraceDirection::Callers, 2, 2)
        .unwrap();
    assert_eq!(truncated.symbol.semantic_path, "renamed_helper");
    assert_eq!(truncated.nodes.len(), 2);
    assert_eq!(truncated.nodes[1].symbol.semantic_path, "orchestrate");
    assert_eq!(truncated.edges.len(), 1);
    assert!(truncated.truncated);

    let full = vfs
        .trace_symbol_neighborhood(&dir, "renamed_helper", TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(full.nodes.len(), 3);
    assert_eq!(full.nodes[0].symbol.semantic_path, "renamed_helper");
    assert_eq!(full.nodes[1].symbol.semantic_path, "orchestrate");
    assert_eq!(full.nodes[2].symbol.semantic_path, "entrypoint");
    assert_eq!(full.nodes[2].depth, 2);
    assert!(!full.truncated);
}
