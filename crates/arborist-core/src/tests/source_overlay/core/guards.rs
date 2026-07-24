use super::*;

#[test]
fn index_source_overlay_skips_byte_range_validation_against_stale_disk_source() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(
        &caller,
        "from helper import helper\n\n\ndef orchestrate() -> int:\n    return helper()\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    fs::write(&caller, "def stale():\n    return 0\n").unwrap();
    let source = "from helper import helper\n\n\ndef orchestrate() -> int:\n    return helper()\n";
    let trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller,
        source,
        "orchestrate",
        TraceDirection::Both,
    )
    .expect("source overlays must replace stale disk content before range validation");

    assert_eq!(trace.symbol.semantic_path, "orchestrate");
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn index_overlay_rejects_inconsistent_indexed_file_counts() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let source = "def helper() -> int:\n    return 1\n";

    fs::write(&helper, source).unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE metadata SET value = '2' WHERE key = 'indexed_files'",
            [],
        )
        .unwrap();
    drop(connection);

    let error = search_symbols_from_index_with_source_filtered(
        &db_path, &helper, source, "helper", 10, None, None,
    )
    .expect_err("source overlays should reject inconsistent persisted file counts");

    assert!(error.to_string().contains("indexed_files metadata 2"));
    assert!(error.to_string().contains("file_state entries 1"));
}

#[test]
fn symbol_query_context_rejects_workspace_overlay_outside_workspace() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let outside = dir.join("outside.py");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&outside, "def outside() -> int:\n    return 1\n").unwrap();

    let error = SymbolQueryContext::workspace(&workspace)
        .unwrap()
        .with_source_overlay(&outside, "def outside() -> int:\n    return 2\n")
        .expect_err("workspace contexts should reject overlays outside the workspace");

    assert!(error.to_string().contains("outside workspace"));
}

#[test]
fn symbol_query_context_rejects_workspace_overlay_in_ignored_directory() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let ignored = workspace.join(".venv").join("ignored.py");

    fs::create_dir_all(ignored.parent().unwrap()).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();

    let error = SymbolQueryContext::workspace(&workspace)
        .unwrap()
        .with_source_overlay(&ignored, "def ignored() -> int:\n    return 2\n")
        .expect_err("workspace contexts should reject overlays in ignored directories");

    assert!(error.to_string().contains("ignored workspace directory"));
}

#[test]
fn symbol_query_context_rejects_workspace_overlay_with_unsupported_extension() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let unsupported = workspace.join("notes.txt");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();

    let error = SymbolQueryContext::workspace(&workspace)
        .unwrap()
        .with_source_overlay(&unsupported, "not source code")
        .expect_err("workspace contexts should reject unsupported source overlays");

    assert!(error.to_string().contains("not a supported source file"));
}

#[test]
fn symbol_query_context_rejects_index_overlay_outside_indexed_workspace() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let outside = dir.join("outside.py");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();
    fs::write(&outside, "def outside() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&workspace, &db_path).unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(&outside, "def outside() -> int:\n    return 2\n")
        .unwrap();
    let error = context
        .list_symbols(10, None, None)
        .expect_err("index contexts should reject overlays outside the indexed workspace");

    assert!(error.to_string().contains("outside indexed workspace"));
}

#[test]
fn symbol_query_context_rejects_index_overlay_in_ignored_directory() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let ignored = workspace.join("node_modules").join("ignored.py");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(ignored.parent().unwrap()).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&workspace, &db_path).unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(&ignored, "def ignored() -> int:\n    return 2\n")
        .unwrap();
    let error = context
        .list_symbols(10, None, None)
        .expect_err("index contexts should reject overlays in ignored directories");

    assert!(error.to_string().contains("ignored workspace directory"));
}

#[test]
fn symbol_query_context_rejects_index_overlay_with_unsupported_extension() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let unsupported = workspace.join("notes.txt");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&workspace, &db_path).unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(&unsupported, "not source code")
        .unwrap();
    let error = context
        .list_symbols(10, None, None)
        .expect_err("index contexts should reject unsupported source overlays");

    assert!(error.to_string().contains("not a supported source file"));
}

#[test]
fn rejects_trace_context_file_outside_workspace() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let outside = dir.join("outside.py");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        &outside,
        "def top_level(value: int) -> int:\n    return value\n",
    )
    .unwrap();

    let error = validate_patch_with_trace_context_from_path(
        &workspace,
        &outside,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 1\n",
        None,
        TraceDirection::Both,
    )
    .expect_err("trace context should reject files outside the workspace");

    assert!(error.to_string().contains("outside workspace"));
}
