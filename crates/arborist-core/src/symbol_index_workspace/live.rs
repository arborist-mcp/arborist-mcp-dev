use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::language::{
    normalize_absolute_path, normalize_path, parse_document, parse_document_with_timeout,
    path_identity, read_source,
};
use crate::model::SymbolMeta;
use crate::source_overlay::{
    normalize_source_overrides_for_workspace,
    normalize_source_overrides_for_workspace_with_deadline,
};
use crate::symbol_dependency::{
    assign_symbol_ids, assign_symbol_ids_with_deadline, resolve_symbol_dependencies,
    resolve_symbol_dependencies_with_overrides,
    resolve_symbol_dependencies_with_overrides_with_deadline,
};
use crate::symbol_extractor::{
    index_symbols_from_document, index_symbols_from_document_with_deadline,
};
use crate::symbol_index_model::IndexedSymbol;
use crate::workspace_scan::{
    WorkspaceScanDeadline, WorkspaceScanLimits, collect_source_files,
    collect_source_files_with_deadline, collect_source_files_with_limits,
    validate_source_file_size,
};

pub(crate) fn load_live_workspace_symbols(
    workspace_root: &Path,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    resolve_workspace_symbols(&workspace_root)
}

pub(crate) fn load_live_workspace_symbols_with_timeout(
    workspace_root: &Path,
    timeout_ms: Option<u64>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    resolve_workspace_symbols_with_timeout(&workspace_root, timeout_ms)
}

pub(crate) fn resolve_workspace_symbols(workspace_root: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    let indexed_paths = collect_source_files(workspace_root)?;
    let indexed_files = indexed_paths.len();
    let raw_symbols = build_workspace_index(&indexed_paths, None)?;
    let resolved_symbols = resolve_symbol_dependencies(&raw_symbols, &indexed_paths);
    Ok((resolved_symbols, indexed_files))
}

pub(crate) fn resolve_workspace_symbols_with_timeout(
    workspace_root: &Path,
    timeout_ms: Option<u64>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let limits = WorkspaceScanLimits {
        timeout_ms,
        ..WorkspaceScanLimits::default()
    };
    let deadline = WorkspaceScanDeadline::new(limits)?;
    let indexed_paths = collect_source_files_with_deadline(workspace_root, limits, &deadline)?;
    let indexed_files = indexed_paths.len();
    let raw_symbols = build_workspace_index_with_deadline(&indexed_paths, None, limits, &deadline)?;
    let resolved_symbols = resolve_symbol_dependencies_with_overrides_with_deadline(
        &raw_symbols,
        &indexed_paths,
        None,
        &deadline,
    )?;
    Ok((resolved_symbols, indexed_files))
}

pub(crate) fn resolve_workspace_symbols_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_overrides =
        normalize_source_overrides_for_workspace(&workspace_root, file_overrides, "workspace")?;
    let limits = WorkspaceScanLimits::default();
    let mut indexed_paths = collect_source_files_with_limits(&workspace_root, limits)?;
    let file_overrides = remap_overrides_to_indexed_paths(&indexed_paths, &file_overrides)?;
    append_override_paths(&mut indexed_paths, &file_overrides, limits.max_files)?;
    let indexed_files = indexed_paths.len();
    let raw_symbols = build_workspace_index(&indexed_paths, Some(&file_overrides))?;
    let resolved_symbols = resolve_symbol_dependencies_with_overrides(
        &raw_symbols,
        &indexed_paths,
        Some(&file_overrides),
    );
    Ok((resolved_symbols, indexed_files))
}

pub(crate) fn resolve_workspace_symbols_with_overrides_with_timeout(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    timeout_ms: Option<u64>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let limits = WorkspaceScanLimits {
        timeout_ms,
        ..WorkspaceScanLimits::default()
    };
    let deadline = WorkspaceScanDeadline::new(limits)?;
    deadline.check("normalizing workspace source overlays")?;
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_overrides = normalize_source_overrides_for_workspace_with_deadline(
        &workspace_root,
        file_overrides,
        "workspace",
        Some(&deadline),
    )?;
    let mut indexed_paths = collect_source_files_with_deadline(&workspace_root, limits, &deadline)?;
    let file_overrides =
        remap_overrides_to_indexed_paths_with_deadline(&indexed_paths, &file_overrides, &deadline)?;
    append_override_paths_with_deadline(
        &mut indexed_paths,
        &file_overrides,
        limits.max_files,
        &deadline,
    )?;
    let indexed_files = indexed_paths.len();
    let raw_symbols = build_workspace_index_with_deadline(
        &indexed_paths,
        Some(&file_overrides),
        limits,
        &deadline,
    )?;
    let resolved_symbols = resolve_symbol_dependencies_with_overrides_with_deadline(
        &raw_symbols,
        &indexed_paths,
        Some(&file_overrides),
        &deadline,
    )?;
    Ok((resolved_symbols, indexed_files))
}

fn remap_overrides_to_indexed_paths(
    indexed_paths: &[PathBuf],
    file_overrides: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    remap_overrides_to_indexed_paths_with_optional_deadline(indexed_paths, file_overrides, None)
}

fn remap_overrides_to_indexed_paths_with_deadline(
    indexed_paths: &[PathBuf],
    file_overrides: &BTreeMap<String, String>,
    deadline: &WorkspaceScanDeadline,
) -> Result<BTreeMap<String, String>> {
    remap_overrides_to_indexed_paths_with_optional_deadline(
        indexed_paths,
        file_overrides,
        Some(deadline),
    )
}

