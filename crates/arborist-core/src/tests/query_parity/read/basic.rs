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
