use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{normalize_absolute_path, normalize_path, parse_document, read_source};
use crate::model::SymbolMeta;
use crate::source_overlay::normalize_source_overrides_for_workspace;
use crate::symbol_dependency::{
    assign_symbol_ids, resolve_symbol_dependencies, resolve_symbol_dependencies_with_overrides,
};
use crate::symbol_extractor::index_symbols_from_document;
use crate::symbol_index_model::IndexedSymbol;
use crate::workspace_scan::collect_source_files;

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
