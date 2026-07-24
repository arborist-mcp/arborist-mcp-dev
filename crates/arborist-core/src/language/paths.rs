use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Result, bail};

pub fn normalize_absolute_path(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        bail!("invalid path: path must not be empty");
    }

    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    let mut normalized = PathBuf::new();
    for component in absolute_path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
}

pub(crate) fn ensure_path_inside_workspace(workspace_root: &Path, path: &Path) -> Result<()> {
    if path_is_inside_workspace(workspace_root, path)? {
        return Ok(());
    }

    bail!(
        "file {} is outside workspace {}",
        path.display(),
        workspace_root.display()
    )
}

pub(crate) fn path_is_inside_workspace(workspace_root: &Path, path: &Path) -> Result<bool> {
    if !path.starts_with(workspace_root) {
        return Ok(false);
    }

    let canonical_workspace = canonicalize_with_existing_ancestor(workspace_root)?;
    let canonical_path = canonicalize_with_existing_ancestor(path)?;
    Ok(canonical_path.starts_with(&canonical_workspace))
}

fn canonicalize_with_existing_ancestor(path: &Path) -> Result<PathBuf> {
    let normalized = normalize_absolute_path(path)?;
    let mut missing_components = Vec::new();
    let mut probe = normalized.as_path();

    while !probe.exists() {
        let Some(file_name) = probe.file_name() else {
            return Ok(normalized);
        };
        missing_components.push(file_name.to_os_string());
        let Some(parent) = probe.parent() else {
            return Ok(normalized);
        };
        probe = parent;
    }

    let mut canonical = fs::canonicalize(probe)?;
    for component in missing_components.iter().rev() {
        canonical.push(component);
    }
    normalize_absolute_path(&canonical)
}

pub fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
