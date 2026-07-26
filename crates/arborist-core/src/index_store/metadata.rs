use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use rusqlite::Connection;

use super::loading::nonempty_string_from_row;

pub(crate) fn load_file_states(connection: &Connection) -> Result<BTreeMap<String, u64>> {
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

pub(crate) fn count_table_rows(connection: &Connection, table_name: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");
    let count = connection.query_row(&sql, [], |row| row.get::<_, i64>(0))?;
    usize::try_from(count).map_err(|error| anyhow!("invalid row count in `{table_name}`: {error}"))
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::load_file_states;

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
}
