use std::path::Path;

use anyhow::{Result, anyhow};

use crate::deadline::DeadlineCheck;
use crate::index_migration;
use crate::index_schema::{
    ANCIENT_SYMBOL_INDEX_SCHEMA_VERSION, LEGACY_SYMBOL_INDEX_SCHEMA_VERSION,
    OLDER_SYMBOL_INDEX_SCHEMA_VERSION, OLDEST_SYMBOL_INDEX_SCHEMA_VERSION,
    PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION, SYMBOL_INDEX_SCHEMA_VERSION,
    load_indexed_files_metadata_with_deadline, load_optional_metadata_value_with_deadline,
    load_symbol_index_workspace_root_with_deadline, open_symbol_index_read_only_with_deadline,
    require_current_symbol_index_schema_with_deadline,
    require_legacy_symbol_index_schema_with_deadline,
    require_older_symbol_index_schema_with_deadline,
    require_oldest_symbol_index_schema_with_deadline,
    require_previous_symbol_index_schema_with_deadline, require_symbol_index_tables_with_deadline,
    validate_symbol_index_analysis_provenance_with_deadline,
};
use crate::index_store::{
    count_table_rows_with_deadline, load_file_states_with_deadline,
    load_indexed_symbols_grouped_by_file_with_deadline, load_resolved_symbols_with_deadline,
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

    let connection = match open_symbol_index_read_only_with_deadline(&db_path, Some(&deadline)) {
        Ok(connection) => connection,
        Err(error) => {
            deadline.check("opening persisted index")?;
            health
                .issues
                .push(format!("failed to open symbol index: {error}"));
            health.migration = index_migration::incomplete_or_foreign_database();
            health.validate_public_output()?;
            return Ok(health);
        }
    };

    let table_validation = require_symbol_index_tables_with_deadline(
        &connection,
        &db_path,
        Some(&deadline as &dyn DeadlineCheck),
    );
    deadline.check("validating persisted index tables")?;
    if let Err(error) = table_validation {
        health.issues.push(error.to_string());
        health.migration = index_migration::incomplete_or_foreign_database();
        health.validate_public_output()?;
        return Ok(health);
    }

    health.schema_version = load_optional_metadata_value_with_deadline(
        &connection,
        "schema_version",
        Some(&deadline as &dyn DeadlineCheck),
    )
    .map_err(|error| {
        anyhow!(
            "failed to inspect schema_version metadata in {}: {}",
            db_path.display(),
            error
        )
    })?;
    deadline.check("loading schema metadata")?;
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
                require_previous_symbol_index_schema_with_deadline(
                    &connection,
                    &db_path,
                    Some(&deadline as &dyn DeadlineCheck),
                )
            } else if health.schema_version.as_deref() == Some(LEGACY_SYMBOL_INDEX_SCHEMA_VERSION) {
                require_legacy_symbol_index_schema_with_deadline(
                    &connection,
                    &db_path,
                    Some(&deadline as &dyn DeadlineCheck),
                )
            } else if health.schema_version.as_deref() == Some(OLDER_SYMBOL_INDEX_SCHEMA_VERSION) {
                require_older_symbol_index_schema_with_deadline(
                    &connection,
                    &db_path,
                    Some(&deadline as &dyn DeadlineCheck),
                )
            } else {
                debug_assert!(matches!(
                    health.schema_version.as_deref(),
                    Some(OLDEST_SYMBOL_INDEX_SCHEMA_VERSION | ANCIENT_SYMBOL_INDEX_SCHEMA_VERSION)
                ));
                require_oldest_symbol_index_schema_with_deadline(
                    &connection,
                    &db_path,
                    Some(&deadline as &dyn DeadlineCheck),
                )
            };
        deadline.check("validating migratable persisted index schema")?;
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
    } else {
        let current_schema_validation = require_current_symbol_index_schema_with_deadline(
            &connection,
            &db_path,
            Some(&deadline as &dyn DeadlineCheck),
        );
        deadline.check("validating persisted index schema")?;
        if let Err(error) = current_schema_validation {
            health.issues.push(error.to_string());
            health.migration = index_migration::incomplete_or_foreign_database();
            health.validate_public_output()?;
            return Ok(health);
        }

        let provenance_validation = validate_symbol_index_analysis_provenance_with_deadline(
            &connection,
            &db_path,
            Some(&deadline as &dyn DeadlineCheck),
        );
        deadline.check("validating index analysis provenance")?;
        if let Err(error) = provenance_validation {
            health.issues.push(error.to_string());
            health.migration = index_migration::failed_health_checks();
        }
    }

    let workspace_root = match load_symbol_index_workspace_root_with_deadline(
        &connection,
        &db_path,
        Some(&deadline as &dyn DeadlineCheck),
    ) {
        Ok(workspace_root) => {
            health.workspace_root = Some(normalize_path(&workspace_root));
            Some(workspace_root)
        }
        Err(error) => {
            health.issues.push(error.to_string());
            None
        }
    };
    deadline.check("loading indexed workspace root")?;

    match load_indexed_files_metadata_with_deadline(
        &connection,
        Some(&deadline as &dyn DeadlineCheck),
    ) {
        Ok(indexed_files) => health.indexed_files = Some(indexed_files),
        Err(error) => health.issues.push(error.to_string()),
    }

    match count_table_rows_with_deadline(&connection, "symbols", Some(&deadline)) {
        Ok(count) => health.indexed_symbols = Some(count),
        Err(error) => health
            .issues
            .push(format!("failed to count persisted symbols: {error}")),
    }
    deadline.check("counting persisted symbols")?;
    match count_table_rows_with_deadline(&connection, "file_state", Some(&deadline)) {
        Ok(count) => health.file_state_entries = Some(count),
        Err(error) => health
            .issues
            .push(format!("failed to count persisted file states: {error}")),
    }
    deadline.check("counting persisted file states")?;

    let file_states = match load_file_states_with_deadline(&connection, Some(&deadline)) {
        Ok(file_states) => Some(file_states),
        Err(error) => {
            health
                .issues
                .push(format!("failed to inspect persisted file states: {error}"));
            None
        }
    };
    deadline.check("loading persisted file states")?;
    let resolved_symbols = match load_resolved_symbols_with_deadline(&connection, Some(&deadline)) {
        Ok((symbols, _)) => Some(symbols),
        Err(error) => {
            health
                .issues
                .push(format!("failed to inspect persisted symbols: {error}"));
            None
        }
    };
    deadline.check("loading persisted symbols")?;
    if let Err(error) = load_indexed_symbols_grouped_by_file_with_deadline(&connection, &deadline) {
        health
            .issues
            .push(format!("failed to inspect persisted raw symbols: {error}"));
    }
    deadline.check("loading persisted raw symbols")?;

    if let (Some(workspace_root), Some(file_states)) =
        (workspace_root.as_deref(), file_states.as_ref())
    {
        let paths_valid = match path_state::validate_persisted_file_state_paths_with_deadline(
            workspace_root,
            file_states,
            Some(&deadline),
        ) {
            Ok(()) => true,
            Err(error) => {
                record_persisted_path_validation_error(
                    &mut health,
                    &deadline,
                    "validating persisted file state paths",
                    error,
                )?;
                false
            }
        };
        if let Some(resolved_symbols) = resolved_symbols.as_deref()
            && let Err(error) = path_state::validate_persisted_symbol_paths_with_deadline(
                workspace_root,
                file_states,
                resolved_symbols,
                None,
                Some(&deadline as &dyn DeadlineCheck),
            )
        {
            record_persisted_path_validation_error(
                &mut health,
                &deadline,
                "validating persisted symbol paths",
                error,
            )?;
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
                Err(error) => record_unindexed_workspace_scan_error(&mut health, &deadline, error)?,
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

fn record_persisted_path_validation_error(
    health: &mut SymbolIndexHealth,
    deadline: &WorkspaceScanDeadline,
    phase: &str,
    error: anyhow::Error,
) -> Result<()> {
    deadline.check(phase)?;
    health.issues.push(error.to_string());
    Ok(())
}
fn record_unindexed_workspace_scan_error(
    health: &mut SymbolIndexHealth,
    deadline: &WorkspaceScanDeadline,
    error: anyhow::Error,
) -> Result<()> {
    deadline.check("scanning indexed workspace for unindexed files")?;
    health.issues.push(format!(
        "failed to scan indexed workspace for unindexed files: {error}"
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use anyhow::anyhow;

    use super::{record_persisted_path_validation_error, record_unindexed_workspace_scan_error};
    use crate::model::SymbolIndexHealth;
    use crate::workspace_scan::WorkspaceScanDeadline;

    fn test_health() -> SymbolIndexHealth {
        SymbolIndexHealth {
            response_schema_version: "4".to_owned(),
            db_path: "C:\\workspace\\symbols.db".to_owned(),
            exists: true,
            ok: false,
            schema_version: None,
            expected_schema_version: "4".to_owned(),
            migration: crate::index_migration::pending_inspection(),
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
        }
    }

    #[test]
    fn unindexed_scan_timeout_is_not_downgraded_to_a_health_issue() {
        let mut health = test_health();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = record_unindexed_workspace_scan_error(
            &mut health,
            &deadline,
            anyhow!("workspace scan timeout exceeded"),
        )
        .expect_err("expired unindexed scans should fail closed");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
        assert!(health.issues.is_empty());
    }

    #[test]
    fn persisted_path_validation_timeout_is_not_downgraded_to_a_health_issue() {
        let mut health = test_health();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = record_persisted_path_validation_error(
            &mut health,
            &deadline,
            "validating persisted file state paths",
            anyhow!("workspace scan timeout exceeded"),
        )
        .expect_err("expired persisted path validation should fail closed");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
        assert!(health.issues.is_empty());
    }
}
