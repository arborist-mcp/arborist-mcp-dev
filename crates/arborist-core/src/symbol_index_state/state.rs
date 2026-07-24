use std::path::Path;

use anyhow::{Result, anyhow};

use crate::index_migration;
use crate::index_schema::{
    PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION, SYMBOL_INDEX_SCHEMA_VERSION, load_indexed_files_metadata,
    load_optional_metadata_value, load_symbol_index_workspace_root, open_symbol_index_read_only,
    require_current_symbol_index_schema, require_legacy_symbol_index_schema,
    require_previous_symbol_index_schema, require_symbol_index_tables,
};
use crate::index_store::{
    count_table_rows, load_file_states, load_indexed_symbols_grouped_by_file, load_resolved_symbols,
};
use crate::language::{normalize_absolute_path, normalize_path};
use crate::model::{SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION, SymbolIndexHealth};
use crate::workspace_scan::{WorkspaceScanDeadline, WorkspaceScanLimits};

use super::freshness::{inspect_symbol_index_freshness, validate_indexed_file_count};
use super::paths as path_state;

pub fn inspect_symbol_index(db_path: &Path) -> Result<SymbolIndexHealth> {
    inspect_symbol_index_with_timeout(db_path, None)
}

pub fn inspect_symbol_index_with_timeout(
    db_path: &Path,
    timeout_ms: Option<u64>,
) -> Result<SymbolIndexHealth> {
    let deadline = WorkspaceScanDeadline::new(WorkspaceScanLimits {
        timeout_ms,
        ..WorkspaceScanLimits::default()
    })?;
    let db_path = normalize_absolute_path(db_path)?;
    let db_path_display = normalize_path(&db_path);
    let mut health = SymbolIndexHealth {
        response_schema_version: SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION.to_string(),
        db_path: db_path_display,
        exists: db_path.exists(),
        ok: false,
        schema_version: None,
        expected_schema_version: SYMBOL_INDEX_SCHEMA_VERSION.to_string(),
        migration: index_migration::pending_inspection(),
        workspace_root: None,
        indexed_files: None,
        indexed_symbols: None,
        file_state_entries: None,
        fresh_file_count: None,
        stale_files: Vec::new(),
        missing_files: Vec::new(),
        unreadable_files: Vec::new(),
        unindexed_files: Vec::new(),
        issues: Vec::new(),
    };

    if !health.exists {
        health
            .issues
            .push(format!("symbol index {} does not exist", db_path.display()));
        health.migration = index_migration::missing_index();
        health.validate_public_output()?;
        return Ok(health);
    }

    let connection = match open_symbol_index_read_only(&db_path) {
        Ok(connection) => connection,
        Err(error) => {
            health
                .issues
                .push(format!("failed to open symbol index: {error}"));
            health.migration = index_migration::incomplete_or_foreign_database();
            health.validate_public_output()?;
            return Ok(health);
        }
    };
    deadline.check("opening persisted index")?;

    if let Err(error) = require_symbol_index_tables(&connection, &db_path) {
        health.issues.push(error.to_string());
        health.migration = index_migration::incomplete_or_foreign_database();
        health.validate_public_output()?;
        return Ok(health);
    }

    health.schema_version =
        load_optional_metadata_value(&connection, "schema_version").map_err(|error| {
            anyhow!(
                "failed to inspect schema_version metadata in {}: {}",
                db_path.display(),
                error
            )
        })?;
    if health.schema_version.is_none() {
        health.issues.push(format!(
            "missing schema_version metadata in symbol index {}",
            db_path.display()
        ));
        health.migration = index_migration::missing_schema_version();
    } else if health
        .schema_version
        .as_deref()
        .is_some_and(index_migration::is_migratable_symbol_index_schema_version)
    {
        health.issues.push(format!(
            "unsupported symbol index schema_version `{}` in {}; expected `{}`",
            health.schema_version.as_deref().unwrap_or_default(),
            db_path.display(),
            SYMBOL_INDEX_SCHEMA_VERSION
        ));
        health.migration = index_migration::unsupported_schema_version(
            health.schema_version.as_deref().unwrap_or_default(),
        );
        let schema_validation =
            if health.schema_version.as_deref() == Some(PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION) {
                require_previous_symbol_index_schema(&connection, &db_path)
            } else {
                require_legacy_symbol_index_schema(&connection, &db_path)
            };
        if let Err(error) = schema_validation {
            health.issues.push(error.to_string());
            health.migration = index_migration::incomplete_or_foreign_database();
            health.validate_public_output()?;
            return Ok(health);
        }
    } else if health.schema_version.as_deref() != Some(SYMBOL_INDEX_SCHEMA_VERSION) {
        health.issues.push(format!(
            "unsupported symbol index schema_version `{}` in {}; expected `{}`",
            health.schema_version.as_deref().unwrap_or_default(),
            db_path.display(),
            SYMBOL_INDEX_SCHEMA_VERSION
        ));
        health.migration = index_migration::unsupported_schema_version(
            health.schema_version.as_deref().unwrap_or_default(),
        );
    } else if let Err(error) = require_current_symbol_index_schema(&connection, &db_path) {
        health.issues.push(error.to_string());
        health.migration = index_migration::incomplete_or_foreign_database();
        health.validate_public_output()?;
        return Ok(health);
    }

    let workspace_root = match load_symbol_index_workspace_root(&connection, &db_path) {
        Ok(workspace_root) => {
            health.workspace_root = Some(normalize_path(&workspace_root));
            Some(workspace_root)
        }
        Err(error) => {
            health.issues.push(error.to_string());
            None
        }
    };

    match load_indexed_files_metadata(&connection) {
        Ok(indexed_files) => health.indexed_files = Some(indexed_files),
        Err(error) => health.issues.push(error.to_string()),
    }

    match count_table_rows(&connection, "symbols") {
        Ok(count) => health.indexed_symbols = Some(count),
        Err(error) => health
            .issues
            .push(format!("failed to count persisted symbols: {error}")),
    }
    match count_table_rows(&connection, "file_state") {
        Ok(count) => health.file_state_entries = Some(count),
        Err(error) => health
            .issues
            .push(format!("failed to count persisted file states: {error}")),
    }

    let file_states = match load_file_states(&connection) {
        Ok(file_states) => Some(file_states),
        Err(error) => {
            health
                .issues
                .push(format!("failed to inspect persisted file states: {error}"));
            None
        }
    };
    let resolved_symbols = match load_resolved_symbols(&connection) {
        Ok((symbols, _)) => Some(symbols),
        Err(error) => {
            health
                .issues
                .push(format!("failed to inspect persisted symbols: {error}"));
            None
        }
    };
    if let Err(error) = load_indexed_symbols_grouped_by_file(&connection) {
        health
            .issues
            .push(format!("failed to inspect persisted raw symbols: {error}"));
    }
    deadline.check("loading persisted index state")?;

    if let (Some(workspace_root), Some(file_states)) =
        (workspace_root.as_deref(), file_states.as_ref())
    {
        let paths_valid =
            match path_state::validate_persisted_file_state_paths(workspace_root, file_states) {
                Ok(()) => true,
                Err(error) => {
                    health.issues.push(error.to_string());
                    false
                }
            };
        if let Some(resolved_symbols) = resolved_symbols.as_deref()
            && let Err(error) = path_state::validate_persisted_symbol_paths(
                workspace_root,
                file_states,
                resolved_symbols,
                None,
            )
        {
            health.issues.push(error.to_string());
        }
        if paths_valid {
            inspect_symbol_index_freshness(&mut health, file_states, &deadline)?;
            match path_state::unindexed_workspace_files(
                workspace_root,
                file_states,
                None,
                Some(&deadline),
            ) {
                Ok(unindexed_files) => {
                    for file_path in &unindexed_files {
                        health
                            .issues
                            .push(format!("workspace source file is not indexed: {file_path}"));
                    }
                    health.unindexed_files = unindexed_files;
                }
                Err(error) => health.issues.push(format!(
                    "failed to scan indexed workspace for unindexed files: {error}"
                )),
            }
        }
    }

    if let (Some(indexed_files), Some(file_state_entries)) =
        (health.indexed_files, health.file_state_entries)
        && let Err(error) = validate_indexed_file_count(indexed_files, file_state_entries)
    {
        health.issues.push(error.to_string());
    }

    health.ok = health.issues.is_empty();
    if health.ok {
        health.migration = index_migration::healthy_index();
    } else if !health.migration.required {
        health.migration = index_migration::failed_health_checks();
    }
    health.validate_public_output()?;
    Ok(health)
}
