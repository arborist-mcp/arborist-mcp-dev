use super::*;

#[test]
fn reads_symbol_source_in_live_workspace_and_persisted_index() {
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

    let live = read_symbol(&dir, "helper").unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.start_point.row, 0);
    assert!(live.end_point.row >= live.start_point.row);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = read_symbol_from_index(&db_path, "helper").unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.source, helper_source.trim_end_matches('\n'));
}

#[test]
fn read_symbol_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_source = "def renamed_helper() -> int:\n    return 2\n";
    vfs.open_file(&helper, Some(renamed_source)).unwrap();

    let result = vfs.read_symbol(&dir, "renamed_helper").unwrap();
    assert_eq!(result.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.source, renamed_source.trim_end_matches('\n'));
    assert_eq!(result.start_point.row, 0);
}

#[test]
fn reads_symbol_at_position_in_live_workspace_and_persisted_index() {
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

    let position = Position { row: 0, column: 5 };
    let live = read_symbol_at_position(&dir, &helper, &position).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.start_point.row, 0);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = read_symbol_at_position_from_index(&db_path, &helper, &position).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.source, helper_source.trim_end_matches('\n'));
    assert_eq!(persisted.start_point.row, 0);
}

#[test]
fn read_symbol_at_position_resolves_decorator_lines() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(
            &helper,
            "def decorator(func):\n    return func\n\n@decorator\ndef helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let result = read_symbol_at_position(&dir, &helper, &Position { row: 3, column: 1 })
        .expect("decorator line should resolve to the decorated symbol");

    assert_eq!(result.symbol.semantic_path, "helper");
    assert_eq!(
        result.symbol.signature.as_deref(),
        Some("@decorator\ndef helper(value: int) -> int:")
    );
    assert!(result.source.starts_with("@decorator\ndef helper"));
    assert_eq!(result.start_point.row, 3);
}

#[test]
fn reads_c_symbol_at_position_for_declaration_and_definition_exactly() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let db_path = dir.join("symbols.db");

    let declaration_source = "int helper(int value);\n";
    let definition_source =
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n";
    fs::write(&header, declaration_source).unwrap();
    fs::write(&source, definition_source).unwrap();

    let declaration_live =
        read_symbol_at_position(&dir, &header, &Position { row: 0, column: 4 }).unwrap();
    let definition_live =
        read_symbol_at_position(&dir, &source, &Position { row: 2, column: 4 }).unwrap();

    assert_eq!(declaration_live.symbol.node_kind, "declaration");
    assert_eq!(
        declaration_live.source,
        declaration_source.trim_end_matches('\n')
    );
    assert_eq!(definition_live.symbol.node_kind, "function_definition");
    assert_eq!(
        definition_live.source,
        "int helper(int value) {\n    return value + 1;\n}"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let declaration_persisted =
        read_symbol_at_position_from_index(&db_path, &header, &Position { row: 0, column: 4 })
            .unwrap();
    let definition_persisted =
        read_symbol_at_position_from_index(&db_path, &source, &Position { row: 2, column: 4 })
            .unwrap();

    assert_eq!(declaration_persisted.symbol.node_kind, "declaration");
    assert_eq!(
        declaration_persisted.source,
        declaration_source.trim_end_matches('\n')
    );
    assert_eq!(definition_persisted.symbol.node_kind, "function_definition");
    assert_eq!(
        definition_persisted.source,
        "int helper(int value) {\n    return value + 1;\n}"
    );
}

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

