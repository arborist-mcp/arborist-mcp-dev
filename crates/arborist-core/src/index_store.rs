use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, types::Type};
use serde::de::DeserializeOwned;

use crate::language::{normalize_absolute_path, normalize_path};
use crate::model::SymbolMeta;

pub(crate) const SYMBOL_INDEX_SCHEMA_VERSION: &str = "1";

pub(crate) fn persist_symbol_index_metadata(
    tx: &Transaction<'_>,
    workspace_root: &Path,
    indexed_files: usize,
) -> Result<()> {
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [SYMBOL_INDEX_SCHEMA_VERSION],
    )?;
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [normalize_path(workspace_root)],
    )?;
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('indexed_files', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [indexed_files.to_string()],
    )?;
    Ok(())
}

pub(crate) fn load_symbol_index_workspace_root(
    connection: &Connection,
    db_path: &Path,
) -> Result<PathBuf> {
    let Some(stored_workspace) = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'workspace_root'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Err(anyhow!(
            "missing workspace_root metadata in symbol index {}",
            db_path.display()
        ));
    };

    normalize_absolute_path(Path::new(&stored_workspace))
}

pub(crate) fn validate_symbol_index_schema_version(
    connection: &Connection,
    db_path: &Path,
) -> Result<()> {
    let Some(value) = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Err(anyhow!(
            "missing schema_version metadata in symbol index {}",
            db_path.display()
        ));
    };

    if value != SYMBOL_INDEX_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported symbol index schema_version `{}` in {}; expected `{}`",
            value,
            db_path.display(),
            SYMBOL_INDEX_SCHEMA_VERSION
        ));
    }

    Ok(())
}

pub(crate) fn load_optional_metadata_value(
    connection: &Connection,
    key: &str,
) -> Result<Option<String>> {
    connection
        .query_row("SELECT value FROM metadata WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(Into::into)
}

pub(crate) fn require_symbol_index_tables(connection: &Connection, db_path: &Path) -> Result<()> {
    for table_name in ["metadata", "symbols", "file_state"] {
        if !table_exists(connection, table_name)? {
            return Err(anyhow!(
                "missing symbol index table `{}` in {}",
                table_name,
                db_path.display()
            ));
        }
    }
    require_table_columns(connection, db_path, "metadata", &["key", "value"])?;
    require_table_column_types(
        connection,
        db_path,
        "metadata",
        &[("key", "TEXT"), ("value", "TEXT")],
    )?;
    require_table_columns(
        connection,
        db_path,
        "symbols",
        &[
            "semantic_path",
            "file_path",
            "node_kind",
            "start_byte",
            "end_byte",
            "signature",
            "dependencies_json",
            "references_json",
        ],
    )?;
    require_table_column_types(
        connection,
        db_path,
        "symbols",
        &[
            ("semantic_path", "TEXT"),
            ("file_path", "TEXT"),
            ("node_kind", "TEXT"),
            ("start_byte", "INTEGER"),
            ("end_byte", "INTEGER"),
            ("signature", "TEXT"),
            ("dependencies_json", "TEXT"),
            ("references_json", "TEXT"),
        ],
    )?;
    require_table_columns(
        connection,
        db_path,
        "file_state",
        &["file_path", "fingerprint"],
    )?;
    require_table_column_types(
        connection,
        db_path,
        "file_state",
        &[("file_path", "TEXT"), ("fingerprint", "INTEGER")],
    )?;
    Ok(())
}

pub(crate) fn ensure_symbol_tables(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS symbols (
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
        CREATE TABLE IF NOT EXISTS file_state (
            file_path TEXT PRIMARY KEY,
            fingerprint INTEGER NOT NULL
        );
        ",
    )?;
    ensure_reference_names_column(connection)?;
    ensure_symbol_id_column(connection)?;
    ensure_scope_path_column(connection)?;
    ensure_parameters_json_column(connection)?;
    ensure_return_type_column(connection)?;
    ensure_docstring_column(connection)?;
    ensure_symbols_primary_key_layout(connection)?;
    Ok(())
}

pub(crate) fn validate_symbol_index_workspace(
    connection: &Connection,
    workspace_root: &Path,
    db_path: &Path,
) -> Result<()> {
    let expected_workspace = normalize_path(workspace_root);
    let stored_workspace = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'workspace_root'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    let Some(stored_workspace) = stored_workspace else {
        return Err(anyhow!(
            "missing workspace_root metadata in symbol index {}",
            db_path.display()
        ));
    };

    if stored_workspace != expected_workspace {
        return Err(anyhow!(
            "symbol index {} belongs to workspace {}, not {}",
            db_path.display(),
            stored_workspace,
            expected_workspace
        ));
    }

    Ok(())
}

pub(crate) fn load_indexed_files_metadata(connection: &Connection) -> Result<usize> {
    let Some(value) = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'indexed_files'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Err(anyhow!("missing indexed_files metadata"));
    };

    value
        .parse::<usize>()
        .map_err(|error| anyhow!("invalid indexed_files metadata `{value}`: {error}"))
}

