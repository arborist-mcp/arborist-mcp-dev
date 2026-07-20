use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use rusqlite::Connection;

use crate::index_migration;
use crate::index_schema::{
    PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION, SYMBOL_INDEX_SCHEMA_VERSION, load_indexed_files_metadata,
    load_optional_metadata_value, load_symbol_index_workspace_root, open_symbol_index_read_only,
    require_current_symbol_index_schema, require_legacy_symbol_index_schema,
    require_previous_symbol_index_schema, require_symbol_index_tables,
    validate_symbol_index_schema_version,
};
use crate::index_store::{
    count_table_rows, load_file_states, load_indexed_symbols_grouped_by_file, load_resolved_symbols,
};
use crate::language::{
    detect_language, normalize_absolute_path, normalize_path, parse_document,
    path_is_inside_workspace, read_source,
};
use crate::model::{SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION, SymbolIndexHealth, SymbolMeta};
use crate::source_overlay::normalize_source_overrides_for_workspace;
use crate::symbol_dependency::{
    assign_symbol_ids, materialize_resolved_symbol_rows, refresh_resolved_symbol_subgraph,
};
use crate::symbol_extractor::index_symbols_from_document;
use crate::symbol_map::resolved_symbol_map;
use crate::symbols::rebuild_symbol_index;
use crate::workspace_scan::{
    DEFAULT_WORKSPACE_MAX_FILES, WorkspaceScanDeadline, WorkspaceScanLimits,
    collect_source_files_with_deadline, collect_source_files_with_limits, should_skip_index_path,
};

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
    deadline.check("loading persisted index state")?;

    if let (Some(workspace_root), Some(file_states)) =
        (workspace_root.as_deref(), file_states.as_ref())
    {
        let paths_valid = match validate_persisted_file_state_paths(workspace_root, file_states) {
            Ok(()) => true,
            Err(error) => {
                health.issues.push(error.to_string());
                false
            }
        };
        if let Some(resolved_symbols) = resolved_symbols.as_deref()
            && let Err(error) =
                validate_persisted_symbol_paths(workspace_root, file_states, resolved_symbols)
        {
            health.issues.push(error.to_string());
        }
        if paths_valid {
            inspect_symbol_index_freshness(&mut health, file_states, &deadline)?;
            match unindexed_workspace_files(workspace_root, file_states, None, Some(&deadline)) {
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

pub fn migrate_symbol_index(db_path: &Path) -> Result<SymbolIndexHealth> {
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

pub(crate) fn load_symbol_index(db_path: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    if !db_path.exists() {
        return Err(anyhow!("symbol index {} does not exist", db_path.display()));
    }

    let connection = open_symbol_index_read_only(db_path)?;
    require_symbol_index_tables(&connection, db_path)?;
    let indexed_files = load_indexed_files_metadata(&connection)?;
    validate_symbol_index_schema_version(&connection, db_path)?;
    require_current_symbol_index_schema(&connection, db_path)?;
    let file_states = load_file_states(&connection)?;
    let resolved_symbols = load_resolved_symbols(&connection)?;
    validate_indexed_file_count(indexed_files, file_states.len())?;
    let workspace_root = load_symbol_index_workspace_root(&connection, db_path)?;
    validate_persisted_index_paths(&workspace_root, &file_states, &resolved_symbols.0)?;
    ensure_symbol_index_fresh(db_path, &workspace_root, &file_states, None)?;
    Ok(resolved_symbols)
}

pub(crate) fn load_symbol_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    if !db_path.exists() {
        return Err(anyhow!("symbol index {} does not exist", db_path.display()));
    }

    let connection = open_symbol_index_read_only(db_path)?;
    require_symbol_index_tables(&connection, db_path)?;
    validate_symbol_index_schema_version(&connection, db_path)?;
    require_current_symbol_index_schema(&connection, db_path)?;
    let workspace_root = load_symbol_index_workspace_root(&connection, db_path)?;
    let file_overrides = normalize_source_overrides_for_workspace(
        &workspace_root,
        file_overrides,
        "indexed workspace",
    )?;

    let mut grouped_symbols = load_indexed_symbols_grouped_by_file(&connection)?;
    let original_grouped_symbols = grouped_symbols.clone();
    let persisted_file_states = load_file_states(&connection)?;
    let (resolved_symbols, persisted_indexed_files) = load_resolved_symbols(&connection)?;
    validate_indexed_file_count(persisted_indexed_files, persisted_file_states.len())?;
    validate_persisted_index_paths(&workspace_root, &persisted_file_states, &resolved_symbols)?;
    ensure_symbol_index_fresh(
        db_path,
        &workspace_root,
        &persisted_file_states,
        Some(&file_overrides),
    )?;
    let mut changed_file_paths = BTreeSet::new();
    let mut added_file_paths = BTreeSet::new();

    for (override_path, override_source) in &file_overrides {
        let override_path = Path::new(override_path);

        let document = parse_document(override_path, override_source)?;
        let symbols = index_symbols_from_document(override_path, override_source, &document)?;
        let normalized_path = normalize_path(override_path);
        if !persisted_file_states.contains_key(&normalized_path) {
            added_file_paths.insert(normalized_path.clone());
        }
        grouped_symbols.insert(normalized_path.clone(), symbols);
        changed_file_paths.insert(normalized_path);
    }

    let mut raw_symbols = grouped_symbols
        .into_values()
        .flat_map(|symbols| symbols.into_iter())
        .collect::<Vec<_>>();
    assign_symbol_ids(&mut raw_symbols)?;

    let old_resolved_map = resolved_symbol_map(&resolved_symbols);
    let old_changed_symbols = original_grouped_symbols
        .iter()
        .filter(|(file_path, _)| changed_file_paths.contains(*file_path))
        .flat_map(|(_, symbols)| symbols.iter().cloned())
        .collect::<Vec<_>>();
    let new_changed_symbols = raw_symbols
        .iter()
        .filter(|symbol| changed_file_paths.contains(&symbol.file_path))
        .cloned()
        .collect::<Vec<_>>();
    let (resolved_map, _) = refresh_resolved_symbol_subgraph(
        &raw_symbols,
        &old_resolved_map,
        &old_changed_symbols,
        &new_changed_symbols,
        &changed_file_paths,
        Some(&file_overrides),
    );
    let indexed_files = persisted_indexed_files + added_file_paths.len();

    Ok((
        materialize_resolved_symbol_rows(&raw_symbols, &resolved_map),
        indexed_files,
    ))
}

pub(crate) fn source_fingerprint(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

fn inspect_symbol_index_freshness(
    health: &mut SymbolIndexHealth,
    file_states: &BTreeMap<String, u64>,
    deadline: &WorkspaceScanDeadline,
) -> Result<()> {
    let mut fresh_files = 0;
    for (file_path, stored_fingerprint) in file_states {
        deadline.check("inspecting indexed file freshness")?;
        let path = Path::new(file_path);
        if !path.exists() {
            health.missing_files.push(file_path.clone());
            health
                .issues
                .push(format!("indexed file is missing: {file_path}"));
            continue;
        }

        match read_source(path) {
            Ok(source) => {
                let current_fingerprint = source_fingerprint(&source);
                if current_fingerprint == *stored_fingerprint {
                    fresh_files += 1;
                } else {
                    health.stale_files.push(file_path.clone());
                    health
                        .issues
                        .push(format!("indexed file is stale: {file_path}"));
                }
            }
            Err(error) => {
                health.unreadable_files.push(file_path.clone());
                health
                    .issues
                    .push(format!("failed to read indexed file {file_path}: {error}"));
            }
        }
    }
    health.fresh_file_count = Some(fresh_files);
    Ok(())
}

fn ensure_symbol_index_fresh(
    db_path: &Path,
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<()> {
    let mut issues = symbol_index_freshness_issues(file_states, file_overrides);
    issues.extend(
        unindexed_workspace_files(workspace_root, file_states, file_overrides, None)?
            .into_iter()
            .map(|file_path| format!("workspace source file is not indexed: {file_path}")),
    );
    if issues.is_empty() {
        return Ok(());
    }

    bail!(
        "symbol index {} is stale; refresh_symbol_index_for_file or rebuild_symbol_index before querying: {}",
        db_path.display(),
        issues.join("; ")
    );
}

fn validate_indexed_file_count(indexed_files: usize, file_state_entries: usize) -> Result<()> {
    if indexed_files != file_state_entries {
        bail!(
            "indexed_files metadata {indexed_files} does not match file_state entries {file_state_entries}"
        );
    }
    Ok(())
}

pub(crate) fn validate_persisted_index_paths(
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
    symbols: &[SymbolMeta],
) -> Result<()> {
    validate_persisted_file_state_paths(workspace_root, file_states)?;
    validate_persisted_symbol_paths(workspace_root, file_states, symbols)
}

fn validate_persisted_file_state_paths(
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
) -> Result<()> {
    for file_path in file_states.keys() {
        validate_persisted_source_path(workspace_root, file_path, "file_state.file_path")?;
    }
    Ok(())
}

fn validate_persisted_symbol_paths(
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
    symbols: &[SymbolMeta],
) -> Result<()> {
    for symbol in symbols {
        validate_persisted_source_path(workspace_root, &symbol.file_path, "symbols.file_path")?;
        if !file_states.contains_key(&symbol.file_path) {
            bail!(
                "persisted symbol path {} has no matching file_state entry",
                symbol.file_path
            );
        }
    }
    Ok(())
}

fn validate_persisted_source_path(
    workspace_root: &Path,
    file_path: &str,
    field_name: &str,
) -> Result<()> {
    let path = Path::new(file_path);
    let normalized_path = normalize_absolute_path(path)?;
    if normalize_path(&normalized_path) != file_path {
        bail!("persisted {field_name} is not a normalized absolute path: {file_path}");
    }
    if !path_is_inside_workspace(workspace_root, &normalized_path)? {
        bail!(
            "persisted {field_name} {} is outside indexed workspace {}",
            file_path,
            workspace_root.display()
        );
    }
    if should_skip_index_path(workspace_root, &normalized_path) {
        bail!("persisted {field_name} is inside an ignored workspace directory: {file_path}");
    }
    if detect_language(&normalized_path).is_err() {
        bail!("persisted {field_name} is not a supported source file: {file_path}");
    }
    Ok(())
}

fn unindexed_workspace_files(
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<String>> {
    let max_files = file_states
        .len()
        .saturating_add(DEFAULT_WORKSPACE_MAX_FILES);
    let limits = WorkspaceScanLimits::with_max_files(max_files);
    let paths = match deadline {
        Some(deadline) => collect_source_files_with_deadline(workspace_root, limits, deadline)?,
        None => collect_source_files_with_limits(workspace_root, limits)?,
    };
    Ok(paths
        .into_iter()
        .map(|path| normalize_path(&path))
        .filter(|path| {
            !file_states.contains_key(path)
                && !file_overrides.is_some_and(|overrides| overrides.contains_key(path))
        })
        .collect())
}

fn symbol_index_freshness_issues(
    file_states: &BTreeMap<String, u64>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Vec<String> {
    let mut issues = Vec::new();
    for (file_path, stored_fingerprint) in file_states {
        if file_overrides.is_some_and(|overrides| overrides.contains_key(file_path)) {
            continue;
        }

        let path = Path::new(file_path);
        if !path.exists() {
            issues.push(format!("indexed file is missing: {file_path}"));
            continue;
        }

        match read_source(path) {
            Ok(source) => {
                let current_fingerprint = source_fingerprint(&source);
                if current_fingerprint != *stored_fingerprint {
                    issues.push(format!("indexed file is stale: {file_path}"));
                }
            }
            Err(error) => {
                issues.push(format!("failed to read indexed file {file_path}: {error}"));
            }
        }
    }
    issues
}
