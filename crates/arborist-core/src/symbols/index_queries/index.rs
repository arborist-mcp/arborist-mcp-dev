use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

use crate::index_schema::{
    load_indexed_files_metadata, require_current_symbol_index_schema, require_symbol_index_tables,
    validate_symbol_index_schema_version, validate_symbol_index_workspace,
};
use crate::index_store::{
    SymbolRefreshPersistence, load_file_states, load_indexed_symbols_grouped_by_file,
    load_resolved_symbols, persist_symbol_index, persist_symbol_refresh,
};
use crate::language::{
    ensure_path_inside_workspace, normalize_absolute_path, normalize_path, parse_document,
    read_source,
};
use crate::model::SymbolIndexStats;
use crate::symbol_dependency::{
    assign_symbol_ids, materialize_resolved_symbol_rows, refresh_resolved_symbol_subgraph,
};
use crate::symbol_extractor::index_symbols_from_document;
use crate::symbol_index_state::{source_fingerprint, validate_persisted_index_paths};
use crate::symbol_index_workspace::{
    expanded_refresh_file_paths, resolve_workspace_symbols_incremental_with_limits,
};
use crate::symbol_map::resolved_symbol_map;
use crate::workspace_scan::{
    WorkspaceScanLimits, should_skip_index_path, validate_source_file_size,
    validate_workspace_scan_limits,
};

pub fn rebuild_symbol_index(workspace_root: &Path, db_path: &Path) -> Result<SymbolIndexStats> {
    rebuild_symbol_index_with_limits(workspace_root, db_path, WorkspaceScanLimits::default())
}

pub fn refresh_symbol_index(workspace_root: &Path, db_path: &Path) -> Result<SymbolIndexStats> {
    refresh_symbol_index_with_limits(workspace_root, db_path, WorkspaceScanLimits::default())
}

pub fn refresh_symbol_index_with_limits(
    workspace_root: &Path,
    db_path: &Path,
    limits: WorkspaceScanLimits,
) -> Result<SymbolIndexStats> {
    rebuild_symbol_index_with_limits(workspace_root, db_path, limits)
}

