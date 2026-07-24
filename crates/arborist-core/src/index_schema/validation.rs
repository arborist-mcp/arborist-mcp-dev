use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

use super::schema::{
    require_symbols_file_path_index, require_table_column_types, require_table_columns,
    require_table_primary_key_layout,
};

pub(crate) fn require_symbol_index_tables(connection: &Connection, db_path: &Path) -> Result<()> {
    for table_name in ["metadata", "symbols", "file_state"] {
        if !super::schema::table_exists(connection, table_name)? {
            return Err(anyhow::anyhow!(
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

pub(crate) fn require_current_symbol_index_schema(
    connection: &Connection,
    db_path: &Path,
) -> Result<()> {
    require_symbol_index_schema_structure(connection, db_path)?;
    require_table_primary_key_layout(
        connection,
        db_path,
        "symbols",
        &[
            ("symbol_id", 1),
            ("file_path", 2),
            ("start_byte", 3),
            ("end_byte", 4),
        ],
    )?;
    require_symbols_file_path_index(connection, db_path)
}

pub(crate) fn require_legacy_symbol_index_schema(
    connection: &Connection,
    db_path: &Path,
) -> Result<()> {
    require_symbol_index_schema_structure_v3(connection, db_path)?;
    require_table_primary_key_layout(
        connection,
        db_path,
        "symbols",
        &[("semantic_path", 1), ("file_path", 2)],
    )
}

pub(crate) fn require_previous_symbol_index_schema(
    connection: &Connection,
    db_path: &Path,
) -> Result<()> {
    require_symbol_index_schema_structure_v3(connection, db_path)?;
    require_table_primary_key_layout(
        connection,
        db_path,
        "symbols",
        &[
            ("symbol_id", 1),
            ("file_path", 2),
            ("start_byte", 3),
            ("end_byte", 4),
        ],
    )?;
    require_symbols_file_path_index(connection, db_path)
}

fn require_symbol_index_schema_structure(connection: &Connection, db_path: &Path) -> Result<()> {
    require_symbol_index_schema_structure_v3(connection, db_path)?;
    require_table_columns(
        connection,
        db_path,
        "symbols",
        &["reference_call_arities_json"],
    )?;
    require_table_column_types(
        connection,
        db_path,
        "symbols",
        &[("reference_call_arities_json", "TEXT")],
    )
}

fn require_symbol_index_schema_structure_v3(connection: &Connection, db_path: &Path) -> Result<()> {
    require_table_columns(
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
    )?;
    require_table_column_types(
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
    )?;
    require_table_primary_key_layout(connection, db_path, "metadata", &[("key", 1)])?;
    require_table_primary_key_layout(connection, db_path, "file_state", &[("file_path", 1)])?;
    Ok(())
}
