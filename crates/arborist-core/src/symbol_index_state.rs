use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use anyhow::{Result, anyhow};
use rusqlite::Connection;

use crate::index_schema::{
    SYMBOL_INDEX_SCHEMA_VERSION, ensure_symbol_tables, load_indexed_files_metadata,
    load_optional_metadata_value, load_symbol_index_workspace_root, require_symbol_index_tables,
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
use crate::symbol_dependency::{
    assign_symbol_ids, materialize_resolved_symbol_rows, refresh_resolved_symbol_subgraph,
};
use crate::symbol_extractor::index_symbols_from_document;
use crate::symbol_map::resolved_symbol_map;
use crate::workspace_scan::should_skip_index_path;

pub fn inspect_symbol_index(db_path: &Path) -> Result<SymbolIndexHealth> {
    let db_path = normalize_absolute_path(db_path)?;
    let db_path_display = normalize_path(&db_path);
    let mut health = SymbolIndexHealth {
        response_schema_version: SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION.to_string(),
        db_path: db_path_display,
        exists: db_path.exists(),
        ok: false,
        schema_version: None,
        expected_schema_version: SYMBOL_INDEX_SCHEMA_VERSION.to_string(),
        workspace_root: None,
        indexed_files: None,
        indexed_symbols: None,
        file_state_entries: None,
        fresh_file_count: None,
        stale_files: Vec::new(),
        missing_files: Vec::new(),
        unreadable_files: Vec::new(),
        issues: Vec::new(),
    };

    if !health.exists {
        health
            .issues
            .push(format!("symbol index {} does not exist", db_path.display()));
        health.validate_public_output()?;
        return Ok(health);
    }

    let connection = match Connection::open(&db_path) {
        Ok(connection) => connection,
        Err(error) => {
            health
                .issues
                .push(format!("failed to open symbol index: {error}"));
            health.validate_public_output()?;
            return Ok(health);
        }
    };

    if let Err(error) = require_symbol_index_tables(&connection, &db_path) {
        health.issues.push(error.to_string());
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
    } else if health.schema_version.as_deref() != Some(SYMBOL_INDEX_SCHEMA_VERSION) {
        health.issues.push(format!(
            "unsupported symbol index schema_version `{}` in {}; expected `{}`",
            health.schema_version.as_deref().unwrap_or_default(),
            db_path.display(),
            SYMBOL_INDEX_SCHEMA_VERSION
        ));
    }

    match load_symbol_index_workspace_root(&connection, &db_path) {
        Ok(workspace_root) => health.workspace_root = Some(normalize_path(&workspace_root)),
        Err(error) => health.issues.push(error.to_string()),
    }

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

    match load_file_states(&connection) {
        Ok(file_states) => inspect_symbol_index_freshness(&mut health, &file_states),
        Err(error) => health
            .issues
            .push(format!("failed to inspect persisted file states: {error}")),
    }

    health.ok = health.issues.is_empty();
    health.validate_public_output()?;
    Ok(health)
}

pub(crate) fn load_symbol_index(db_path: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    if !db_path.exists() {
        return Err(anyhow!("symbol index {} does not exist", db_path.display()));
    }

    let connection = Connection::open(db_path)?;
    require_symbol_index_tables(&connection, db_path)?;
    load_indexed_files_metadata(&connection)?;
    validate_symbol_index_schema_version(&connection, db_path)?;
    ensure_symbol_tables(&connection)?;
    load_resolved_symbols(&connection)
}

pub(crate) fn load_symbol_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    if !db_path.exists() {
        return Err(anyhow!("symbol index {} does not exist", db_path.display()));
    }

    let connection = Connection::open(db_path)?;
    require_symbol_index_tables(&connection, db_path)?;
    let workspace_root = load_symbol_index_workspace_root(&connection, db_path)?;
    validate_symbol_index_schema_version(&connection, db_path)?;
    ensure_symbol_tables(&connection)?;

    let mut grouped_symbols = load_indexed_symbols_grouped_by_file(&connection)?;
    let original_grouped_symbols = grouped_symbols.clone();
    let persisted_file_states = load_file_states(&connection)?;
    let mut changed_file_paths = BTreeSet::new();
    let mut added_file_paths = BTreeSet::new();

    for (override_path, override_source) in file_overrides {
        let override_path = normalize_absolute_path(Path::new(override_path))?;
        if !path_is_inside_workspace(&workspace_root, &override_path)?
            || should_skip_index_path(&workspace_root, &override_path)
            || detect_language(&override_path).is_err()
        {
            continue;
        }

        let document = parse_document(&override_path, override_source)?;
        let symbols = index_symbols_from_document(&override_path, override_source, &document)?;
        let normalized_path = normalize_path(&override_path);
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

    let (resolved_symbols, persisted_indexed_files) = load_resolved_symbols(&connection)?;
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
) {
    let mut fresh_files = 0;
    for (file_path, stored_fingerprint) in file_states {
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
}
