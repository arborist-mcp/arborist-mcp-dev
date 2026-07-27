use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction};

use crate::language::{normalize_absolute_path, normalize_path};
use crate::workspace_scan::WorkspaceScanDeadline;

use super::schema::SYMBOL_INDEX_SCHEMA_VERSION;

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
    load_symbol_index_workspace_root_with_deadline(connection, db_path, None)
}

pub(crate) fn load_symbol_index_workspace_root_with_deadline(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<PathBuf> {
    if let Some(deadline) = deadline {
        deadline.check("loading indexed workspace root")?;
    }
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
    if let Some(deadline) = deadline {
        deadline.check("loading indexed workspace root")?;
    }

    let stored_workspace_path = Path::new(&stored_workspace);
    if !stored_workspace_path.is_absolute() {
        return Err(anyhow!(
            "workspace_root metadata in symbol index {} is not a normalized absolute path: {}",
            db_path.display(),
            stored_workspace
        ));
    }

    let normalized_workspace = normalize_absolute_path(stored_workspace_path)?;
    if let Some(deadline) = deadline {
        deadline.check("validating indexed workspace root")?;
    }
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
    load_optional_metadata_value_with_deadline(connection, key, None)
}

pub(crate) fn load_optional_metadata_value_with_deadline(
    connection: &Connection,
    key: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if let Some(deadline) = deadline {
        deadline.check("loading index metadata")?;
    }
    let value = connection
        .query_row("SELECT value FROM metadata WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;
    if let Some(deadline) = deadline {
        deadline.check("loading index metadata")?;
    }
    Ok(value)
}

pub(crate) fn validate_symbol_index_workspace_with_deadline(
    connection: &Connection,
    workspace_root: &Path,
    db_path: &Path,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<()> {
    let expected_workspace = normalize_path(workspace_root);
    let stored_workspace =
        load_symbol_index_workspace_root_with_deadline(connection, db_path, deadline)?;
    let stored_workspace = normalize_path(&stored_workspace);

    if stored_workspace != expected_workspace {
        return Err(anyhow!(
            "symbol index {} belongs to workspace {}, not {}",
            db_path.display(),
            stored_workspace,
            expected_workspace
        ));
    }

    if let Some(deadline) = deadline {
        deadline.check("validating indexed workspace")?;
    }
    Ok(())
}

pub(crate) fn load_indexed_files_metadata(connection: &Connection) -> Result<usize> {
    load_indexed_files_metadata_with_deadline(connection, None)
}

pub(crate) fn load_indexed_files_metadata_with_deadline(
    connection: &Connection,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<usize> {
    if let Some(deadline) = deadline {
        deadline.check("loading indexed file metadata")?;
    }
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

    let indexed_files = value
        .parse::<usize>()
        .map_err(|error| anyhow!("invalid indexed_files metadata `{value}`: {error}"))?;
    if let Some(deadline) = deadline {
        deadline.check("loading indexed file metadata")?;
    }
    Ok(indexed_files)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use rusqlite::Connection;

    use crate::workspace_scan::WorkspaceScanDeadline;

    use super::{
        load_indexed_files_metadata_with_deadline, load_optional_metadata_value_with_deadline,
        load_symbol_index_workspace_root_with_deadline,
        validate_symbol_index_workspace_with_deadline,
    };

    #[test]
    fn load_indexed_files_metadata_checks_deadline_before_query() {
        let connection = Connection::open_in_memory().unwrap();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = load_indexed_files_metadata_with_deadline(&connection, Some(&deadline))
            .expect_err("expired deadline should stop before metadata loading");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }

    #[test]
    fn load_optional_metadata_checks_deadline_before_query() {
        let connection = Connection::open_in_memory().unwrap();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = load_optional_metadata_value_with_deadline(
            &connection,
            "schema_version",
            Some(&deadline),
        )
        .expect_err("expired deadline should stop before metadata loading");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }

    #[test]
    fn load_workspace_root_checks_deadline_before_query() {
        let connection = Connection::open_in_memory().unwrap();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = load_symbol_index_workspace_root_with_deadline(
            &connection,
            Path::new("symbols.db"),
            Some(&deadline),
        )
        .expect_err("expired deadline should stop before workspace metadata loading");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }

    #[test]
    fn validate_workspace_checks_deadline_before_query() {
        let connection = Connection::open_in_memory().unwrap();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = validate_symbol_index_workspace_with_deadline(
            &connection,
            Path::new("workspace"),
            Path::new("symbols.db"),
            Some(&deadline),
        )
        .expect_err("expired deadline should stop before workspace validation");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }
}
