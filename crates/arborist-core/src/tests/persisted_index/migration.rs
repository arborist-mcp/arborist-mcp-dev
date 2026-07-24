use super::*;

#[test]
fn refresh_existing_database_with_unrelated_symbols_table_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute("CREATE TABLE symbols (name TEXT NOT NULL)", [])
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject databases with non-index symbols tables");

    assert!(error.to_string().contains("missing symbol index table"));
    let connection = Connection::open(&db_path).unwrap();
    let created_table_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('metadata', 'file_state')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(created_table_count, 0);
}

#[test]
fn rebuild_existing_database_with_unrelated_symbols_table_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute("CREATE TABLE symbols (name TEXT NOT NULL)", [])
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = rebuild_symbol_index(&dir, &db_path)
        .expect_err("rebuild should reject databases with non-index symbols tables");

    assert!(error.to_string().contains("missing symbol index table"));
    let connection = Connection::open(&db_path).unwrap();
    assert_eq!(symbol_table_columns(&connection), vec!["name"]);
    let created_table_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('metadata', 'file_state')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(created_table_count, 0);
}

#[test]
fn refresh_existing_database_with_incomplete_symbol_columns_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_incomplete_symbol_index_tables(&connection);
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject tables that only resemble symbol indexes");

    assert!(error.to_string().contains("missing required column"));
    let connection = Connection::open(&db_path).unwrap();
    assert_eq!(symbol_table_columns(&connection), vec!["name"]);
}

#[test]
fn refresh_existing_database_with_incompatible_column_types_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_symbol_index_schema_with_text_byte_columns(&connection);
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject incompatible symbol index column types");

    assert!(error.to_string().contains("incompatible type"));
    let connection = Connection::open(&db_path).unwrap();
    assert_eq!(
        symbol_table_column_type(&connection, "symbols", "start_byte"),
        "TEXT"
    );
}

#[test]
fn trace_existing_database_with_unrelated_symbols_table_does_not_migrate() {
    let dir = temporary_dir();
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute("CREATE TABLE symbols (name TEXT NOT NULL)", [])
        .unwrap();
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("trace should reject databases with non-index symbols tables");

    assert!(error.to_string().contains("missing symbol index table"));
    let connection = Connection::open(&db_path).unwrap();
    let created_table_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('metadata', 'file_state')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(created_table_count, 0);
}

#[test]
fn trace_existing_database_with_incomplete_symbol_columns_does_not_migrate() {
    let dir = temporary_dir();
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_incomplete_symbol_index_tables(&connection);
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("trace should reject tables that only resemble symbol indexes");

    assert!(error.to_string().contains("missing required column"));
    let connection = Connection::open(&db_path).unwrap();
    assert_eq!(symbol_table_columns(&connection), vec!["name"]);
}

#[test]
fn trace_existing_database_with_incompatible_column_types_does_not_migrate() {
    let dir = temporary_dir();
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_symbol_index_schema_with_text_byte_columns(&connection);
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("trace should reject incompatible symbol index column types");

    assert!(error.to_string().contains("incompatible type"));
    let connection = Connection::open(&db_path).unwrap();
    assert_eq!(
        symbol_table_column_type(&connection, "symbols", "start_byte"),
        "TEXT"
    );
}

#[test]
fn trace_rejects_missing_metadata_before_legacy_migration() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_legacy_symbol_index_schema_without_reference_names(&connection, None, None);
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("missing metadata should reject before legacy migration");

    assert!(error.to_string().contains("missing indexed_files metadata"));
    let connection = Connection::open(&db_path).unwrap();
    assert!(!symbol_table_columns(&connection).contains(&"reference_names_json".to_string()));
}

#[test]
fn rebuilt_symbol_index_writes_schema_version_metadata() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    let schema_version: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "4");
}

