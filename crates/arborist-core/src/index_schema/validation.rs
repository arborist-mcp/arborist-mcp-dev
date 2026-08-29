use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

use crate::deadline::DeadlineCheck;

use super::schema::{
    require_symbols_file_path_index, require_table_column_types, require_table_columns,
    require_table_primary_key_layout,
};

pub(crate) fn require_symbol_index_tables_with_deadline(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    for table_name in ["metadata", "symbols", "file_state"] {
        if !checked(deadline, "validating persisted index tables", || {
            super::schema::table_exists(connection, table_name)
        })? {
            return Err(anyhow::anyhow!(
                "missing symbol index table `{}` in {}",
                table_name,
                db_path.display()
            ));
        }
    }
    require_table_columns_checked(connection, db_path, "metadata", &["key", "value"], deadline)?;
    require_table_column_types_checked(
        connection,
        db_path,
        "metadata",
        &[("key", "TEXT"), ("value", "TEXT")],
        deadline,
    )?;
    require_table_columns_checked(
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
        deadline,
    )?;
    require_table_column_types_checked(
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
        deadline,
    )?;
    require_table_columns_checked(
        connection,
        db_path,
        "file_state",
        &["file_path", "fingerprint"],
        deadline,
    )?;
    require_table_column_types_checked(
        connection,
        db_path,
        "file_state",
        &[("file_path", "TEXT"), ("fingerprint", "INTEGER")],
        deadline,
    )?;
    Ok(())
}

pub(crate) fn require_current_symbol_index_schema_with_deadline(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    require_symbol_index_schema_structure(connection, db_path, deadline)?;
    require_current_symbols_primary_key_and_index(connection, db_path, deadline)
}

pub(crate) fn require_previous_symbol_index_schema_with_deadline(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    require_symbol_index_schema_structure(connection, db_path, deadline)?;
    require_current_symbols_primary_key_and_index(connection, db_path, deadline)
}

pub(crate) fn require_legacy_symbol_index_schema_with_deadline(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    require_symbol_index_schema_structure_v4(connection, db_path, deadline)?;
    require_current_symbols_primary_key_and_index(connection, db_path, deadline)
}

pub(crate) fn require_older_symbol_index_schema_with_deadline(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    require_symbol_index_schema_structure_v3(connection, db_path, deadline)?;
    require_current_symbols_primary_key_and_index(connection, db_path, deadline)
}

pub(crate) fn require_oldest_symbol_index_schema_with_deadline(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    require_symbol_index_schema_structure_v3(connection, db_path, deadline)?;
    require_table_primary_key_layout_checked(
        connection,
        db_path,
        "symbols",
        &[("semantic_path", 1), ("file_path", 2)],
        deadline,
    )
}

fn require_current_symbols_primary_key_and_index(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    require_table_primary_key_layout_checked(
        connection,
        db_path,
        "symbols",
        &[
            ("symbol_id", 1),
            ("file_path", 2),
            ("start_byte", 3),
            ("end_byte", 4),
        ],
        deadline,
    )?;
    checked(deadline, "validating persisted index schema", || {
        require_symbols_file_path_index(connection, db_path)
    })
}

fn require_symbol_index_schema_structure(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    require_symbol_index_schema_structure_v4(connection, db_path, deadline)?;
    require_table_columns_checked(
        connection,
        db_path,
        "symbols",
        &["reference_facts_json"],
        deadline,
    )?;
    require_table_column_types_checked(
        connection,
        db_path,
        "symbols",
        &[("reference_facts_json", "TEXT")],
        deadline,
    )
}

fn require_symbol_index_schema_structure_v4(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    require_symbol_index_schema_structure_v3(connection, db_path, deadline)?;
    require_table_columns_checked(
        connection,
        db_path,
        "symbols",
        &["reference_call_arities_json"],
        deadline,
    )?;
    require_table_column_types_checked(
        connection,
        db_path,
        "symbols",
        &[("reference_call_arities_json", "TEXT")],
        deadline,
    )
}

fn require_symbol_index_schema_structure_v3(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    require_table_columns_checked(
        connection,
        db_path,
        "symbols",
        &[
            "symbol_id",
            "semantic_path",
            "scope_path",
            "file_path",
            "node_kind",
            "start_byte",
            "end_byte",
            "signature",
            "parameters_json",
            "return_type",
            "docstring",
            "dependencies_json",
            "references_json",
            "reference_names_json",
        ],
        deadline,
    )?;
    require_table_column_types_checked(
        connection,
        db_path,
        "symbols",
        &[
            ("symbol_id", "TEXT"),
            ("semantic_path", "TEXT"),
            ("scope_path", "TEXT"),
            ("file_path", "TEXT"),
            ("node_kind", "TEXT"),
            ("start_byte", "INTEGER"),
            ("end_byte", "INTEGER"),
            ("signature", "TEXT"),
            ("parameters_json", "TEXT"),
            ("return_type", "TEXT"),
            ("docstring", "TEXT"),
            ("dependencies_json", "TEXT"),
            ("references_json", "TEXT"),
            ("reference_names_json", "TEXT"),
        ],
        deadline,
    )?;
    require_table_primary_key_layout_checked(
        connection,
        db_path,
        "metadata",
        &[("key", 1)],
        deadline,
    )?;
    require_table_primary_key_layout_checked(
        connection,
        db_path,
        "file_state",
        &[("file_path", 1)],
        deadline,
    )
}

fn require_table_columns_checked(
    connection: &Connection,
    db_path: &Path,
    table_name: &str,
    required_columns: &[&str],
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    checked(deadline, "validating persisted index schema", || {
        require_table_columns(connection, db_path, table_name, required_columns)
    })
}

fn require_table_column_types_checked(
    connection: &Connection,
    db_path: &Path,
    table_name: &str,
    required_types: &[(&str, &str)],
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    checked(deadline, "validating persisted index schema", || {
        require_table_column_types(connection, db_path, table_name, required_types)
    })
}

fn require_table_primary_key_layout_checked(
    connection: &Connection,
    db_path: &Path,
    table_name: &str,
    expected_columns: &[(&str, i64)],
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    checked(deadline, "validating persisted index schema", || {
        require_table_primary_key_layout(connection, db_path, table_name, expected_columns)
    })
}

fn checked<T>(
    deadline: Option<&dyn DeadlineCheck>,
    phase: &str,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    let value = operation()?;
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::Path;

    use anyhow::{Result, bail};
    use rusqlite::Connection;

    use crate::deadline::DeadlineCheck;

    use super::require_symbol_index_tables_with_deadline;

    struct FailsAfterChecks {
        checks: Cell<usize>,
        allowed_checks: usize,
    }

    impl DeadlineCheck for FailsAfterChecks {
        fn check(&self, _phase: &str) -> Result<()> {
            let checks = self.checks.get();
            self.checks.set(checks + 1);
            if checks >= self.allowed_checks {
                bail!("test deadline expired");
            }
            Ok(())
        }
    }

    #[test]
    fn table_validation_checks_deadline_between_sqlite_queries() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE metadata (key TEXT, value TEXT);")
            .unwrap();
        let deadline = FailsAfterChecks {
            checks: Cell::new(0),
            allowed_checks: 1,
        };

        let error = require_symbol_index_tables_with_deadline(
            &connection,
            Path::new("index.db"),
            Some(&deadline),
        )
        .expect_err("deadline should stop validation after its first SQLite query");

        assert!(error.to_string().contains("test deadline expired"));
        assert_eq!(deadline.checks.get(), 2);
    }
}
