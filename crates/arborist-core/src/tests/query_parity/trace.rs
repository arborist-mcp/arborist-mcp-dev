use super::*;

#[test]
fn traces_unqualified_cpp_using_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let definitions = dir.join("definitions.cpp");
    let caller = dir.join("caller.cpp");
    fs::write(
        &definitions,
        "namespace api { namespace base { int convert(int value) { return value + 1; } } }\n",
    )
    .unwrap();
    fs::write(&caller, "namespace api { int caller() { return 0; } }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some("namespace api { using base::convert; int caller() { return convert(1); } }\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "api::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::base::convert(int)"]
    );
}

#[test]
fn traces_symbol_graph_at_position_in_live_workspace_and_persisted_index() {
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

    let position = Position { row: 0, column: 5 };
    let live =
        trace_symbol_graph_at_position(&dir, &helper, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "orchestrate");
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &helper,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "orchestrate");
}

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
fn trace_symbol_graph_at_position_uses_dirty_vfs_overrides() {
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
        .trace_symbol_graph_at_position(
            &dir,
            &helper,
            &Position { row: 0, column: 5 },
            TraceDirection::Callers,
        )
        .unwrap();
    assert_eq!(result.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.callers.len(), 1);
    assert_eq!(result.callers[0].semantic_path, "orchestrate");
}

#[test]
fn trace_symbol_graph_at_position_with_source_normalizes_path_without_writing_disk() {
    let dir = temporary_dir();
    let nested = dir.join("child");
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let caller_alias = nested.join("..").join("caller.py");

    fs::create_dir_all(&nested).unwrap();
    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let result = trace_symbol_graph_at_position_with_source(
            &dir,
            &caller_alias,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
            &Position { row: 3, column: 5 },
            TraceDirection::Both,
        )
        .unwrap();

    assert!(!caller.exists());
    assert_eq!(result.symbol.semantic_path, "orchestrate");
    assert_eq!(result.symbol.file_path, normalize_path(&caller));
    assert!(
        result
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
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
