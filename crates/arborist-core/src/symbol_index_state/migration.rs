use std::path::Path;

use anyhow::{Result, bail};
use rusqlite::Connection;

use crate::index_migration;
use crate::index_schema::{
    PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION, load_optional_metadata_value,
    load_symbol_index_workspace_root, require_legacy_symbol_index_schema,
    require_previous_symbol_index_schema, require_symbol_index_tables,
};
use crate::index_store::{load_file_states, load_resolved_symbols};
use crate::language::normalize_absolute_path;
use crate::symbols::rebuild_symbol_index;

use super::freshness::validate_indexed_file_count;
use super::inspection::inspect_symbol_index;
use super::paths::validate_persisted_index_paths;

pub fn migrate_symbol_index(db_path: &Path) -> Result<crate::model::SymbolIndexHealth> {
    let db_path = normalize_absolute_path(db_path)?;
    if !db_path.exists() {
        bail!("symbol index {} does not exist", db_path.display());
    }

    let mut connection = Connection::open(&db_path)?;
    let workspace_root = if load_optional_metadata_value(&connection, "schema_version")?
        .as_deref()
        .is_some_and(index_migration::is_migratable_symbol_index_schema_version)
    {
        require_symbol_index_tables(&connection, &db_path)?;
        if load_optional_metadata_value(&connection, "schema_version")?.as_deref()
            == Some(PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION)
        {
            require_previous_symbol_index_schema(&connection, &db_path)?;
        } else {
            require_legacy_symbol_index_schema(&connection, &db_path)?;
        }
        let workspace_root = load_symbol_index_workspace_root(&connection, &db_path)?;
        let file_states = load_file_states(&connection)?;
        let (symbols, indexed_files) = load_resolved_symbols(&connection)?;
        validate_indexed_file_count(indexed_files, file_states.len())?;
        validate_persisted_index_paths(&workspace_root, &file_states, &symbols)?;
        Some(workspace_root)
    } else {
        None
    };
    index_migration::migrate_symbol_index(&mut connection, &db_path)?;
    drop(connection);
    if let Some(workspace_root) = workspace_root {
        rebuild_symbol_index(&workspace_root, &db_path)?;
    }
    inspect_symbol_index(&db_path)
}