#[test]
fn trace_rejects_missing_schema_version_before_legacy_migration() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_legacy_symbol_index_schema_without_reference_names(
        &connection,
        Some(&normalize_path(&dir)),
        Some("0"),
    );
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("missing schema_version should reject before legacy migration");

    assert!(
        error
            .to_string()
            .contains("missing schema_version metadata")
    );
    let connection = Connection::open(&db_path).unwrap();
    assert!(!symbol_table_columns(&connection).contains(&"reference_names_json".to_string()));
}

#[test]
fn current_schema_missing_columns_is_rejected_without_implicit_migration() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_legacy_symbol_index_schema_without_reference_names(
        &connection,
        Some(&normalize_path(&dir)),
        Some("0"),
    );
    connection
        .execute(
            "INSERT INTO metadata(key, value) VALUES('schema_version', '4')",
            [],
        )
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();
    assert!(!health.ok);
    assert_eq!(health.migration.action, "manual");
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("reference_names_json"))
    );

    for error in [
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .expect_err("queries must not add missing current-schema columns")
            .to_string(),
        refresh_symbol_index_for_file(&dir, &db_path, &helper)
            .expect_err("refresh must not add missing current-schema columns")
            .to_string(),
        rebuild_symbol_index(&dir, &db_path)
            .expect_err("rebuild must not add missing current-schema columns")
            .to_string(),
    ] {
        assert!(error.contains("missing required column `reference_names_json`"));
    }

    let connection = Connection::open(&db_path).unwrap();
    assert!(!symbol_table_columns(&connection).contains(&"reference_names_json".to_string()));
    assert!(
        !symbol_table_columns(&connection).contains(&"reference_call_arities_json".to_string())
    );
}

#[test]
fn trace_rejects_unsupported_schema_version() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_minimal_symbol_index_schema(&connection);
    connection
        .execute(
            "UPDATE metadata SET value = '99' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("unsupported schema_version should be rejected");

    assert!(
        error
            .to_string()
            .contains("unsupported symbol index schema_version")
    );
    assert!(error.to_string().contains("expected `4`"));
}

#[test]
fn migrates_previous_symbol_index_schema_in_place() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    downgrade_symbol_index_schema_to_v2(&connection);
    connection
        .execute("DROP INDEX idx_symbols_file_path", [])
        .unwrap();
    connection
        .execute(
            "UPDATE metadata SET value = '1' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    drop(connection);

    let pending = inspect_symbol_index(&db_path).unwrap();
    assert!(!pending.ok);
    assert_eq!(pending.migration.action, "migrate");

    let migrated = migrate_symbol_index(&db_path).unwrap();
    assert!(migrated.ok, "{:#?}", migrated.issues);
    assert_eq!(migrated.schema_version.as_deref(), Some("4"));
    assert_eq!(migrated.migration.action, "none");

    let connection = Connection::open(&db_path).unwrap();
    let index_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'idx_symbols_file_path')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(index_exists);
    drop(connection);
    assert_eq!(
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .unwrap()
            .symbol
            .semantic_path,
        "helper"
    );
}

#[test]
fn migrates_v2_symbol_index_schema_to_v4_without_losing_symbols() {
    let dir = temporary_dir();
    let helper = dir.join("helper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper,
        "namespace api { int convert(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    downgrade_symbol_index_schema_to_v2(&connection);
    drop(connection);

    let pending = inspect_symbol_index(&db_path).unwrap();
    assert!(!pending.ok);
    assert_eq!(pending.schema_version.as_deref(), Some("2"));
    assert_eq!(pending.migration.action, "migrate");

    let migrated = migrate_symbol_index(&db_path).unwrap();
    assert!(migrated.ok, "{:#?}", migrated.issues);
    assert_eq!(migrated.schema_version.as_deref(), Some("4"));
    assert_eq!(migrated.indexed_symbols, Some(1));
    assert_eq!(
        trace_symbol_graph_from_index(&db_path, "api::convert(int)", TraceDirection::Both)
            .unwrap()
            .symbol
            .symbol_id,
        "api::convert(int)"
    );

    let connection = Connection::open(&db_path).unwrap();
    let primary_key = connection
        .prepare("PRAGMA table_info(symbols)")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
        })
        .unwrap()
        .filter_map(|row| row.ok())
        .filter(|(_, order)| *order > 0)
        .collect::<Vec<_>>();
    assert_eq!(
        primary_key,
        vec![
            ("symbol_id".to_string(), 1),
            ("file_path".to_string(), 2),
            ("start_byte".to_string(), 3),
            ("end_byte".to_string(), 4),
        ]
    );
}

