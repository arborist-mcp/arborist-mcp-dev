use std::path::Path;

use anyhow::{Result, bail};
use rusqlite::Connection;

use crate::index_migration;
use crate::index_schema::{
    LEGACY_SYMBOL_INDEX_SCHEMA_VERSION, PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION,
    load_optional_metadata_value, load_optional_metadata_value_with_deadline,
    load_symbol_index_workspace_root, load_symbol_index_workspace_root_with_deadline,
    require_legacy_symbol_index_schema, require_older_symbol_index_schema,
    require_previous_symbol_index_schema, require_symbol_index_tables,
};
use crate::index_store::{
    load_file_states_with_deadline, load_legacy_file_states, load_resolved_symbols,
    load_resolved_symbols_with_deadline,
};
use crate::language::normalize_absolute_path;
use crate::symbols::rebuild_symbol_index;
use crate::workspace_scan::{WorkspaceScanDeadline, WorkspaceScanLimits};

use super::freshness::validate_indexed_file_count;
use super::inspection::inspect_symbol_index;
use super::paths::{
    validate_persisted_index_paths, validate_persisted_index_paths_with_overrides_and_deadline,
};

pub fn migrate_symbol_index(db_path: &Path) -> Result<crate::model::SymbolIndexHealth> {
    migrate_symbol_index_inner(db_path, None)
}

pub fn migrate_symbol_index_with_timeout(
    db_path: &Path,
    timeout_ms: Option<u64>,
) -> Result<crate::model::SymbolIndexHealth> {
    let Some(timeout_ms) = timeout_ms else {
        return migrate_symbol_index(db_path);
    };
    let deadline = WorkspaceScanDeadline::new(WorkspaceScanLimits {
        timeout_ms: Some(timeout_ms),
        ..WorkspaceScanLimits::default()
    })?;
    migrate_symbol_index_with_deadline(db_path, &deadline)
}

pub(crate) fn migrate_symbol_index_with_deadline(
    db_path: &Path,
    deadline: &WorkspaceScanDeadline,
) -> Result<crate::model::SymbolIndexHealth> {
    migrate_symbol_index_inner(db_path, Some(deadline))
}

fn migrate_symbol_index_inner(
    db_path: &Path,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<crate::model::SymbolIndexHealth> {
    check_optional_deadline(deadline, "preparing symbol index migration")?;
    let db_path = normalize_absolute_path(db_path)?;
    if !db_path.exists() {
        bail!("symbol index {} does not exist", db_path.display());
    }

    let mut connection = Connection::open(&db_path)?;
    check_optional_deadline(deadline, "opening symbol index for migration")?;
    let schema_version = match deadline {
        Some(deadline) => load_optional_metadata_value_with_deadline(
            &connection,
            "schema_version",
            Some(deadline),
        )?,
        None => load_optional_metadata_value(&connection, "schema_version")?,
    };
    let workspace_root = if schema_version
        .as_deref()
        .is_some_and(index_migration::is_migratable_symbol_index_schema_version)
    {
        require_symbol_index_tables(&connection, &db_path)?;
        if schema_version.as_deref() == Some(PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION) {
            require_previous_symbol_index_schema(&connection, &db_path)?;
        } else if schema_version.as_deref() == Some(LEGACY_SYMBOL_INDEX_SCHEMA_VERSION) {
            require_legacy_symbol_index_schema(&connection, &db_path)?;
        } else {
            require_older_symbol_index_schema(&connection, &db_path)?;
        }
        check_optional_deadline(deadline, "validating migratable symbol index schema")?;
        let workspace_root = match deadline {
            Some(deadline) => load_symbol_index_workspace_root_with_deadline(
                &connection,
                &db_path,
                Some(deadline),
            )?,
            None => load_symbol_index_workspace_root(&connection, &db_path)?,
        };
        let file_states = match deadline {
            Some(deadline) => load_file_states_with_deadline(&connection, Some(deadline))?,
            None => load_legacy_file_states(&connection)?,
        };
        let (symbols, indexed_files) = match deadline {
            Some(deadline) => load_resolved_symbols_with_deadline(&connection, Some(deadline))?,
            None => load_resolved_symbols(&connection)?,
        };
        validate_indexed_file_count(indexed_files, file_states.len())?;
        match deadline {
            Some(deadline) => validate_persisted_index_paths_with_overrides_and_deadline(
                &workspace_root,
                &file_states,
                &symbols,
                None,
                Some(deadline),
            )?,
            None => validate_persisted_index_paths(&workspace_root, &file_states, &symbols)?,
        }
        Some(workspace_root)
    } else {
        None
    };
    match deadline {
        Some(deadline) => {
            index_migration::migrate_symbol_index_with_deadline(
                &mut connection,
                &db_path,
                deadline,
            )?;
        }
        None => index_migration::migrate_symbol_index(&mut connection, &db_path)?,
    }
    drop(connection);

    // Schema migration is the final timeout gate. Once it starts, finish the
    // required rebuild and health inspection rather than reporting a timeout
    // after the database may already have changed.
    if let Some(workspace_root) = workspace_root {
        rebuild_symbol_index(&workspace_root, &db_path)?;
    }
    inspect_symbol_index(&db_path)
}

fn check_optional_deadline(deadline: Option<&WorkspaceScanDeadline>, phase: &str) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(())
}
