use super::*;
use std::time::{Duration, Instant};

use crate::symbol_index_state::migrate_symbol_index_with_deadline;
use crate::workspace_scan::WorkspaceScanDeadline;

#[test]
fn migration_rejects_invalid_timeouts_before_path_work() {
    let zero = migrate_symbol_index_with_timeout(std::path::Path::new(""), Some(0))
        .expect_err("zero migration timeout should fail before path validation");
    assert!(
        zero.to_string()
            .contains("invalid workspace scan timeout_ms: value must be greater than zero")
    );

    let excessive = migrate_symbol_index_with_timeout(
        std::path::Path::new("symbols.db"),
        Some(MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1),
    )
    .expect_err("excessive migration timeout should fail before file work");
    assert!(
        excessive
            .to_string()
            .contains(&format!("must not exceed {MAX_WORKSPACE_SCAN_TIMEOUT_MS}"))
    );
}

#[test]
fn expired_migration_deadline_leaves_legacy_schema_unchanged() {
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
    let deadline = WorkspaceScanDeadline {
        deadline: Some(Instant::now() - Duration::from_millis(1)),
        timeout_ms: Some(1),
    };

    let error = migrate_symbol_index_with_deadline(&db_path, &deadline)
        .expect_err("expired migration should fail before schema mutation");

    assert!(
        error
            .to_string()
            .contains("workspace scan timeout exceeded")
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
    assert!(
        !symbol_table_columns(&connection).contains(&"reference_call_arities_json".to_string())
    );
}

#[test]
fn timed_migration_upgrades_legacy_schema_and_rebuilds_index() {
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

    let migrated = migrate_symbol_index_with_timeout(&db_path, Some(MAX_WORKSPACE_SCAN_TIMEOUT_MS))
        .expect("timed migration should succeed");

    assert!(migrated.ok, "{:#?}", migrated.issues);
    assert_eq!(migrated.schema_version.as_deref(), Some("6"));
    assert_eq!(
        read_symbol_from_index(&db_path, "helper")
            .unwrap()
            .symbol
            .semantic_path,
        "helper"
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
    assert_eq!(schema_version, "6");
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
            "INSERT INTO metadata(key, value) VALUES('schema_version', '6')",
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
    assert!(error.to_string().contains("expected `6`"));
}

#[test]
fn migrates_v5_symbol_index_schema_to_v6_and_writes_analysis_provenance() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    downgrade_symbol_index_schema_to_v5(&connection);
    drop(connection);

    let pending = inspect_symbol_index(&db_path).unwrap();
    assert!(!pending.ok);
    assert_eq!(pending.schema_version.as_deref(), Some("5"));
    assert_eq!(pending.migration.action, "migrate");

    let migrated = migrate_symbol_index(&db_path).unwrap();
    assert!(migrated.ok, "{:#?}", migrated.issues);
    assert_eq!(migrated.schema_version.as_deref(), Some("6"));

    let connection = Connection::open(&db_path).unwrap();
    let analysis_provenance: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'analysis_provenance'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        analysis_provenance,
        crate::index_schema::current_analysis_provenance_json().unwrap()
    );
}

#[test]
fn migrates_v1_symbol_index_schema_in_place() {
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
    assert_eq!(migrated.schema_version.as_deref(), Some("6"));
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
fn migrates_v2_symbol_index_schema_to_v6_without_losing_symbols() {
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
    assert_eq!(migrated.schema_version.as_deref(), Some("6"));
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
            WHEN OLD.key = 'schema_version' AND NEW.value = '6'
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
fn migration_rolls_back_when_analysis_provenance_write_fails() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    downgrade_symbol_index_schema_to_v5(&connection);
    connection
        .execute_batch(
            "
            CREATE TRIGGER reject_analysis_provenance_write
            BEFORE INSERT ON metadata
            WHEN NEW.key = 'analysis_provenance'
            BEGIN
                SELECT RAISE(ABORT, 'forced analysis_provenance write failure');
            END;
            ",
        )
        .unwrap();
    drop(connection);

    let error = migrate_symbol_index(&db_path)
        .expect_err("a failed analysis provenance write must roll back the migration");
    assert!(
        error
            .to_string()
            .contains("forced analysis_provenance write failure")
    );

    let connection = Connection::open(&db_path).unwrap();
    let schema_version: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "5");
    let provenance_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM metadata WHERE key = 'analysis_provenance'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(provenance_count, 0);
}

#[test]
fn migrates_v4_symbol_index_schema_to_v6_and_writes_reference_facts() {
    let dir = temporary_dir();
    let helper = dir.join("helper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper,
        "namespace api {
int convert(int value) { return value; }
int caller() { return convert(1); }
}
",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    downgrade_symbol_index_schema_to_v4(&connection);
    drop(connection);

    let pending = inspect_symbol_index(&db_path).unwrap();
    assert!(!pending.ok);
    assert_eq!(pending.schema_version.as_deref(), Some("4"));
    assert_eq!(pending.migration.action, "migrate");

    let migrated = migrate_symbol_index(&db_path).unwrap();
    assert!(migrated.ok, "{:#?}", migrated.issues);
    assert_eq!(migrated.schema_version.as_deref(), Some("6"));

    let connection = Connection::open(&db_path).unwrap();
    let reference_facts_json: String = connection
        .query_row(
            "SELECT reference_facts_json FROM symbols WHERE semantic_path = 'api::caller'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        reference_facts_json,
        r#"[{"spelling":"caller","call_arities":null,"language_details":"none"},{"spelling":"convert","call_arities":[1],"language_details":"none"}]"#
    );
}

#[test]
fn migrates_v3_symbol_index_schema_to_v6_and_rebuilds_call_arity_metadata() {
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
    assert_eq!(migrated.schema_version.as_deref(), Some("6"));

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
