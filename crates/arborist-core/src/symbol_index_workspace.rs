use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::Connection;

pub(crate) use crate::include_graph::expanded_refresh_file_paths;
#[cfg(test)]
pub(crate) use crate::include_graph::transitive_c_include_dependents;
use crate::index_schema::ensure_symbol_tables;
use crate::index_store::{load_file_states, load_indexed_symbols_grouped_by_file};
use crate::language::{normalize_absolute_path, normalize_path, parse_document, read_source};
use crate::model::SymbolMeta;
use crate::source_overlay::normalize_source_overrides_for_workspace;
use crate::symbol_dependency::{
    assign_symbol_ids, resolve_symbol_dependencies, resolve_symbol_dependencies_with_overrides,
};
use crate::symbol_extractor::index_symbols_from_document;
use crate::symbol_index_model::{IndexedSymbol, PersistedFileState};
use crate::symbol_index_state::source_fingerprint;
use crate::workspace_scan::{
    WorkspaceScanDeadline, WorkspaceScanLimits, collect_source_files,
    collect_source_files_with_deadline, validate_source_file_size,
};

pub(crate) type IncrementalWorkspaceSymbols = (
    Vec<IndexedSymbol>,
    Vec<SymbolMeta>,
    Vec<PersistedFileState>,
    usize,
    usize,
    usize,
);

pub(crate) fn load_live_workspace_symbols(
    workspace_root: &Path,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    resolve_workspace_symbols(&workspace_root)
}

pub(crate) fn resolve_workspace_symbols(workspace_root: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    let indexed_paths = collect_source_files(workspace_root)?;
    let indexed_files = indexed_paths.len();
    let raw_symbols = build_workspace_index(&indexed_paths, None)?;
    let resolved_symbols = resolve_symbol_dependencies(&raw_symbols);
    Ok((resolved_symbols, indexed_files))
}

pub(crate) fn resolve_workspace_symbols_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_overrides =
        normalize_source_overrides_for_workspace(&workspace_root, file_overrides, "workspace")?;
    let mut indexed_paths = collect_source_files(&workspace_root)?;
    let mut known_paths: BTreeSet<String> = indexed_paths
        .iter()
        .map(|path| normalize_path(path))
        .collect();

    for override_path in file_overrides.keys() {
        let override_path = Path::new(override_path).to_path_buf();
        let normalized_path = normalize_path(&override_path);
        if known_paths.insert(normalized_path) {
            indexed_paths.push(override_path);
        }
    }

    indexed_paths.sort();
    let indexed_files = indexed_paths.len();
    let raw_symbols = build_workspace_index(&indexed_paths, Some(&file_overrides))?;
    let resolved_symbols =
        resolve_symbol_dependencies_with_overrides(&raw_symbols, Some(&file_overrides));
    Ok((resolved_symbols, indexed_files))
}

pub(crate) fn resolve_workspace_symbols_incremental_with_deadline(
    workspace_root: &Path,
    db_path: &Path,
    limits: WorkspaceScanLimits,
    deadline: &WorkspaceScanDeadline,
) -> Result<IncrementalWorkspaceSymbols> {
    let indexed_paths = collect_source_files_with_deadline(workspace_root, limits, deadline)?;
    let indexed_files = indexed_paths.len();
    let connection = Connection::open(db_path)?;
    ensure_symbol_tables(&connection)?;

    let persisted_states = load_file_states(&connection)?;
    let persisted_symbols = load_indexed_symbols_grouped_by_file(&connection)?;

    let mut raw_symbols = Vec::new();
    let mut file_states = Vec::new();
    let mut rebuilt_files = 0;
    let mut reused_files = 0;

    for path in indexed_paths {
        deadline.check("indexing workspace files")?;
        validate_source_file_size(&path, limits)?;
        let source = read_source(&path)?;
        let normalized_path = normalize_path(&path);
        let fingerprint = source_fingerprint(&source);

        file_states.push(PersistedFileState {
            file_path: normalized_path.clone(),
            fingerprint,
        });

        if persisted_states
            .get(&normalized_path)
            .is_some_and(|stored| *stored == fingerprint)
            && let Some(stored_symbols) = persisted_symbols.get(&normalized_path)
        {
            raw_symbols.extend(stored_symbols.iter().cloned());
            reused_files += 1;
            continue;
        }

        let document = parse_document(&path, &source)?;
        raw_symbols.extend(index_symbols_from_document(&path, &source, &document)?);
        rebuilt_files += 1;
    }

    deadline.check("assigning symbol identities")?;
    assign_symbol_ids(&mut raw_symbols)?;
    deadline.check("resolving workspace symbols")?;
    let resolved_symbols = resolve_symbol_dependencies(&raw_symbols);
    Ok((
        raw_symbols,
        resolved_symbols,
        file_states,
        indexed_files,
        rebuilt_files,
        reused_files,
    ))
}

fn build_workspace_index(
    paths: &[PathBuf],
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();

    for path in paths {
        let normalized_path = normalize_path(path);
        let source = match file_overrides.and_then(|overrides| overrides.get(&normalized_path)) {
            Some(source) => source.clone(),
            None => read_source(path)?,
        };
        let document = parse_document(path, &source)?;
        symbols.extend(index_symbols_from_document(path, &source, &document)?);
    }

    assign_symbol_ids(&mut symbols)?;
    Ok(symbols)
}