fn remap_overrides_to_indexed_paths_with_optional_deadline(
    indexed_paths: &[PathBuf],
    file_overrides: &BTreeMap<String, String>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<BTreeMap<String, String>> {
    if let Some(deadline) = deadline {
        deadline.check("remapping workspace source overlays")?;
    }

    let mut indexed_paths_by_identity = BTreeMap::new();
    for path in indexed_paths {
        if let Some(deadline) = deadline {
            deadline.check("remapping workspace source overlays")?;
        }
        let normalized_path = normalize_path(path);
        let identity = path_identity(&normalized_path);
        if indexed_paths_by_identity
            .insert(identity, normalized_path.clone())
            .is_some()
        {
            bail!(
                "workspace scan contains multiple case-insensitive paths for {}",
                normalized_path
            );
        }
    }

    let mut remapped_overrides = BTreeMap::new();
    for (file_path, source) in file_overrides {
        if let Some(deadline) = deadline {
            deadline.check("remapping workspace source overlays")?;
        }
        let normalized_path = normalize_path(Path::new(file_path));
        let resolved_path = indexed_paths_by_identity
            .get(&path_identity(&normalized_path))
            .cloned()
            .unwrap_or(normalized_path);
        if remapped_overrides
            .insert(resolved_path.clone(), source.clone())
            .is_some()
        {
            bail!("source overlay contains duplicate file path {resolved_path}");
        }
    }
    Ok(remapped_overrides)
}

fn append_override_paths(
    indexed_paths: &mut Vec<PathBuf>,
    file_overrides: &BTreeMap<String, String>,
    max_files: usize,
) -> Result<()> {
    let mut known_paths: BTreeSet<String> = indexed_paths
        .iter()
        .map(|path| path_identity(&normalize_path(path)))
        .collect();

    for override_path in file_overrides.keys() {
        let override_path = Path::new(override_path).to_path_buf();
        let normalized_path = normalize_path(&override_path);
        if known_paths.insert(path_identity(&normalized_path)) {
            if indexed_paths.len() >= max_files {
                bail!(
                    "workspace scan exceeded max_files while adding source overlays: max_files={max_files}"
                );
            }
            indexed_paths.push(override_path);
        }
    }

    indexed_paths.sort();
    Ok(())
}

fn append_override_paths_with_deadline(
    indexed_paths: &mut Vec<PathBuf>,
    file_overrides: &BTreeMap<String, String>,
    max_files: usize,
    deadline: &WorkspaceScanDeadline,
) -> Result<()> {
    let mut known_paths: BTreeSet<String> = indexed_paths
        .iter()
        .map(|path| path_identity(&normalize_path(path)))
        .collect();

    for override_path in file_overrides.keys() {
        deadline.check("adding workspace overrides")?;
        let override_path = Path::new(override_path).to_path_buf();
        let normalized_path = normalize_path(&override_path);
        if known_paths.insert(path_identity(&normalized_path)) {
            if indexed_paths.len() >= max_files {
                bail!(
                    "workspace scan exceeded max_files while adding source overlays: max_files={max_files}"
                );
            }
            indexed_paths.push(override_path);
        }
    }

    indexed_paths.sort();
    Ok(())
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

fn build_workspace_index_with_deadline(
    paths: &[PathBuf],
    file_overrides: Option<&BTreeMap<String, String>>,
    limits: WorkspaceScanLimits,
    deadline: &WorkspaceScanDeadline,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();

    for path in paths {
        deadline.check("indexing workspace files")?;
        validate_source_file_size(path, limits)?;
        let normalized_path = normalize_path(path);
        let source = match file_overrides.and_then(|overrides| overrides.get(&normalized_path)) {
            Some(source) => source.clone(),
            None => read_source(path)?,
        };
        deadline.check("parsing workspace files")?;
        let document = parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing workspace files")?,
        )?;
        symbols.extend(index_symbols_from_document_with_deadline(
            path,
            &source,
            &document,
            Some(deadline),
        )?);
    }

    deadline.check("assigning symbol identities")?;
    assign_symbol_ids_with_deadline(&mut symbols, deadline)?;
    Ok(symbols)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use super::{append_override_paths, remap_overrides_to_indexed_paths_with_deadline};
    use crate::workspace_scan::WorkspaceScanDeadline;

    #[test]
    fn override_paths_respect_workspace_file_limit() {
        let mut indexed_paths = vec![PathBuf::from("existing.py")];
        let overrides = BTreeMap::from([
            ("one.py".to_string(), String::new()),
            ("two.py".to_string(), String::new()),
        ]);

        let error = append_override_paths(&mut indexed_paths, &overrides, 2)
            .expect_err("source overlays must not bypass the workspace file limit");
        assert!(error.to_string().contains("max_files=2"));
        assert_eq!(indexed_paths.len(), 2);
    }

    #[test]
    fn override_paths_do_not_count_existing_files_twice() {
        let mut indexed_paths = vec![PathBuf::from("existing.py")];
        let overrides = BTreeMap::from([("existing.py".to_string(), String::new())]);

        append_override_paths(&mut indexed_paths, &overrides, 1)
            .expect("an override for an indexed file should not consume another slot");
        assert_eq!(indexed_paths.len(), 1);
    }

    #[test]
    fn remapping_override_paths_rejects_expired_deadline() {
        let indexed_paths = vec![PathBuf::from("existing.py")];
        let overrides = BTreeMap::from([("existing.py".to_string(), String::new())]);
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error =
            remap_overrides_to_indexed_paths_with_deadline(&indexed_paths, &overrides, &deadline)
                .expect_err("expired deadline should reject workspace overlay remapping");

        assert!(
            error
                .to_string()
                .contains("remapping workspace source overlays")
        );
    }
}
