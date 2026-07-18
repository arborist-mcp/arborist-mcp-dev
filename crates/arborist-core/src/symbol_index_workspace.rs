use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::Connection;

use crate::index_schema::ensure_symbol_tables;
use crate::index_store::{load_file_states, load_indexed_symbols_grouped_by_file};
use crate::language::{
    c_include_targets, c_local_include_targets, detect_language, normalize_absolute_path,
    normalize_path, parse_document, path_is_inside_workspace, read_source, resolve_local_c_include,
};
use crate::model::{LanguageId, SymbolMeta};
use crate::symbol_dependency::{
    assign_symbol_ids, resolve_symbol_dependencies, resolve_symbol_dependencies_with_overrides,
};
use crate::symbol_extractor::index_symbols_from_document;
use crate::symbol_index_model::{IndexedSymbol, PersistedFileState};
use crate::symbol_index_state::source_fingerprint;
use crate::workspace_scan::{
    WorkspaceScanDeadline, WorkspaceScanLimits, collect_source_files,
    collect_source_files_with_deadline, should_skip_index_path, validate_source_file_size,
};

pub(crate) type IncrementalWorkspaceSymbols = (
    Vec<IndexedSymbol>,
    Vec<SymbolMeta>,
    Vec<PersistedFileState>,
    usize,
    usize,
    usize,
);

pub(crate) fn expanded_refresh_file_paths(
    workspace_root: &Path,
    file_path: &Path,
    deadline: &WorkspaceScanDeadline,
) -> Result<Vec<PathBuf>> {
    let mut refresh_paths = BTreeSet::new();
    refresh_paths.insert(file_path.to_path_buf());

    if matches!(detect_language(file_path)?, LanguageId::C | LanguageId::Cpp) {
        refresh_paths.extend(transitive_c_include_dependents_with_deadline(
            workspace_root,
            file_path,
            deadline,
        )?);
    }

    Ok(refresh_paths.into_iter().collect())
}

#[cfg(test)]
pub(crate) fn transitive_c_include_dependents(
    workspace_root: &Path,
    target_path: &Path,
) -> Result<BTreeSet<PathBuf>> {
    let deadline = WorkspaceScanDeadline::new(WorkspaceScanLimits::default())?;
    transitive_c_include_dependents_with_deadline(workspace_root, target_path, &deadline)
}

fn transitive_c_include_dependents_with_deadline(
    workspace_root: &Path,
    target_path: &Path,
    deadline: &WorkspaceScanDeadline,
) -> Result<BTreeSet<PathBuf>> {
    let reverse_index = reverse_local_c_include_index(workspace_root, deadline)?;
    let normalized_target = normalize_path(target_path);
    let mut queue = vec![normalized_target.clone()];
    let mut visited = BTreeSet::from([normalized_target]);
    let mut dependents = BTreeSet::new();

    while let Some(current_path) = queue.pop() {
        deadline.check("expanding C include dependents")?;
        let Some(children) = reverse_index.get(&current_path) else {
            continue;
        };

        for dependent_path in children {
            let normalized_dependent = normalize_path(dependent_path);
            if visited.insert(normalized_dependent.clone()) {
                dependents.insert(dependent_path.clone());
                queue.push(normalized_dependent);
            }
        }
    }

    Ok(dependents)
}

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
    let mut indexed_paths = collect_source_files(&workspace_root)?;
    let mut known_paths: BTreeSet<String> = indexed_paths
        .iter()
        .map(|path| normalize_path(path))
        .collect();

    for override_path in file_overrides.keys() {
        let override_path = normalize_absolute_path(Path::new(override_path))?;
        if !path_is_inside_workspace(&workspace_root, &override_path)?
            || should_skip_index_path(&workspace_root, &override_path)
            || detect_language(&override_path).is_err()
        {
            continue;
        }

        let normalized_path = normalize_path(&override_path);
        if known_paths.insert(normalized_path) {
            indexed_paths.push(override_path);
        }
    }

    indexed_paths.sort();
    let indexed_files = indexed_paths.len();
    let raw_symbols = build_workspace_index(&indexed_paths, Some(file_overrides))?;
    let resolved_symbols =
        resolve_symbol_dependencies_with_overrides(&raw_symbols, Some(file_overrides));
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

fn reverse_local_c_include_index(
    workspace_root: &Path,
    deadline: &WorkspaceScanDeadline,
) -> Result<BTreeMap<String, BTreeSet<PathBuf>>> {
    let mut reverse_index = BTreeMap::new();

    for path in collect_source_files_with_deadline(
        workspace_root,
        WorkspaceScanLimits::default(),
        deadline,
    )? {
        deadline.check("building C include reverse index")?;
        if !matches!(detect_language(&path), Ok(LanguageId::C | LanguageId::Cpp)) {
            continue;
        }

        let source = read_source(&path)?;
        let document = parse_document(&path, &source)?;
        let local_include_targets = c_local_include_targets(document.tree.root_node(), &source)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        for include_target in c_include_targets(document.tree.root_node(), &source)? {
            let Some(include_path) =
                resolve_local_c_include(&path, &include_target).or_else(|| {
                    local_include_targets
                        .contains(&include_target)
                        .then(|| unresolved_local_c_include_path(&path, &include_target))
                        .flatten()
                })
            else {
                continue;
            };
            if !path_is_inside_workspace(workspace_root, &include_path)? {
                continue;
            }

            reverse_index
                .entry(normalize_path(&include_path))
                .or_insert_with(BTreeSet::new)
                .insert(path.clone());
        }
    }

    Ok(reverse_index)
}

fn unresolved_local_c_include_path(current_path: &Path, include_target: &str) -> Option<PathBuf> {
    let parent = current_path.parent()?;
    normalize_absolute_path(&parent.join(include_target)).ok()
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