pub(crate) fn count_table_rows(connection: &Connection, table_name: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");
    let count = connection.query_row(&sql, [], |row| row.get::<_, i64>(0))?;
    usize::try_from(count).map_err(|error| anyhow!("invalid row count in `{table_name}`: {error}"))
}

pub(crate) fn persisted_byte_range(symbol: &SymbolMeta) -> Result<(i64, i64)> {
    if symbol.byte_range.0 > symbol.byte_range.1 {
        return Err(anyhow!(
            "invalid byte range for {}: start {} is after end {}",
            symbol.semantic_path,
            symbol.byte_range.0,
            symbol.byte_range.1
        ));
    }

    Ok((
        i64::try_from(symbol.byte_range.0).map_err(|error| {
            anyhow!("invalid start byte for {}: {}", symbol.semantic_path, error)
        })?,
        i64::try_from(symbol.byte_range.1)
            .map_err(|error| anyhow!("invalid end byte for {}: {}", symbol.semantic_path, error))?,
    ))
}

pub(crate) fn nonempty_string_from_row(
    row: &Row<'_>,
    column: usize,
    column_name: &str,
) -> rusqlite::Result<String> {
    let value: String = row.get(column)?;
    if value.trim().is_empty() {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("empty {column_name}"),
            )),
        ));
    }
    Ok(value)
}

pub(crate) fn json_from_column<T: DeserializeOwned>(
    json: &str,
    column: usize,
) -> rusqlite::Result<T> {
    serde_json::from_str(json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error))
    })
}

pub(crate) fn string_list_from_json_column(
    json: &str,
    column: usize,
    column_name: &str,
) -> rusqlite::Result<Vec<String>> {
    let values: Vec<String> = json_from_column(json, column)?;
    if values.iter().any(|value| value.trim().is_empty()) {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("empty {column_name} entry"),
            )),
        ));
    }
    Ok(values)
}

pub(crate) fn byte_range_from_row(
    row: &Row<'_>,
    start_column: usize,
    end_column: usize,
) -> rusqlite::Result<(usize, usize)> {
    let start = nonnegative_i64_as_usize(row.get(start_column)?, start_column)?;
    let end = nonnegative_i64_as_usize(row.get(end_column)?, end_column)?;
    if start > end {
        return Err(integer_conversion_error(
            end_column,
            format!("end_byte {end} is before start_byte {start}"),
        ));
    }
    Ok((start, end))
}

fn table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table_name],
            |_| Ok(()),
        )
        .optional()
        .map(|hit| hit.is_some())
        .map_err(Into::into)
}

fn require_table_columns(
    connection: &Connection,
    db_path: &Path,
    table_name: &str,
    required_columns: &[&str],
) -> Result<()> {
    let columns = table_columns(connection, table_name)?;
    for required_column in required_columns {
        if !columns.contains(*required_column) {
            return Err(anyhow!(
                "symbol index table `{}` in {} is missing required column `{}`",
                table_name,
                db_path.display(),
                required_column
            ));
        }
    }
    Ok(())
}

