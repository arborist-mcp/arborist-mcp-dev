use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use rusqlite::Connection;

use super::loading::nonempty_string_from_row;
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn load_file_states(connection: &Connection) -> Result<BTreeMap<String, u64>> {
    load_file_states_with_deadline(connection, None)
}

pub(crate) fn load_file_states_with_deadline(
    connection: &Connection,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<BTreeMap<String, u64>> {
    let mut statement =
        connection.prepare("SELECT file_path, fingerprint FROM file_state ORDER BY file_path")?;
    let rows = statement.query_map([], |row| {
        Ok((
            nonempty_string_from_row(row, 0, "file_state.file_path")?,
            row.get::<_, i64>(1)?,
        ))
    })?;

    let mut states = BTreeMap::new();
    for row in rows {
        if let Some(deadline) = deadline {
            deadline.check("loading indexed file states")?;
        }
        let (file_path, fingerprint) = row?;
        let fingerprint = u64::try_from(fingerprint).map_err(|error| {
            anyhow!(
                "invalid fingerprint for file_state.file_path {}: {}",
                file_path,
                error
            )
        })?;
        states.insert(file_path, fingerprint);
    }
    Ok(states)
}

pub(crate) fn load_legacy_file_states(connection: &Connection) -> Result<BTreeMap<String, u64>> {
    let mut statement =
        connection.prepare("SELECT file_path, fingerprint FROM file_state ORDER BY file_path")?;
    let rows = statement.query_map([], |row| {
        Ok((
            nonempty_string_from_row(row, 0, "file_state.file_path")?,
            row.get::<_, i64>(1)?,
        ))
    })?;

    let mut states = BTreeMap::new();
    for row in rows {
        let (file_path, fingerprint) = row?;
        states.insert(file_path, fingerprint as u64);
    }
    Ok(states)
}

pub(crate) fn persisted_fingerprint(fingerprint: u64) -> Result<i64> {
    i64::try_from(fingerprint)
        .map_err(|error| anyhow!("fingerprint cannot be persisted as SQLite INTEGER: {error}"))
}

pub(crate) fn count_table_rows(connection: &Connection, table_name: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");
    let count = connection.query_row(&sql, [], |row| row.get::<_, i64>(0))?;
    usize::try_from(count).map_err(|error| anyhow!("invalid row count in `{table_name}`: {error}"))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use rusqlite::Connection;

    use crate::workspace_scan::WorkspaceScanDeadline;

    use super::{load_file_states, load_file_states_with_deadline, load_legacy_file_states};

    #[test]
    fn load_file_states_rejects_negative_fingerprints() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE file_state (
                    file_path TEXT PRIMARY KEY NOT NULL,
                    fingerprint INTEGER NOT NULL
                );
                INSERT INTO file_state(file_path, fingerprint)
                VALUES ('/workspace/helper.py', -1);",
            )
            .unwrap();

        let error = load_file_states(&connection)
            .expect_err("negative persisted fingerprints should be rejected");

        assert!(error.to_string().contains("invalid fingerprint"));
    }

    #[test]
    fn persisted_fingerprint_rejects_values_outside_sqlite_integer_range() {
        let error = super::persisted_fingerprint(i64::MAX as u64 + 1)
            .expect_err("out-of-range fingerprints should be rejected");

        assert!(
            error
                .to_string()
                .contains("cannot be persisted as SQLite INTEGER")
        );
    }

    #[test]
    fn load_file_states_checks_deadline_before_each_row() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE file_state (
                    file_path TEXT PRIMARY KEY NOT NULL,
                    fingerprint INTEGER NOT NULL
                );
                INSERT INTO file_state(file_path, fingerprint)
                VALUES ('/workspace/helper.py', 1);",
            )
            .unwrap();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = load_file_states_with_deadline(&connection, Some(&deadline))
            .expect_err("expired row loading should stop before consuming persisted state");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }

    #[test]
    fn load_legacy_file_states_restores_signed_u64_fingerprints() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE file_state (
                    file_path TEXT PRIMARY KEY NOT NULL,
                    fingerprint INTEGER NOT NULL
                );
                INSERT INTO file_state(file_path, fingerprint)
                VALUES ('/workspace/helper.py', -1);",
            )
            .unwrap();

        let states = load_legacy_file_states(&connection).unwrap();

        assert_eq!(states["/workspace/helper.py"], u64::MAX);
    }
}