#[test]
fn migration_rolls_back_index_creation_when_schema_version_update_fails() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    downgrade_symbol_index_schema_to_v2(&connection);
    connection
        .execute("DROP INDEX idx_symbols_file_path", [])
        .unwrap();
    connection
        .execute(
            "UPDATE metadata SET value = '1' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    connection
        .execute_batch(
            "
            CREATE TRIGGER reject_schema_version_upgrade
            BEFORE UPDATE OF value ON metadata
            WHEN OLD.key = 'schema_version' AND NEW.value = '4'
            BEGIN
                SELECT RAISE(ABORT, 'forced schema_version update failure');
            END;
            ",
        )
        .unwrap();
    drop(connection);

    let error = migrate_symbol_index(&db_path)
        .expect_err("a failed schema version update must roll back the migration");
    assert!(
        error
            .to_string()
            .contains("forced schema_version update failure")
    );

    let connection = Connection::open(&db_path).unwrap();
    let schema_version: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "1");
    let index_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'idx_symbols_file_path')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!index_exists);
}

#[test]
fn migrates_v3_symbol_index_schema_to_v4_and_rebuilds_call_arity_metadata() {
    let dir = temporary_dir();
    let helper = dir.join("helper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper,
        "namespace api {\nint convert(int value) { return value; }\nint convert(int left, int right) { return left + right; }\nint caller() { return convert(1); }\n}\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    downgrade_symbol_index_schema_to_v3(&connection);
    drop(connection);

    let pending = inspect_symbol_index(&db_path).unwrap();
    assert!(!pending.ok);
    assert_eq!(pending.schema_version.as_deref(), Some("3"));
    assert_eq!(pending.migration.action, "migrate");

    let migrated = migrate_symbol_index(&db_path).unwrap();
    assert!(migrated.ok, "{:#?}", migrated.issues);
    assert_eq!(migrated.schema_version.as_deref(), Some("4"));

    let connection = Connection::open(&db_path).unwrap();
    let call_arities: String = connection
        .query_row(
            "SELECT reference_call_arities_json FROM symbols WHERE semantic_path = 'api::caller'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(call_arities, "{\"convert\":[1]}");
    drop(connection);

    let trace =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::convert(int)"]
    );
}

#[test]
fn migration_rejects_unknown_schema_versions_without_rewrite() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_minimal_symbol_index_schema(&connection);
    connection
        .execute(
            "UPDATE metadata SET value = '99' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    drop(connection);

    let error =
        migrate_symbol_index(&db_path).expect_err("unknown schema versions must not be migrated");
    assert!(error.to_string().contains("cannot be migrated"));

    let connection = Connection::open(&db_path).unwrap();
    let schema_version: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "99");
}

#[test]
fn migration_rejects_invalid_v1_persisted_paths_without_rewrite() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    downgrade_symbol_index_schema_to_v2(&connection);
    connection
        .execute("DROP INDEX idx_symbols_file_path", [])
        .unwrap();
    connection
        .execute(
            "UPDATE metadata SET value = '1' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    connection
        .execute("UPDATE file_state SET file_path = ''", [])
        .unwrap();
    drop(connection);

    let error = migrate_symbol_index(&db_path)
        .expect_err("invalid persisted paths must prevent schema migration");
    assert!(
        error.to_string().contains("empty file_state.file_path"),
        "{error}"
    );

    let connection = Connection::open(&db_path).unwrap();
    let schema_version: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "1");
    let index_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'idx_symbols_file_path')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!index_exists);
}