pub fn rebuild_symbol_index_with_limits(
    workspace_root: &Path,
    db_path: &Path,
    limits: WorkspaceScanLimits,
) -> Result<SymbolIndexStats> {
    validate_workspace_scan_limits(limits)?;
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let db_path = normalize_absolute_path(db_path)?;
    if db_path.exists() {
        let connection = Connection::open(&db_path)?;
        require_symbol_index_tables(&connection, &db_path)?;
        validate_symbol_index_workspace(&connection, &workspace_root, &db_path)?;
        load_indexed_files_metadata(&connection)?;
        validate_symbol_index_schema_version(&connection, &db_path)?;
        require_current_symbol_index_schema(&connection, &db_path)?;
    }
    let (raw_symbols, resolved_symbols, file_states, indexed_files, rebuilt_files, reused_files) =
        resolve_workspace_symbols_incremental_with_limits(&workspace_root, &db_path, limits)?;
    persist_symbol_index(
        &db_path,
        &workspace_root,
        &raw_symbols,
        &resolved_symbols,
        &file_states,
        indexed_files,
    )?;

    let result = SymbolIndexStats {
        db_path: normalize_path(&db_path),
        indexed_files,
        indexed_symbols: resolved_symbols.len(),
        rebuilt_files,
        reused_files,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub fn refresh_symbol_index_for_file(
    workspace_root: &Path,
    db_path: &Path,
    file_path: &Path,
) -> Result<SymbolIndexStats> {
    refresh_symbol_index_for_file_with_limits(
        workspace_root,
        db_path,
        file_path,
        WorkspaceScanLimits::default(),
    )
}

pub fn refresh_symbol_index_for_file_with_limits(
    workspace_root: &Path,
    db_path: &Path,
    file_path: &Path,
    limits: WorkspaceScanLimits,
) -> Result<SymbolIndexStats> {
    validate_workspace_scan_limits(limits)?;
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;

    ensure_path_inside_workspace(&workspace_root, &file_path)?;

    if !db_path.exists() {
        return rebuild_symbol_index_with_limits(&workspace_root, &db_path, limits);
    }

    let connection = Connection::open(&db_path)?;
    require_symbol_index_tables(&connection, &db_path)?;
    validate_symbol_index_workspace(&connection, &workspace_root, &db_path)?;
    load_indexed_files_metadata(&connection)?;
    validate_symbol_index_schema_version(&connection, &db_path)?;
    require_current_symbol_index_schema(&connection, &db_path)?;

    let old_resolved_symbols = load_resolved_symbols(&connection)?.0;
    let old_resolved_map = resolved_symbol_map(&old_resolved_symbols);
    let mut grouped_symbols = load_indexed_symbols_grouped_by_file(&connection)?;
    let refresh_paths = if should_skip_index_path(&workspace_root, &file_path) {
        vec![file_path.clone()]
    } else {
        expanded_refresh_file_paths(&workspace_root, &file_path)?
    };

    let mut file_states = load_file_states(&connection)?;
    validate_persisted_index_paths(&workspace_root, &file_states, &old_resolved_symbols)?;
    let mut old_changed_symbols = Vec::new();
    let mut changed_file_paths = BTreeSet::new();
    let mut rebuilt_files = 0;

    for refresh_path in &refresh_paths {
        let normalized_refresh_path = normalize_path(refresh_path);
        let skip_refresh_path = should_skip_index_path(&workspace_root, refresh_path);
        let had_indexed_state = file_states.contains_key(&normalized_refresh_path)
            || grouped_symbols.contains_key(&normalized_refresh_path);
        old_changed_symbols.extend(
            grouped_symbols
                .get(&normalized_refresh_path)
                .cloned()
                .unwrap_or_default(),
        );

        if refresh_path.exists() && !skip_refresh_path {
            validate_source_file_size(refresh_path, limits)?;
            let source = read_source(refresh_path)?;
            let document = parse_document(refresh_path, &source)?;
            let fresh_symbols = index_symbols_from_document(refresh_path, &source, &document)?;

            file_states.insert(normalized_refresh_path.clone(), source_fingerprint(&source));
            grouped_symbols.insert(normalized_refresh_path.clone(), fresh_symbols);
            rebuilt_files += 1;
        } else {
            file_states.remove(&normalized_refresh_path);
            grouped_symbols.remove(&normalized_refresh_path);
            if had_indexed_state {
                rebuilt_files += 1;
            }
        }
        changed_file_paths.insert(normalized_refresh_path);
    }

    let mut raw_symbols = grouped_symbols
        .into_values()
        .flat_map(|symbols| symbols.into_iter())
        .collect::<Vec<_>>();
    assign_symbol_ids(&mut raw_symbols)?;
    let new_changed_symbols = raw_symbols
        .iter()
        .filter(|symbol| changed_file_paths.contains(&symbol.file_path))
        .cloned()
        .collect::<Vec<_>>();
    let (resolved_map, impacted_paths) = refresh_resolved_symbol_subgraph(
        &raw_symbols,
        &old_resolved_map,
        &old_changed_symbols,
        &new_changed_symbols,
        &changed_file_paths,
    );
    let resolved_symbols = materialize_resolved_symbol_rows(&raw_symbols, &resolved_map);
    let indexed_files = file_states.len();
    let reused_files = indexed_files.saturating_sub(rebuilt_files);

    persist_symbol_refresh(SymbolRefreshPersistence {
        db_path: &db_path,
        workspace_root: &workspace_root,
        raw_symbols: &raw_symbols,
        symbols: &resolved_symbols,
        resolved_symbols_by_id: &resolved_map,
        file_states: &file_states,
        changed_file_paths: &changed_file_paths,
        impacted_paths: &impacted_paths,
        indexed_files,
    })?;

    let result = SymbolIndexStats {
        db_path: normalize_path(&db_path),
        indexed_files,
        indexed_symbols: resolved_symbols.len(),
        rebuilt_files,
        reused_files,
    };
    result.validate_public_output()?;
    Ok(result)
}
