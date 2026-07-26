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
        let fingerprint = u64::from_ne_bytes(fingerprint.to_ne_bytes());
        states.insert(file_path, fingerprint);
    }
    Ok(states)
}

pub(crate) fn load_legacy_file_states(connection: &Connection) -> Result<BTreeMap<String, u64>> {
    load_file_states(connection)
}

pub(crate) fn persisted_fingerprint(fingerprint: u64) -> Result<i64> {
    Ok(i64::from_ne_bytes(fingerprint.to_ne_bytes()))
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
    fn load_file_states_restores_signed_fingerprints() {
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

        let states = load_file_states(&connection).unwrap();
        assert_eq!(states["/workspace/helper.py"], u64::MAX);
    }

    #[test]
    fn persisted_fingerprint_round_trips_full_u64_range() {
        let fingerprint = u64::MAX;
        let stored = super::persisted_fingerprint(fingerprint).unwrap();
        assert_eq!(u64::from_ne_bytes(stored.to_ne_bytes()), fingerprint);
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
