use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow};
use rusqlite::{Connection, OptionalExtension};

use crate::deadline::DeadlineCheck;

use super::tables::{
    ensure_symbols_column, ensure_symbols_file_path_index, ensure_symbols_primary_key_layout,
};

pub(crate) const SYMBOL_INDEX_SCHEMA_VERSION: &str = "6";
pub(crate) const PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION: &str = "5";
pub(crate) const LEGACY_SYMBOL_INDEX_SCHEMA_VERSION: &str = "4";
pub(crate) const OLDER_SYMBOL_INDEX_SCHEMA_VERSION: &str = "3";
pub(crate) const OLDEST_SYMBOL_INDEX_SCHEMA_VERSION: &str = "2";
pub(crate) const ANCIENT_SYMBOL_INDEX_SCHEMA_VERSION: &str = "1";

pub(crate) fn ensure_symbol_tables_with_deadline(
    connection: &Connection,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    run_symbol_index_schema_step(deadline, || {
        connection.execute_batch("PRAGMA journal_mode = WAL;")?;
        Ok(())
    })?;
    run_symbol_index_schema_step(deadline, || {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(())
    })?;
    run_symbol_index_schema_step(deadline, || {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS symbols (
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
                extension_receiver TEXT,
                dependencies_json TEXT NOT NULL,
                references_json TEXT NOT NULL,
                reference_names_json TEXT NOT NULL DEFAULT '[]',
                reference_call_arities_json TEXT NOT NULL DEFAULT '{}',
                reference_facts_json TEXT NOT NULL DEFAULT '[]',
                PRIMARY KEY (symbol_id, file_path, start_byte, end_byte)
            );",
        )?;
        Ok(())
    })?;
    run_symbol_index_schema_step(deadline, || {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS file_state (
                file_path TEXT PRIMARY KEY,
                fingerprint INTEGER NOT NULL
            );",
        )?;
        Ok(())
    })?;

    let mut symbol_columns =
        run_symbol_index_schema_step(deadline, || table_columns(connection, "symbols"))?;
    run_symbol_index_schema_step(deadline, || {
        ensure_symbols_column(
            connection,
            &mut symbol_columns,
            "reference_names_json",
            "ALTER TABLE symbols ADD COLUMN reference_names_json TEXT NOT NULL DEFAULT '[]'",
        )
    })?;
    run_symbol_index_schema_step(deadline, || {
        ensure_symbols_column(
            connection,
            &mut symbol_columns,
            "reference_call_arities_json",
            "ALTER TABLE symbols ADD COLUMN reference_call_arities_json TEXT NOT NULL DEFAULT '{}'",
        )
    })?;
    run_symbol_index_schema_step(deadline, || {
        ensure_symbols_column(
            connection,
            &mut symbol_columns,
            "reference_facts_json",
            "ALTER TABLE symbols ADD COLUMN reference_facts_json TEXT NOT NULL DEFAULT '[]'",
        )
    })?;
    run_symbol_index_schema_step(deadline, || {
        ensure_symbols_column(
            connection,
            &mut symbol_columns,
            "extension_receiver",
            "ALTER TABLE symbols ADD COLUMN extension_receiver TEXT",
        )
    })?;
    if run_symbol_index_schema_step(deadline, || {
        ensure_symbols_column(
            connection,
            &mut symbol_columns,
            "symbol_id",
            "ALTER TABLE symbols ADD COLUMN symbol_id TEXT NOT NULL DEFAULT ''",
        )
    })? {
        run_symbol_index_schema_step(deadline, || {
            connection.execute(
                "UPDATE symbols SET symbol_id = semantic_path WHERE symbol_id = ''",
                [],
            )?;
            Ok(())
        })?;
    }
    run_symbol_index_schema_step(deadline, || {
        ensure_symbols_column(
            connection,
            &mut symbol_columns,
            "scope_path",
            "ALTER TABLE symbols ADD COLUMN scope_path TEXT",
        )
    })?;
    run_symbol_index_schema_step(deadline, || {
        ensure_symbols_column(
            connection,
            &mut symbol_columns,
            "parameters_json",
            "ALTER TABLE symbols ADD COLUMN parameters_json TEXT NOT NULL DEFAULT '[]'",
        )
    })?;
    run_symbol_index_schema_step(deadline, || {
        ensure_symbols_column(
            connection,
            &mut symbol_columns,
            "return_type",
            "ALTER TABLE symbols ADD COLUMN return_type TEXT",
        )
    })?;
    run_symbol_index_schema_step(deadline, || {
        ensure_symbols_column(
            connection,
            &mut symbol_columns,
            "docstring",
            "ALTER TABLE symbols ADD COLUMN docstring TEXT",
        )
    })?;
    run_symbol_index_schema_step(deadline, || ensure_symbols_primary_key_layout(connection))?;
    run_symbol_index_schema_step(deadline, || ensure_symbols_file_path_index(connection))?;
    Ok(())
}

fn run_symbol_index_schema_step<T>(
    deadline: Option<&dyn DeadlineCheck>,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    if let Some(deadline) = deadline {
        deadline.check("initializing symbol index schema")?;
    }
    let value = operation()?;
    if let Some(deadline) = deadline {
        deadline.check("initializing symbol index schema")?;
    }
    Ok(value)
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
    use std::cell::Cell;
    use std::collections::BTreeSet;
    use std::path::Path;

    use rusqlite::Connection;

    use crate::deadline::DeadlineCheck;

    use super::super::tables::ensure_symbols_column;
    use super::{
        ensure_symbol_tables_with_deadline, require_table_primary_key_layout, table_columns,
        table_exists,
    };

    struct FailOnNthSchemaCheck {
        remaining: Cell<usize>,
    }

    impl FailOnNthSchemaCheck {
        fn new(remaining: usize) -> Self {
            Self {
                remaining: Cell::new(remaining),
            }
        }
    }

    impl DeadlineCheck for FailOnNthSchemaCheck {
        fn check(&self, phase: &str) -> anyhow::Result<()> {
            assert_eq!(phase, "initializing symbol index schema");
            let remaining = self.remaining.get();
            if remaining == 1 {
                anyhow::bail!("test deadline expired during {phase}");
            }
            self.remaining.set(remaining - 1);
            Ok(())
        }
    }

    #[test]
    fn schema_initialization_checks_deadline_between_ddl_steps() {
        let connection = Connection::open_in_memory().unwrap();
        let deadline = FailOnNthSchemaCheck::new(5);

        let error = ensure_symbol_tables_with_deadline(&connection, Some(&deadline))
            .expect_err("deadline should stop schema initialization before the symbols table");

        assert!(
            error
                .to_string()
                .contains("test deadline expired during initializing symbol index schema")
        );
        assert!(table_exists(&connection, "metadata").unwrap());
        assert!(!table_exists(&connection, "symbols").unwrap());
        assert!(!table_exists(&connection, "file_state").unwrap());
    }

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
