use std::path::Path;

use anyhow::{Result, anyhow, bail};
use rusqlite::Connection;

use crate::deadline::DeadlineCheck;
use crate::index_schema::{
    ANCIENT_SYMBOL_INDEX_SCHEMA_VERSION, LEGACY_SYMBOL_INDEX_SCHEMA_VERSION,
    OLDER_SYMBOL_INDEX_SCHEMA_VERSION, OLDEST_SYMBOL_INDEX_SCHEMA_VERSION,
    PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION, load_indexed_files_metadata,
    load_optional_metadata_value, load_symbol_index_workspace_root,
    migrate_symbol_index_schema_to_current_with_deadline,
    require_legacy_symbol_index_schema_with_deadline,
    require_older_symbol_index_schema_with_deadline,
    require_oldest_symbol_index_schema_with_deadline,
    require_previous_symbol_index_schema_with_deadline, require_symbol_index_tables_with_deadline,
};
use crate::index_store::{
    validate_legacy_indexed_symbols, validate_legacy_indexed_symbols_with_deadline,
    validate_pre_provenance_indexed_symbols, validate_pre_provenance_indexed_symbols_with_deadline,
    validate_previous_indexed_symbols, validate_previous_indexed_symbols_with_deadline,
};

use super::is_migratable_symbol_index_schema_version;

pub(crate) fn migrate_symbol_index(connection: &mut Connection, db_path: &Path) -> Result<()> {
    migrate_symbol_index_inner(connection, db_path, None)
}

pub(crate) fn migrate_symbol_index_with_deadline(
    connection: &mut Connection,
    db_path: &Path,
    deadline: &dyn DeadlineCheck,
) -> Result<()> {
    migrate_symbol_index_inner(connection, db_path, Some(deadline))
}

fn migrate_symbol_index_inner(
    connection: &mut Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    check_optional_deadline(deadline, "validating symbol index tables")?;
    require_symbol_index_tables_with_deadline(connection, db_path, deadline)?;
    check_optional_deadline(deadline, "loading symbol index schema version")?;
    let stored_version =
        load_optional_metadata_value(connection, "schema_version")?.ok_or_else(|| {
            anyhow!(
                "missing schema_version metadata in symbol index {}",
                db_path.display()
            )
        })?;

    if !is_migratable_symbol_index_schema_version(&stored_version) {
        bail!(
            "symbol index schema_version `{stored_version}` in {} cannot be migrated by this Arborist build; expected `{ANCIENT_SYMBOL_INDEX_SCHEMA_VERSION}`, `{OLDEST_SYMBOL_INDEX_SCHEMA_VERSION}`, `{OLDER_SYMBOL_INDEX_SCHEMA_VERSION}`, `{LEGACY_SYMBOL_INDEX_SCHEMA_VERSION}`, or `{PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION}`",
            db_path.display()
        );
    }

    if stored_version == PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION {
        require_previous_symbol_index_schema_with_deadline(connection, db_path, deadline)?;
    } else if stored_version == LEGACY_SYMBOL_INDEX_SCHEMA_VERSION {
        require_legacy_symbol_index_schema_with_deadline(connection, db_path, deadline)?;
    } else if stored_version == OLDER_SYMBOL_INDEX_SCHEMA_VERSION {
        require_older_symbol_index_schema_with_deadline(connection, db_path, deadline)?;
    } else {
        debug_assert!(matches!(
            stored_version.as_str(),
            OLDEST_SYMBOL_INDEX_SCHEMA_VERSION | ANCIENT_SYMBOL_INDEX_SCHEMA_VERSION
        ));
        require_oldest_symbol_index_schema_with_deadline(connection, db_path, deadline)?;
    }
    check_optional_deadline(deadline, "validating legacy symbol index schema")?;
    load_symbol_index_workspace_root(connection, db_path)?;
    check_optional_deadline(deadline, "loading legacy indexed workspace")?;
    load_indexed_files_metadata(connection)?;
    check_optional_deadline(deadline, "loading legacy indexed file count")?;
    if stored_version == PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION {
        match deadline {
            Some(deadline) => {
                validate_pre_provenance_indexed_symbols_with_deadline(connection, Some(deadline))?
            }
            None => validate_pre_provenance_indexed_symbols(connection)?,
        }
    } else if stored_version == LEGACY_SYMBOL_INDEX_SCHEMA_VERSION {
        match deadline {
            Some(deadline) => {
                validate_previous_indexed_symbols_with_deadline(connection, Some(deadline))?
            }
            None => validate_previous_indexed_symbols(connection)?,
        }
    } else {
        match deadline {
            Some(deadline) => {
                validate_legacy_indexed_symbols_with_deadline(connection, Some(deadline))?
            }
            None => validate_legacy_indexed_symbols(connection)?,
        }
    }
    check_optional_deadline(deadline, "migrating symbol index schema")?;
    migrate_symbol_index_schema_to_current_with_deadline(connection, deadline)
}

fn check_optional_deadline(deadline: Option<&dyn DeadlineCheck>, phase: &str) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(())
}
