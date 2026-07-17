use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) fn temporary_dir() -> PathBuf {
    let suffix = format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let dir = std::env::temp_dir().join(format!("arborist-mcp-{suffix}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub(super) fn create_minimal_symbol_index_schema(connection: &Connection) {
    connection
        .execute_batch(
            "
                CREATE TABLE metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                CREATE TABLE symbols (
                    symbol_id TEXT NOT NULL,
                    semantic_path TEXT NOT NULL,
                    scope_path TEXT,
                    file_path TEXT NOT NULL,
                    node_kind TEXT NOT NULL,
                    start_byte INTEGER NOT NULL,
                    end_byte INTEGER NOT NULL,
                    signature TEXT,
                    parameters_json TEXT NOT NULL DEFAULT '[]',
                    return_type TEXT,
                    docstring TEXT,
                    dependencies_json TEXT NOT NULL,
                    references_json TEXT NOT NULL,
                    reference_names_json TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (semantic_path, file_path)
                );
                CREATE TABLE file_state (
                    file_path TEXT PRIMARY KEY,
                    fingerprint INTEGER NOT NULL
                );
                CREATE INDEX idx_symbols_file_path ON symbols(file_path);
                INSERT INTO metadata(key, value) VALUES('schema_version', '2');
                INSERT INTO metadata(key, value) VALUES('indexed_files', '1');
                ",
        )
        .unwrap();
}

pub(super) fn create_incomplete_symbol_index_tables(connection: &Connection) {
    connection
        .execute_batch(
            "
                CREATE TABLE metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                CREATE TABLE symbols (
                    name TEXT NOT NULL
                );
                CREATE TABLE file_state (
                    file_path TEXT PRIMARY KEY,
                    fingerprint INTEGER NOT NULL
                );
                ",
        )
        .unwrap();
}

pub(super) fn create_symbol_index_schema_with_text_byte_columns(connection: &Connection) {
    connection
        .execute_batch(
            "
                CREATE TABLE metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                CREATE TABLE symbols (
                    symbol_id TEXT NOT NULL,
                    semantic_path TEXT NOT NULL,
                    scope_path TEXT,
                    file_path TEXT NOT NULL,
                    node_kind TEXT NOT NULL,
                    start_byte TEXT NOT NULL,
                    end_byte INTEGER NOT NULL,
                    signature TEXT,
                    parameters_json TEXT NOT NULL DEFAULT '[]',
                    return_type TEXT,
                    docstring TEXT,
                    dependencies_json TEXT NOT NULL,
                    references_json TEXT NOT NULL,
                    reference_names_json TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (semantic_path, file_path)
                );
                CREATE TABLE file_state (
                    file_path TEXT PRIMARY KEY,
                    fingerprint INTEGER NOT NULL
                );
                INSERT INTO metadata(key, value) VALUES('schema_version', '2');
                INSERT INTO metadata(key, value) VALUES('indexed_files', '0');
                ",
        )
        .unwrap();
}

pub(super) fn create_legacy_symbol_index_schema_without_reference_names(
    connection: &Connection,
    workspace_root: Option<&str>,
    indexed_files: Option<&str>,
) {
    connection
        .execute_batch(
            "
                CREATE TABLE metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                CREATE TABLE symbols (
                    symbol_id TEXT NOT NULL,
                    semantic_path TEXT NOT NULL,
                    scope_path TEXT,
                    file_path TEXT NOT NULL,
                    node_kind TEXT NOT NULL,
                    start_byte INTEGER NOT NULL,
                    end_byte INTEGER NOT NULL,
                    signature TEXT,
                    parameters_json TEXT NOT NULL DEFAULT '[]',
                    return_type TEXT,
                    docstring TEXT,
                    dependencies_json TEXT NOT NULL,
                    references_json TEXT NOT NULL,
                    PRIMARY KEY (semantic_path, file_path)
                );
                CREATE TABLE file_state (
                    file_path TEXT PRIMARY KEY,
                    fingerprint INTEGER NOT NULL
                );
                ",
        )
        .unwrap();

    if let Some(workspace_root) = workspace_root {
        connection
            .execute(
                "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)",
                [workspace_root],
            )
            .unwrap();
    }
    if let Some(indexed_files) = indexed_files {
        connection
            .execute(
                "INSERT INTO metadata(key, value) VALUES('indexed_files', ?1)",
                [indexed_files],
            )
            .unwrap();
    }
}

pub(super) fn symbol_table_columns(connection: &Connection) -> Vec<String> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)").unwrap();
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap();
    columns.map(|column| column.unwrap()).collect()
}

pub(super) fn symbol_table_column_type(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> String {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table_name})"))
        .unwrap();
    let columns = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .unwrap();
    columns
        .map(|column| column.unwrap())
        .find_map(|(name, column_type)| (name == column_name).then_some(column_type))
        .unwrap()
}

pub(super) fn normalize_string_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