fn require_table_column_types(
    connection: &Connection,
    db_path: &Path,
    table_name: &str,
    required_columns: &[(&str, &str)],
) -> Result<()> {
    let column_types = table_column_types(connection, table_name)?;
    for (column_name, expected_type) in required_columns {
        let actual_type = column_types
            .get(*column_name)
            .map(|value| value.to_ascii_uppercase())
            .unwrap_or_default();
        if actual_type != *expected_type {
            return Err(anyhow!(
                "symbol index table `{}` in {} has incompatible type `{}` for column `{}`; expected `{}`",
                table_name,
                db_path.display(),
                actual_type,
                column_name,
                expected_type
            ));
        }
    }
    Ok(())
}

fn table_columns(connection: &Connection, table_name: &str) -> Result<BTreeSet<String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    let mut names = BTreeSet::new();
    for column in columns {
        names.insert(column?);
    }
    Ok(names)
}

fn table_column_types(
    connection: &Connection,
    table_name: &str,
) -> Result<BTreeMap<String, String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let columns = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })?;
    let mut types = BTreeMap::new();
    for column in columns {
        let (name, column_type) = column?;
        types.insert(name, column_type);
    }
    Ok(types)
}

fn nonnegative_i64_as_usize(value: i64, column: usize) -> rusqlite::Result<usize> {
    if value < 0 {
        return Err(integer_conversion_error(
            column,
            format!("expected non-negative integer, got {value}"),
        ));
    }
    usize::try_from(value).map_err(|error| integer_conversion_error(column, error.to_string()))
}

fn integer_conversion_error(column: usize, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        Type::Integer,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}

fn ensure_reference_names_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "reference_names_json" {
            return Ok(());
        }
    }

    connection.execute(
        "ALTER TABLE symbols ADD COLUMN reference_names_json TEXT NOT NULL DEFAULT '[]'",
        [],
    )?;
    Ok(())
}

fn ensure_symbol_id_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "symbol_id" {
            return Ok(());
        }
    }

    connection.execute(
        "ALTER TABLE symbols ADD COLUMN symbol_id TEXT NOT NULL DEFAULT ''",
        [],
    )?;
    connection.execute(
        "UPDATE symbols SET symbol_id = semantic_path WHERE symbol_id = ''",
        [],
    )?;
    Ok(())
}

fn ensure_scope_path_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "scope_path" {
            return Ok(());
        }
    }

    connection.execute("ALTER TABLE symbols ADD COLUMN scope_path TEXT", [])?;
    Ok(())
}

fn ensure_parameters_json_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "parameters_json" {
            return Ok(());
        }
    }

    connection.execute(
        "ALTER TABLE symbols ADD COLUMN parameters_json TEXT NOT NULL DEFAULT '[]'",
        [],
    )?;
    Ok(())
}

fn ensure_return_type_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "return_type" {
            return Ok(());
        }
    }

    connection.execute("ALTER TABLE symbols ADD COLUMN return_type TEXT", [])?;
    Ok(())
}

fn ensure_docstring_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "docstring" {
            return Ok(());
        }
    }

    connection.execute("ALTER TABLE symbols ADD COLUMN docstring TEXT", [])?;
    Ok(())
}

fn ensure_symbols_primary_key_layout(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
    })?;

    let mut semantic_path_pk = 0;
    let mut file_path_pk = 0;
    for column in columns {
        let (name, pk_order) = column?;
        match name.as_str() {
            "semantic_path" => semantic_path_pk = pk_order,
            "file_path" => file_path_pk = pk_order,
            _ => {}
        }
    }

    if semantic_path_pk == 1 && file_path_pk == 2 {
        return Ok(());
    }

    if semantic_path_pk == 0 && file_path_pk == 0 {
        return Ok(());
    }

    connection.execute_batch(
        "
        ALTER TABLE symbols RENAME TO symbols_legacy;
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
        INSERT INTO symbols (
            symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
            signature, parameters_json, return_type, docstring, dependencies_json,
            references_json, reference_names_json
        )
        SELECT
            COALESCE(NULLIF(symbol_id, ''), semantic_path),
            semantic_path, scope_path, file_path, node_kind, start_byte, end_byte, signature,
            COALESCE(parameters_json, '[]'), return_type, docstring,
            dependencies_json, references_json,
            COALESCE(reference_names_json, '[]')
        FROM symbols_legacy;
        DROP TABLE symbols_legacy;
        ",
    )?;
    Ok(())
}
