use super::*;

#[test]
fn rebuild_symbol_index_skips_cache_and_environment_dirs() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let cache_dir = dir.join(".pytest_cache");
    let venv_dir = dir.join("venv");
    let uppercase_venv_dir = dir.join(".VENV");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&cache_dir).unwrap();
    fs::create_dir_all(&venv_dir).unwrap();
    fs::create_dir_all(&uppercase_venv_dir).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(
        cache_dir.join("cached.py"),
        "def cached() -> int:\n    return 2\n",
    )
    .unwrap();
    fs::write(
        venv_dir.join("installed.py"),
        "def installed() -> int:\n    return 3\n",
    )
    .unwrap();
    fs::write(
        uppercase_venv_dir.join("uppercase_installed.py"),
        "def uppercase_installed() -> int:\n    return 4\n",
    )
    .unwrap();

    let stats = rebuild_symbol_index(&dir, &db_path).unwrap();

    assert_eq!(stats.indexed_files, 1);
    assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_ok());
    assert!(trace_symbol_graph_from_index(&db_path, "cached", TraceDirection::Both).is_err());
    assert!(trace_symbol_graph_from_index(&db_path, "installed", TraceDirection::Both).is_err());
    assert!(
        trace_symbol_graph_from_index(&db_path, "uppercase_installed", TraceDirection::Both)
            .is_err()
    );
}

#[test]
fn rebuild_symbol_index_rejects_oversized_source_file() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> str:\n    return 'too large'\n").unwrap();

    let error = rebuild_symbol_index_with_limits(
        &dir,
        &db_path,
        WorkspaceScanLimits {
            max_files: 20_000,
            max_file_bytes: Some(8),
            timeout_ms: None,
        },
    )
    .expect_err("rebuild should reject source files larger than max_file_bytes");

    assert!(error.to_string().contains("source file too large"));
    assert!(error.to_string().contains("max_file_bytes=8"));
    assert!(error.to_string().contains("helper.py"));
}

#[test]
fn refresh_symbol_index_ignores_files_in_skipped_dirs() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let venv_dir = dir.join("VENV");
    let installed = venv_dir.join("installed.py");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&venv_dir).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&installed, "def installed() -> int:\n    return 3\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &installed).unwrap();

    assert_eq!(stats.indexed_files, 1);
    assert_eq!(stats.rebuilt_files, 0);
    assert_eq!(stats.reused_files, 1);
    assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_ok());
    assert!(trace_symbol_graph_from_index(&db_path, "installed", TraceDirection::Both).is_err());
}

#[test]
fn refresh_symbol_index_rejects_oversized_source_file() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    fs::write(&helper, "def helper() -> str:\n    return 'too large'\n").unwrap();
    let error = refresh_symbol_index_for_file_with_limits(
        &dir,
        &db_path,
        &helper,
        WorkspaceScanLimits {
            max_files: 20_000,
            max_file_bytes: Some(8),
            timeout_ms: None,
        },
    )
    .expect_err("refresh should reject source files larger than max_file_bytes");

    assert!(error.to_string().contains("source file too large"));
    assert!(error.to_string().contains("max_file_bytes=8"));
    assert!(error.to_string().contains("helper.py"));
}

#[test]
fn refresh_symbol_index_rejects_invalid_source_file_size_limit() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let error = refresh_symbol_index_for_file_with_limits(
        &dir,
        &db_path,
        &helper,
        WorkspaceScanLimits {
            max_files: 20_000,
            max_file_bytes: Some(0),
            timeout_ms: None,
        },
    )
    .expect_err("refresh should reject invalid max_file_bytes before reading files");

    assert!(error.to_string().contains("max_file_bytes"));
    assert!(error.to_string().contains("greater than zero"));
}

#[test]
fn refresh_existing_non_index_database_does_not_create_schema() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("not-symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    let connection = Connection::open(&db_path).unwrap();
    drop(connection);

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject existing non-index databases");

    assert!(error.to_string().contains("missing symbol index table"));

    let connection = Connection::open(&db_path).unwrap();
    let table_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(table_count, 0);
}

#[test]
fn rebuild_existing_empty_database_does_not_initialize_schema() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("empty.db");
    let connection = Connection::open(&db_path).unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = rebuild_symbol_index(&dir, &db_path)
        .expect_err("rebuild should reject existing databases without symbol index tables");

    assert!(error.to_string().contains("missing symbol index table"));
    let connection = Connection::open(&db_path).unwrap();
    let table_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(table_count, 0);
}

#[test]
fn rebuilds_and_traces_javascript_direct_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("api.js");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "export function helper(value) { return value + 1; }\nexport function caller() { return helper(1); }\n",
    )
    .unwrap();

    let stats = rebuild_symbol_index(&dir, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 1);
    assert_eq!(stats.indexed_symbols, 2);

    let trace = trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.semantic_path, "caller");
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.semantic_path.as_str())
            .collect::<Vec<_>>(),
        vec!["helper"]
    );
}

#[test]
fn traces_named_typescript_imports_to_the_local_module() {
    let dir = temporary_dir();
    let imported = dir.join("imported.ts");
    let alternate = dir.join("alternate.ts");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");
    fs::write(
        &imported,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(
        &alternate,
        "export function helper(value: number): number { return value + 2; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import { helper as selected } from \"./imported\";\nexport function caller(value: number): number { return selected(value); }\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
    assert_eq!(live_trace.callees.len(), 1);
    assert_eq!(live_trace.callees[0].semantic_path, "helper");
    assert_eq!(
        live_trace.callees[0].file_path,
        imported.to_string_lossy().replace('\\', "/")
    );
    assert_ne!(live_trace.callees[0].symbol_id, "helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();

    let trace = trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "helper");
    assert_eq!(
        trace.callees[0].file_path,
        imported.to_string_lossy().replace('\\', "/")
    );
    assert_ne!(trace.callees[0].symbol_id, "helper");
}

#[test]
fn does_not_trace_unresolved_named_typescript_imports_to_workspace_symbols() {
    let dir = temporary_dir();
    let caller = dir.join("caller.ts");
    let unrelated = dir.join("unrelated.ts");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller,
        "import { helper } from \"./missing\";\nexport function caller(value: number): number { return helper(value); }\n",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();

    let trace = trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());
}
