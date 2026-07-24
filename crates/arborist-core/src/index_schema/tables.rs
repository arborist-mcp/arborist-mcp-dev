use std::collections::BTreeSet;

use anyhow::{Result, anyhow};
use rusqlite::Connection;

pub(super) fn ensure_symbols_column(
    connection: &Connection,
    columns: &mut BTreeSet<String>,
    column_name: &str,
    add_column_sql: &str,
) -> Result<bool> {
    if columns.contains(column_name) {
        return Ok(false);
    }

    connection.execute(add_column_sql, [])?;
    columns.insert(column_name.to_string());
    Ok(true)
}

pub(super) fn ensure_symbols_primary_key_layout(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
    })?;

    let mut symbol_id_pk = 0;
    let mut file_path_pk = 0;
    let mut start_byte_pk = 0;
    let mut end_byte_pk = 0;
    for column in columns {
        let (name, pk_order) = column?;
        match name.as_str() {
            "symbol_id" => symbol_id_pk = pk_order,
            "file_path" => file_path_pk = pk_order,
            "start_byte" => start_byte_pk = pk_order,
            "end_byte" => end_byte_pk = pk_order,
            _ => {}
        }
    }

    if symbol_id_pk == 1 && file_path_pk == 2 && start_byte_pk == 3 && end_byte_pk == 4 {
        return Ok(());
    }

    Err(anyhow!(
        "symbol index symbols table has incompatible primary key layout; migrate or rebuild the index"
    ))
}

pub(super) fn ensure_symbols_file_path_index(connection: &Connection) -> Result<()> {
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_symbols_file_path ON symbols(file_path)",
        [],
    )?;
    Ok(())
}
