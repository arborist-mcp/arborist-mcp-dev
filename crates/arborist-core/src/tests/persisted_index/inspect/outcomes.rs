use super::*;

#[test]
fn inspect_symbol_index_reports_stale_files() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 2\n").unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(health.exists);
    assert!(!health.ok);
    assert_eq!(health.file_state_entries, Some(1));
    assert_eq!(health.fresh_file_count, Some(0));
    assert_eq!(health.stale_files, vec![normalize_path(&helper)]);
    assert!(health.missing_files.is_empty());
    assert!(health.unreadable_files.is_empty());
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("indexed file is stale"))
    );
}

#[test]
fn persisted_index_queries_reject_stale_file_states() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 2\n").unwrap();

    for error in [
        read_symbol_from_index(&db_path, "helper")
            .expect_err("read_symbol should reject stale persisted indexes")
            .to_string(),
        search_symbols_from_index(&db_path, "helper", 10)
            .expect_err("search_symbols should reject stale persisted indexes")
            .to_string(),
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .expect_err("trace_symbol_graph should reject stale persisted indexes")
            .to_string(),
    ] {
        assert!(error.contains("symbol index"));
        assert!(error.contains("is stale"));
        assert!(error.contains("indexed file is stale"));
        assert!(error.contains("helper.py"));
    }
}

#[test]
fn inspect_symbol_index_reports_missing_indexed_files() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::remove_file(&helper).unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(health.exists);
    assert!(!health.ok);
    assert_eq!(health.file_state_entries, Some(1));
    assert_eq!(health.fresh_file_count, Some(0));
    assert!(health.stale_files.is_empty());
    assert_eq!(health.missing_files, vec![normalize_path(&helper)]);
    assert!(health.unreadable_files.is_empty());
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("indexed file is missing"))
    );
}

#[test]
fn inspect_symbol_index_reports_missing_database_without_creating_it() {
    let dir = temporary_dir();
    let db_path = dir.join("missing.db");

    let health = inspect_symbol_index(&db_path).unwrap();

    assert_eq!(health.response_schema_version, "4");
    assert!(!health.exists);
    assert!(!health.ok);
    assert!(health.migration.required);
    assert_eq!(health.migration.action, "rebuild");
    assert!(health.issues[0].contains("does not exist"));
    assert!(!db_path.exists());
}

#[test]
fn inspect_symbol_index_reports_manual_action_when_database_cannot_be_opened() {
    let dir = temporary_dir();
    let db_path = dir.join("not-a-database");
    fs::create_dir_all(&db_path).unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(health.exists);
    assert!(!health.ok);
    assert!(health.schema_version.is_none());
    assert!(health.migration.required);
    assert_eq!(health.migration.action, "manual");
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("failed to open symbol index"))
    );
    assert!(db_path.is_dir());
}

#[test]
fn inspect_symbol_index_reports_manual_action_for_non_index_database() {
    let dir = temporary_dir();
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute("CREATE TABLE symbols (name TEXT NOT NULL)", [])
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(health.exists);
    assert!(!health.ok);
    assert!(health.migration.required);
    assert_eq!(health.migration.action, "manual");
    assert!(
        health
            .migration
            .reason
            .contains("not a complete Arborist symbol index")
    );
}
