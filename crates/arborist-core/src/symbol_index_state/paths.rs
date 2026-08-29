use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::index_store::validate_resolved_symbol_edges_with_deadline;
use crate::language::{
    detect_language, normalize_absolute_path, normalize_path, path_identity,
    path_is_inside_workspace, read_source,
};
use crate::model::SymbolMeta;
use crate::workspace_scan::{
    DEFAULT_WORKSPACE_MAX_FILES, MAX_WORKSPACE_SCAN_FILES, WorkspaceScanDeadline,
    WorkspaceScanLimits, collect_source_files_with_deadline, collect_source_files_with_limits,
    should_skip_index_path,
};

use super::fingerprints::source_fingerprint;

pub(crate) fn resolve_persisted_file_path(
    file_path: &Path,
    file_states: &BTreeMap<String, u64>,
) -> Result<PathBuf> {
    resolve_persisted_file_path_with_deadline(file_path, file_states, None)
}

fn resolve_persisted_file_path_with_deadline(
    file_path: &Path,
    file_states: &BTreeMap<String, u64>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<PathBuf> {
    if let Some(deadline) = deadline {
        deadline.check("remapping indexed source overlays")?;
    }

    let normalized_path = normalize_path(file_path);
    if !cfg!(windows) {
        return Ok(file_path.to_path_buf());
    }

    let normalized_identity = path_identity(&normalized_path);
    let mut matches = Vec::new();
    for persisted_path in file_states.keys() {
        if let Some(deadline) = deadline {
            deadline.check("remapping indexed source overlays")?;
        }
        if path_identity(persisted_path) == normalized_identity {
            matches.push(persisted_path);
        }
    }
    match matches.as_slice() {
        [] => Ok(file_path.to_path_buf()),
        [persisted_path] => Ok(PathBuf::from(persisted_path)),
        _ => bail!(
            "persisted index contains multiple case-insensitive file_state paths for {}",
            normalized_path
        ),
    }
}

pub(crate) fn remap_file_overrides_to_persisted_paths(
    file_overrides: &BTreeMap<String, String>,
    file_states: &BTreeMap<String, u64>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<BTreeMap<String, String>> {
    if let Some(deadline) = deadline {
        deadline.check("remapping indexed source overlays")?;
    }

    let mut remapped_overrides = BTreeMap::new();
    for (file_path, source) in file_overrides {
        if let Some(deadline) = deadline {
            deadline.check("remapping indexed source overlays")?;
        }
        let resolved_path =
            resolve_persisted_file_path_with_deadline(Path::new(file_path), file_states, deadline)?;
        let normalized_path = normalize_path(&resolved_path);
        if remapped_overrides
            .insert(normalized_path.clone(), source.clone())
            .is_some()
        {
            bail!("source overlay contains duplicate file path {normalized_path}");
        }
    }
    Ok(remapped_overrides)
}

pub(crate) fn validate_persisted_index_paths(
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
    symbols: &[SymbolMeta],
) -> Result<()> {
    validate_persisted_index_paths_with_overrides(workspace_root, file_states, symbols, None)
}

pub(crate) fn validate_persisted_index_paths_with_overrides(
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
    symbols: &[SymbolMeta],
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<()> {
    validate_persisted_index_paths_with_overrides_and_deadline(
        workspace_root,
        file_states,
        symbols,
        file_overrides,
        None,
    )
}

pub(crate) fn validate_persisted_index_paths_with_overrides_and_deadline(
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
    symbols: &[SymbolMeta],
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<()> {
    validate_persisted_file_state_paths_with_deadline(workspace_root, file_states, deadline)?;
    validate_persisted_symbol_paths_with_deadline(
        workspace_root,
        file_states,
        symbols,
        file_overrides,
        deadline,
    )
}

pub(super) fn validate_persisted_file_state_paths_with_deadline(
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<()> {
    for file_path in file_states.keys() {
        if let Some(deadline) = deadline {
            deadline.check("validating persisted file paths")?;
        }
        validate_persisted_source_path(workspace_root, file_path, "file_state.file_path")?;
    }
    Ok(())
}

pub(super) fn validate_persisted_symbol_paths_with_deadline(
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
    symbols: &[SymbolMeta],
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<()> {
    let mut sources_by_path = BTreeMap::new();
    let mut validated_paths = BTreeSet::new();
    for symbol in symbols {
        if let Some(deadline) = deadline {
            deadline.check("validating persisted symbol paths")?;
        }
        if validated_paths.insert(symbol.file_path.clone()) {
            validate_persisted_source_path(workspace_root, &symbol.file_path, "symbols.file_path")?;
        }
        if !file_states.contains_key(&symbol.file_path) {
            bail!(
                "persisted symbol path {} has no matching file_state entry",
                symbol.file_path
            );
        }
        let path = Path::new(&symbol.file_path);
        if path.exists()
            && !file_overrides.is_some_and(|overrides| overrides.contains_key(&symbol.file_path))
        {
            let source = match sources_by_path.entry(symbol.file_path.clone()) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => entry.insert(read_source(path)?),
            };
            if source
                .get(symbol.byte_range.0..symbol.byte_range.1)
                .is_none()
            {
                bail!(
                    "persisted symbol byte range {}..{} for {} is invalid for {}",
                    symbol.byte_range.0,
                    symbol.byte_range.1,
                    symbol.symbol_id,
                    symbol.file_path
                );
            }
        }
    }
    validate_resolved_symbol_edges_with_deadline(symbols, deadline)
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

pub(crate) fn unindexed_workspace_files(
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<String>> {
    let max_files = file_states
        .len()
        .saturating_add(DEFAULT_WORKSPACE_MAX_FILES)
        .min(MAX_WORKSPACE_SCAN_FILES);
    let limits = WorkspaceScanLimits::with_max_files(max_files);
    let paths = match deadline {
        Some(deadline) => collect_source_files_with_deadline(workspace_root, limits, deadline)?,
        None => collect_source_files_with_limits(workspace_root, limits)?,
    };
    let mut unindexed = Vec::new();
    for path in paths {
        if let Some(deadline) = deadline {
            deadline.check("filtering unindexed workspace files")?;
        }
        let path = normalize_path(&path);
        if !file_states.contains_key(&path)
            && !file_overrides.is_some_and(|overrides| overrides.contains_key(&path))
        {
            unindexed.push(path);
        }
    }
    if let Some(deadline) = deadline {
        deadline.check("filtering unindexed workspace files")?;
    }
    Ok(unindexed)
}

pub(crate) fn symbol_index_freshness_issues(
    file_states: &BTreeMap<String, u64>,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<String>> {
    let mut issues = Vec::new();
    for (file_path, stored_fingerprint) in file_states {
        if let Some(deadline) = deadline {
            deadline.check("checking indexed file freshness")?;
        }
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
    if let Some(deadline) = deadline {
        deadline.check("checking indexed file freshness")?;
    }
    Ok(issues)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};

    use super::{
        remap_file_overrides_to_persisted_paths, symbol_index_freshness_issues,
        validate_persisted_index_paths_with_overrides_and_deadline,
    };
    use crate::workspace_scan::WorkspaceScanDeadline;

    #[test]
    fn freshness_checks_reject_expired_deadline_before_reading_files() {
        let mut file_states = BTreeMap::new();
        file_states.insert("C:\\workspace\\source.py".to_owned(), 0);
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = symbol_index_freshness_issues(&file_states, None, Some(&deadline))
            .expect_err("expired freshness checks should stop before reading files");
        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }

    #[test]
    fn persisted_overlay_remapping_rejects_expired_deadline() {
        let file_overrides =
            BTreeMap::from([("C:\\workspace\\source.py".to_owned(), String::new())]);
        let file_states = BTreeMap::from([("C:\\workspace\\source.py".to_owned(), 0)]);
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error =
            remap_file_overrides_to_persisted_paths(&file_overrides, &file_states, Some(&deadline))
                .expect_err("expired deadline should reject persisted overlay remapping");

        assert!(
            error
                .to_string()
                .contains("remapping indexed source overlays")
        );
    }

    #[test]
    fn persisted_path_validation_rejects_expired_deadline_before_path_checks() {
        let mut file_states = BTreeMap::new();
        file_states.insert("C:\\workspace\\source.py".to_owned(), 0);
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = validate_persisted_index_paths_with_overrides_and_deadline(
            std::path::Path::new("C:\\workspace"),
            &file_states,
            &[],
            None,
            Some(&deadline),
        )
        .expect_err("expired path validation should stop before path normalization");
        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }
}