#[test]
fn migration_rejects_invalid_legacy_reference_names_without_rewrite() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    downgrade_symbol_index_schema_to_v2(&connection);
    connection
        .execute("DROP INDEX idx_symbols_file_path", [])
        .unwrap();
    connection
        .execute(
            "UPDATE metadata SET value = '1' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    connection
        .execute("UPDATE symbols SET reference_names_json = 'not JSON'", [])
        .unwrap();
    drop(connection);

    let error = migrate_symbol_index(&db_path)
        .expect_err("invalid legacy reference names must prevent schema migration");
    assert!(
        error
            .to_string()
            .contains("invalid persisted legacy symbol row"),
        "{error}"
    );

    let connection = Connection::open(&db_path).unwrap();
    let schema_version: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "1");
    let index_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'idx_symbols_file_path')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!index_exists);
}

#[test]
fn inspect_symbol_index_reports_schema_version_issues_without_migration() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_legacy_symbol_index_schema_without_reference_names(
        &connection,
        Some(&normalize_path(&dir)),
        Some("0"),
    );
    drop(connection);

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
            .any(|issue| issue.contains("missing schema_version metadata"))
    );
    let connection = Connection::open(&db_path).unwrap();
    assert!(!symbol_table_columns(&connection).contains(&"reference_names_json".to_string()));
}

#[test]
fn inspect_symbol_index_reports_unsupported_schema_version_without_rewrite() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_minimal_symbol_index_schema(&connection);
    connection
        .execute(
            "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)",
            [normalize_path(&dir)],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE metadata SET value = '99' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(!health.ok);
    assert_eq!(health.schema_version.as_deref(), Some("99"));
    assert!(health.migration.required);
    assert_eq!(health.migration.action, "rebuild");
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("unsupported symbol index schema_version"))
    );
    let connection = Connection::open(&db_path).unwrap();
    let schema_version: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "99");
}

#[test]
fn refresh_existing_database_with_missing_indexed_files_metadata_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute(
            "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)",
            [normalize_path(&dir)],
        )
        .unwrap();
    connection
        .execute("DELETE FROM metadata WHERE key = 'indexed_files'", [])
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject databases with missing indexed_files metadata");

    assert!(error.to_string().contains("missing indexed_files metadata"));
    let connection = Connection::open(&db_path).unwrap();
    let metadata_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM metadata WHERE key = 'indexed_files'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(metadata_count, 0);
}

#[test]
fn refresh_existing_database_with_missing_schema_version_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_legacy_symbol_index_schema_without_reference_names(
        &connection,
        Some(&normalize_path(&dir)),
        Some("0"),
    );
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject databases with missing schema_version metadata");

    assert!(
        error
            .to_string()
            .contains("missing schema_version metadata")
    );
    let connection = Connection::open(&db_path).unwrap();
    assert!(!symbol_table_columns(&connection).contains(&"reference_names_json".to_string()));
}

#[test]
fn refresh_existing_database_with_unsupported_schema_version_does_not_rewrite() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute(
            "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)",
            [normalize_path(&dir)],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE metadata SET value = '99' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject unsupported schema versions");

    assert!(
        error
            .to_string()
            .contains("unsupported symbol index schema_version")
    );
    let connection = Connection::open(&db_path).unwrap();
    let schema_version: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "99");
}

#[test]
fn refresh_existing_database_with_missing_workspace_metadata_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject databases with missing workspace_root metadata");

    assert!(
        error
            .to_string()
            .contains("missing workspace_root metadata")
    );
    let connection = Connection::open(&db_path).unwrap();
    let metadata_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM metadata WHERE key = 'workspace_root'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(metadata_count, 0);
}
