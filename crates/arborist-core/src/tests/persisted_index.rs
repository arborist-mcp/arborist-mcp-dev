use std::fs;

use rusqlite::Connection;

use super::support::{
    create_incomplete_symbol_index_tables,
    create_legacy_symbol_index_schema_without_reference_names, create_minimal_symbol_index_schema,
    create_symbol_index_schema_with_text_byte_columns, symbol_table_column_type,
    symbol_table_columns, temporary_dir,
};
use crate::language::normalize_path;
use crate::{
    TraceDirection, WorkspaceScanLimits, inspect_symbol_index, read_symbol_from_index,
    rebuild_symbol_index, rebuild_symbol_index_with_limits, refresh_symbol_index_for_file,
    refresh_symbol_index_for_file_with_limits, search_symbols_from_index,
    trace_symbol_graph_from_index,
};

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
fn trace_from_missing_symbol_index_does_not_create_database() {
    let dir = temporary_dir();
    let missing_db_path = dir.join("missing-symbols.db");

    let error =
        trace_symbol_graph_from_index(&missing_db_path, "orchestrate", TraceDirection::Both)
            .unwrap_err();

    assert!(error.to_string().contains("does not exist"));
    assert!(!missing_db_path.exists());
}

#[test]
fn rejects_blank_trace_symbol_paths() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("helper.py"),
        "def helper() -> int:\n    return 1\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let error = trace_symbol_graph_from_index(&db_path, " \t", TraceDirection::Both)
        .expect_err("blank trace symbol paths should be rejected");

    assert!(error.to_string().contains("symbol_path"));
    assert!(error.to_string().contains("blank"));
}

#[test]
fn trace_from_existing_non_index_database_does_not_create_schema() {
    let dir = temporary_dir();
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("empty databases should not be initialized by read paths");

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
fn trace_from_index_rejects_negative_persisted_byte_ranges() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute_batch(
            "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    'helper', 'helper', 'helper.py', 'function_definition', -1, 5,
                    '[]', '[]', '[]', '[]'
                );
                ",
        )
        .unwrap();

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("negative persisted byte ranges should be rejected");

    assert!(error.to_string().contains("non-negative integer"));
}

#[test]
fn trace_from_index_rejects_invalid_persisted_json_columns() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute_batch(
            "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    'helper', 'helper', 'helper.py', 'function_definition', 0, 5,
                    '[]', '{not-json', '[]', '[]'
                );
                ",
        )
        .unwrap();

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("invalid persisted JSON columns should be rejected");

    assert!(error.to_string().contains("Conversion error"));
}

#[test]
fn trace_from_index_rejects_empty_persisted_symbol_identity() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute_batch(
            "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    '', 'helper', 'helper.py', 'function_definition', 0, 5,
                    '[]', '[]', '[]', '[]'
                );
                ",
        )
        .unwrap();

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("empty persisted symbol identity should be rejected");

    assert!(error.to_string().contains("empty symbol_id"));
}

#[test]
fn trace_from_index_rejects_empty_persisted_graph_edges() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute_batch(
            "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    'helper', 'helper', 'helper.py', 'function_definition', 0, 5,
                    '[]', '[\"\"]', '[]', '[]'
                );
                ",
        )
        .unwrap();

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("empty persisted graph edges should be rejected");

    assert!(error.to_string().contains("empty dependencies_json entry"));
}

#[test]
fn trace_from_index_rejects_invalid_indexed_files_metadata() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute(
            "UPDATE metadata SET value = 'many' WHERE key = 'indexed_files'",
            [],
        )
        .unwrap();

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("invalid indexed_files metadata should be rejected");

    assert!(error.to_string().contains("invalid indexed_files metadata"));
}

#[test]
fn trace_from_index_rejects_missing_indexed_files_metadata() {
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
        .execute("DELETE FROM metadata WHERE key = 'indexed_files'", [])
        .unwrap();
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("missing indexed_files metadata should be rejected");

    assert!(error.to_string().contains("missing indexed_files metadata"));
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
    assert_eq!(schema_version, "1");
}

#[test]
fn inspect_symbol_index_reports_healthy_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();

    assert_eq!(health.response_schema_version, "3");
    assert!(health.exists);
    assert!(health.ok);
    assert_eq!(health.schema_version.as_deref(), Some("1"));
    assert_eq!(health.expected_schema_version, "1");
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

    assert_eq!(health.response_schema_version, "3");
    assert!(!health.exists);
    assert!(!health.ok);
    assert!(health.migration.required);
    assert_eq!(health.migration.action, "rebuild");
    assert!(health.issues[0].contains("does not exist"));
    assert!(!db_path.exists());
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
    assert!(error.to_string().contains("expected `1`"));
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
