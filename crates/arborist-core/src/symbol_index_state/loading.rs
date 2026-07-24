use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow};

use crate::index_schema::{
    load_indexed_files_metadata, load_symbol_index_workspace_root, open_symbol_index_read_only,
    require_current_symbol_index_schema, require_symbol_index_tables,
    validate_symbol_index_schema_version,
};
use crate::index_store::{
    load_file_states, load_indexed_symbols_grouped_by_file, load_resolved_symbols,
};
use crate::language::{normalize_path, parse_document};
use crate::model::SymbolMeta;
use crate::source_overlay::normalize_source_overrides_for_workspace;
use crate::symbol_dependency::{
    assign_symbol_ids, materialize_resolved_symbol_rows, refresh_resolved_symbol_subgraph,
};
use crate::symbol_extractor::index_symbols_from_document;
use crate::symbol_map::resolved_symbol_map;

use super::paths::{validate_persisted_index_paths, validate_persisted_index_paths_with_overrides};
use super::state::{ensure_symbol_index_fresh, validate_indexed_file_count};

pub(crate) fn load_symbol_index(db_path: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    if !db_path.exists() {
        return Err(anyhow!("symbol index {} does not exist", db_path.display()));
    }

    let connection = open_symbol_index_read_only(db_path)?;
    require_symbol_index_tables(&connection, db_path)?;
    let indexed_files = load_indexed_files_metadata(&connection)?;
    validate_symbol_index_schema_version(&connection, db_path)?;
    require_current_symbol_index_schema(&connection, db_path)?;
    load_indexed_symbols_grouped_by_file(&connection)?;
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
    validate_persisted_index_paths_with_overrides(
        &workspace_root,
        &persisted_file_states,
        &resolved_symbols,
        Some(&file_overrides),
    )?;
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
