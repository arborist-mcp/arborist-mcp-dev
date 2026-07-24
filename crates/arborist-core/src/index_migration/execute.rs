use std::path::Path;

use anyhow::{Result, anyhow, bail};
use rusqlite::Connection;

use crate::index_schema::{
    LEGACY_SYMBOL_INDEX_SCHEMA_VERSION, OLDEST_SYMBOL_INDEX_SCHEMA_VERSION,
    PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION, load_indexed_files_metadata,
    load_optional_metadata_value, load_symbol_index_workspace_root,
    migrate_symbol_index_schema_to_current, require_legacy_symbol_index_schema,
    require_previous_symbol_index_schema, require_symbol_index_tables,
};
use crate::index_store::validate_legacy_indexed_symbols;

use super::is_migratable_symbol_index_schema_version;

pub(crate) fn migrate_symbol_index(connection: &mut Connection, db_path: &Path) -> Result<()> {
    require_symbol_index_tables(connection, db_path)?;
    let stored_version =
        load_optional_metadata_value(connection, "schema_version")?.ok_or_else(|| {
            anyhow!(
                "missing schema_version metadata in symbol index {}",
                db_path.display()
            )
        })?;

    if !is_migratable_symbol_index_schema_version(&stored_version) {
        bail!(
            "symbol index schema_version `{stored_version}` in {} cannot be migrated by this Arborist build; expected `{OLDEST_SYMBOL_INDEX_SCHEMA_VERSION}`, `{LEGACY_SYMBOL_INDEX_SCHEMA_VERSION}`, or `{PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION}`",
            db_path.display()
        );
    }

    if stored_version == PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION {
        require_previous_symbol_index_schema(connection, db_path)?;
    } else {
        require_legacy_symbol_index_schema(connection, db_path)?;
    }
    load_symbol_index_workspace_root(connection, db_path)?;
    load_indexed_files_metadata(connection)?;
    validate_legacy_indexed_symbols(connection)?;
    migrate_symbol_index_schema_to_current(connection)
}
