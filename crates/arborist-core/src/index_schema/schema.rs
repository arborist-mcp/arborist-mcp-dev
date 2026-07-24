use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction};

use crate::language::{normalize_absolute_path, normalize_path};

use super::tables::{
    ensure_symbols_column, ensure_symbols_file_path_index, ensure_symbols_primary_key_layout,
};

pub(crate) const SYMBOL_INDEX_SCHEMA_VERSION: &str = "4";
pub(crate) const PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION: &str = "3";
pub(crate) const LEGACY_SYMBOL_INDEX_SCHEMA_VERSION: &str = "2";
pub(crate) const OLDEST_SYMBOL_INDEX_SCHEMA_VERSION: &str = "1";

pub(crate) fn open_symbol_index_read_only(db_path: &Path) -> Result<Connection> {
    Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(Into::into)
}

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

    let stored_workspace_path = Path::new(&stored_workspace);
    if !stored_workspace_path.is_absolute() {
        return Err(anyhow!(
            "workspace_root metadata in symbol index {} is not a normalized absolute path: {}",
            db_path.display(),
            stored_workspace
        ));
    }

    let normalized_workspace = normalize_absolute_path(stored_workspace_path)?;
    if normalize_path(&normalized_workspace) != stored_workspace {
        return Err(anyhow!(
            "workspace_root metadata in symbol index {} is not a normalized absolute path: {}",
            db_path.display(),
            stored_workspace
        ));
    }

    Ok(normalized_workspace)
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
            reference_call_arities_json TEXT NOT NULL DEFAULT '{}',
            PRIMARY KEY (symbol_id, file_path, start_byte, end_byte)
        );
        CREATE TABLE IF NOT EXISTS file_state (
            file_path TEXT PRIMARY KEY,
            fingerprint INTEGER NOT NULL
        );
        ",
    )?;
    let mut symbol_columns = table_columns(connection, "symbols")?;
    ensure_symbols_column(
        connection,
        &mut symbol_columns,
        "reference_names_json",
        "ALTER TABLE symbols ADD COLUMN reference_names_json TEXT NOT NULL DEFAULT '[]'",
    )?;
    if ensure_symbols_column(
        connection,
        &mut symbol_columns,
        "symbol_id",
        "ALTER TABLE symbols ADD COLUMN symbol_id TEXT NOT NULL DEFAULT ''",
    )? {
        connection.execute(
            "UPDATE symbols SET symbol_id = semantic_path WHERE symbol_id = ''",
            [],
        )?;
    }
    ensure_symbols_column(
        connection,
        &mut symbol_columns,
        "scope_path",
        "ALTER TABLE symbols ADD COLUMN scope_path TEXT",
    )?;
    ensure_symbols_column(
        connection,
        &mut symbol_columns,
        "parameters_json",
        "ALTER TABLE symbols ADD COLUMN parameters_json TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_symbols_column(
        connection,
        &mut symbol_columns,
        "return_type",
        "ALTER TABLE symbols ADD COLUMN return_type TEXT",
    )?;
    ensure_symbols_column(
        connection,
        &mut symbol_columns,
        "docstring",
        "ALTER TABLE symbols ADD COLUMN docstring TEXT",
    )?;
    ensure_symbols_primary_key_layout(connection)?;
    ensure_symbols_file_path_index(connection)?;
    Ok(())
}

pub(crate) fn validate_symbol_index_workspace(
    connection: &Connection,
    workspace_root: &Path,
    db_path: &Path,
) -> Result<()> {
    let expected_workspace = normalize_path(workspace_root);
    let stored_workspace = load_symbol_index_workspace_root(connection, db_path)?;
    let stored_workspace = normalize_path(&stored_workspace);

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

pub(super) fn table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
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

pub(super) fn require_table_columns(
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

pub(super) fn require_table_column_types(
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

pub(super) fn table_columns(connection: &Connection, table_name: &str) -> Result<BTreeSet<String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    let mut names = BTreeSet::new();
    for column in columns {
        names.insert(column?);
    }
    Ok(names)
}

pub(super) fn table_column_types(
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

pub(super) fn require_table_primary_key_layout(
    connection: &Connection,
    db_path: &Path,
    table_name: &str,
    expected_columns: &[(&str, i64)],
) -> Result<()> {
    let actual_columns = table_primary_key_layout(connection, table_name)?;
    let expected_columns = expected_columns
        .iter()
        .map(|(name, order)| ((*name).to_string(), *order))
        .collect::<BTreeMap<_, _>>();
    if actual_columns != expected_columns {
        return Err(anyhow!(
            "symbol index table `{}` in {} has incompatible primary key layout",
            table_name,
            db_path.display()
        ));
    }
    Ok(())
}

pub(super) fn require_symbols_file_path_index(
    connection: &Connection,
    db_path: &Path,
) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA index_list(symbols)")?;
    let indexes = statement.query_map([], |row| row.get::<_, String>(1))?;
    for index in indexes {
        if index? != "idx_symbols_file_path" {
            continue;
        }

        let mut columns = connection.prepare("PRAGMA index_info(idx_symbols_file_path)")?;
        let names = columns.query_map([], |row| row.get::<_, String>(2))?;
        let names = names.collect::<rusqlite::Result<Vec<_>>>()?;
        if names == ["file_path"] {
            return Ok(());
        }
        break;
    }

    Err(anyhow!(
        "symbol index table `symbols` in {} is missing required index `idx_symbols_file_path` on `file_path`",
        db_path.display()
    ))
}

fn table_primary_key_layout(
    connection: &Connection,
    table_name: &str,
) -> Result<BTreeMap<String, i64>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let columns = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
    })?;
    let mut primary_key = BTreeMap::new();
    for column in columns {
        let (name, order) = column?;
        if order > 0 {
            primary_key.insert(name, order);
        }
    }
    Ok(primary_key)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use rusqlite::Connection;

    use super::super::tables::ensure_symbols_column;
    use super::{require_table_primary_key_layout, table_columns};

    #[test]
    fn current_schema_validation_rejects_incompatible_primary_keys() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE symbols (
                    semantic_path TEXT NOT NULL,
                    file_path TEXT NOT NULL
                );",
            )
            .unwrap();

        let error = require_table_primary_key_layout(
            &connection,
            Path::new("symbols.db"),
            "symbols",
            &[("semantic_path", 1), ("file_path", 2)],
        )
        .expect_err("missing primary key columns should be rejected");

        assert!(
            error
                .to_string()
                .contains("incompatible primary key layout")
        );
    }

    #[test]
    fn ensures_missing_symbols_columns_once() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE symbols (semantic_path TEXT NOT NULL);")
            .unwrap();
        let mut columns = table_columns(&connection, "symbols").unwrap();

        assert!(
            ensure_symbols_column(
                &connection,
                &mut columns,
                "scope_path",
                "ALTER TABLE symbols ADD COLUMN scope_path TEXT",
            )
            .unwrap()
        );
        assert!(columns.contains("scope_path"));
        assert!(
            !ensure_symbols_column(
                &connection,
                &mut columns,
                "scope_path",
                "ALTER TABLE symbols ADD COLUMN scope_path TEXT",
            )
            .unwrap()
        );

        assert_eq!(table_columns(&connection, "symbols").unwrap(), columns);
        assert_eq!(
            columns,
            BTreeSet::from(["scope_path".to_string(), "semantic_path".to_string()])
        );
    }
}