#[test]
fn reads_symbol_discovery_context_in_live_workspace_and_persisted_index() {
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
        read_symbol_discovery_context(&dir, "helper", TraceDirection::Callers, 2, 10).unwrap();
    assert_eq!(live.read.indexed_files, 3);
    assert_eq!(live.trace.indexed_files, 3);
    assert_eq!(live.neighborhood_context.neighborhood.indexed_files, 3);
    assert_eq!(live.read.symbol.semantic_path, "helper");
    assert_eq!(live.trace.symbol.semantic_path, "helper");
    assert_eq!(
        live.neighborhood_context.neighborhood.symbol.semantic_path,
        "helper"
    );
    assert_eq!(live.read.source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.trace.callers.len(), 1);
    assert_eq!(live.trace.callers[0].semantic_path, "orchestrate");
    assert_eq!(live.neighborhood_context.reads.len(), 3);
    assert_eq!(
        live.neighborhood_context.reads[0].source,
        helper_source.trim_end_matches('\n')
    );
    assert_eq!(
        live.neighborhood_context.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.neighborhood_context.reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = read_symbol_discovery_context_from_index(
        &db_path,
        "helper",
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.read.indexed_files, 3);
    assert_eq!(persisted.trace.indexed_files, 3);
    assert_eq!(persisted.neighborhood_context.neighborhood.indexed_files, 3);
    assert_eq!(persisted.read.symbol.symbol_id, "helper");
    assert_eq!(persisted.trace.symbol.symbol_id, "helper");
    assert_eq!(
        persisted.neighborhood_context.neighborhood.symbol.symbol_id,
        "helper"
    );
    assert_eq!(persisted.read.source, helper_source.trim_end_matches('\n'));
    assert_eq!(
        persisted.neighborhood_context.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        persisted.neighborhood_context.reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );
}

#[test]
fn read_symbol_discovery_context_at_position_with_source_normalizes_path_without_writing_disk() {
    let dir = temporary_dir();
    let nested = dir.join("child");
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let caller_alias = nested.join("..").join("caller.py");
    let entry = dir.join("entry.py");

    fs::create_dir_all(&nested).unwrap();
    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &entry,
            "from caller import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let result = read_symbol_discovery_context_at_position_with_source(
            &dir,
            &caller_alias,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
            &Position { row: 3, column: 5 },
            TraceDirection::Both,
            2,
            10,
        )
        .unwrap();

    assert!(!caller.exists());
    assert_eq!(result.read.symbol.semantic_path, "orchestrate");
    assert_eq!(result.read.symbol.file_path, normalize_path(&caller));
    assert_eq!(result.trace.symbol.file_path, normalize_path(&caller));
    assert!(
        result
            .neighborhood_context
            .reads
            .iter()
            .any(|read| read.symbol.semantic_path == "helper")
    );
}

#[test]
fn reads_symbol_discovery_context_at_position_in_live_workspace_and_persisted_index() {
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

    let position = Position { row: 0, column: 5 };
    let live = read_symbol_discovery_context_at_position(
        &dir,
        &helper,
        &position,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(live.read.indexed_files, 3);
    assert_eq!(live.trace.indexed_files, 3);
    assert_eq!(live.neighborhood_context.neighborhood.indexed_files, 3);
    assert_eq!(live.read.symbol.semantic_path, "helper");
    assert_eq!(live.trace.callers[0].semantic_path, "orchestrate");
    assert_eq!(live.neighborhood_context.reads.len(), 3);
    assert_eq!(live.read.source, helper_source.trim_end_matches('\n'));
    assert_eq!(
        live.neighborhood_context.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.neighborhood_context.reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = read_symbol_discovery_context_at_position_from_index(
        &db_path,
        &helper,
        &position,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.read.indexed_files, 3);
    assert_eq!(persisted.trace.indexed_files, 3);
    assert_eq!(persisted.neighborhood_context.neighborhood.indexed_files, 3);
    assert_eq!(persisted.read.symbol.symbol_id, "helper");
    assert_eq!(
        persisted.neighborhood_context.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        persisted.neighborhood_context.reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );
}

#[test]
fn read_symbol_discovery_context_uses_dirty_vfs_overrides() {
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

    let result = vfs
        .read_symbol_discovery_context(&dir, "renamed_helper", TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(result.read.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.trace.symbol.semantic_path, "renamed_helper");
    assert_eq!(
        result
            .neighborhood_context
            .neighborhood
            .symbol
            .semantic_path,
        "renamed_helper"
    );
    assert_eq!(result.read.source, renamed_helper.trim_end_matches('\n'));
    assert_eq!(result.trace.callers.len(), 1);
    assert_eq!(result.trace.callers[0].semantic_path, "orchestrate");
    assert_eq!(result.neighborhood_context.reads.len(), 3);
    assert_eq!(
        result.neighborhood_context.reads[0].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        result.neighborhood_context.reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        result.neighborhood_context.reads[0].symbol.semantic_path,
        "renamed_helper"
    );
    assert_eq!(
        result.neighborhood_context.reads[1].symbol.semantic_path,
        "orchestrate"
    );
    assert_eq!(
        result.neighborhood_context.reads[2].symbol.semantic_path,
        "entrypoint"
    );
}

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
