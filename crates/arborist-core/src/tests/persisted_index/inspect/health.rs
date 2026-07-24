use super::*;

#[test]
fn inspect_symbol_index_rejects_invalid_timeout_before_opening_database() {
    let dir = temporary_dir();
    let db_path = dir.join("missing.db");

    for timeout_ms in [0, MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1] {
        let error = inspect_symbol_index_with_timeout(&db_path, Some(timeout_ms)).expect_err(
            "invalid inspection timeout should be rejected before reading the database",
        );

        assert!(
            error
                .to_string()
                .contains("invalid workspace scan timeout_ms")
        );
    }
    assert!(!db_path.exists());
}

#[test]
fn inspect_symbol_index_reports_healthy_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();

    assert_eq!(health.response_schema_version, "4");
    assert!(health.exists);
    assert!(health.ok);
    assert_eq!(health.schema_version.as_deref(), Some("4"));
    assert_eq!(health.expected_schema_version, "4");
    assert!(!health.migration.required);
    assert_eq!(health.migration.action, "none");
    assert_eq!(
        health.workspace_root.as_deref(),
        Some(normalize_path(&dir).as_str())
    );
    assert_eq!(health.indexed_files, Some(1));
    assert_eq!(health.indexed_symbols, Some(1));
    assert_eq!(health.file_state_entries, Some(1));
    assert_eq!(health.fresh_file_count, Some(1));
    assert!(health.stale_files.is_empty());
    assert!(health.missing_files.is_empty());
    assert!(health.unreadable_files.is_empty());
    assert!(health.unindexed_files.is_empty());
    assert!(health.issues.is_empty());
}

#[test]
fn inspect_and_queries_reject_unindexed_workspace_files() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let added = dir.join("added.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&added, "def added() -> int:\n    return 2\n").unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(!health.ok);
    assert_eq!(health.unindexed_files, vec![normalize_path(&added)]);
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("workspace source file is not indexed"))
    );

    for error in [
        read_symbol_from_index(&db_path, "helper")
            .expect_err("read_symbol should reject incomplete persisted indexes")
            .to_string(),
        search_symbols_from_index(&db_path, "helper", 10)
            .expect_err("search_symbols should reject incomplete persisted indexes")
            .to_string(),
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .expect_err("trace_symbol_graph should reject incomplete persisted indexes")
            .to_string(),
    ] {
        assert!(error.contains("symbol index"));
        assert!(error.contains("is stale"));
        assert!(error.contains("workspace source file is not indexed"));
        assert!(error.contains("added.py"));
    }
}

#[test]
fn inspect_and_queries_reject_inconsistent_indexed_file_counts() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE metadata SET value = '2' WHERE key = 'indexed_files'",
            [],
        )
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(!health.ok);
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("does not match file_state entries"))
    );

    for error in [
        read_symbol_from_index(&db_path, "helper")
            .expect_err("read_symbol should reject inconsistent file counts")
            .to_string(),
        search_symbols_from_index(&db_path, "helper", 10)
            .expect_err("search_symbols should reject inconsistent file counts")
            .to_string(),
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .expect_err("trace_symbol_graph should reject inconsistent file counts")
            .to_string(),
    ] {
        assert!(error.contains("indexed_files metadata 2"));
        assert!(error.contains("file_state entries 1"));
    }
}
