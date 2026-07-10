use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::language;

pub(crate) fn source_override_for_path(
    path: &Path,
    source: &str,
) -> Result<(PathBuf, BTreeMap<String, String>)> {
    let path = language::normalize_absolute_path(path)?;
    let mut overrides = BTreeMap::new();
    overrides.insert(language::normalize_path(&path), source.to_string());
    Ok((path, overrides))
}

pub(crate) fn source_overrides_for_workspace_path(
    workspace_root: &Path,
    path: &Path,
    source: &str,
) -> Result<(PathBuf, PathBuf, BTreeMap<String, String>)> {
    let workspace_root = language::normalize_absolute_path(workspace_root)?;
    let (path, overrides) = source_override_for_path(path, source)?;
    ensure_path_inside_workspace(&workspace_root, &path)?;
    Ok((workspace_root, path, overrides))
}

pub(crate) fn ensure_path_inside_workspace(workspace_root: &Path, path: &Path) -> Result<()> {
    if path.starts_with(workspace_root) {
        return Ok(());
    }

    bail!(
        "file {} is outside workspace {}",
        path.display(),
        workspace_root.display()
    );
}
