use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::limits::{
    WorkspaceScanDeadline, WorkspaceScanLimits, validate_source_file_size,
    validate_workspace_scan_limits,
};
use crate::language::{detect_language, path_is_inside_workspace};

pub(crate) const SKIPPED_WORKSPACE_DIR_NAMES: &[&str] = &[
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".venv",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "target",
    "venv",
];

pub(crate) fn collect_source_files(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    collect_source_files_with_limits(workspace_root, WorkspaceScanLimits::default())
}

pub(crate) fn collect_source_files_with_limits(
    workspace_root: &Path,
    limits: WorkspaceScanLimits,
) -> Result<Vec<PathBuf>> {
    let deadline = WorkspaceScanDeadline::new(limits)?;
    collect_source_files_with_deadline(workspace_root, limits, &deadline)
}

pub(crate) fn collect_source_files_with_deadline(
    workspace_root: &Path,
    limits: WorkspaceScanLimits,
    deadline: &WorkspaceScanDeadline,
) -> Result<Vec<PathBuf>> {
    validate_workspace_scan_limits(limits)?;
    let mut files = Vec::new();
    walk_workspace(workspace_root, workspace_root, &mut files, limits, deadline)?;
    deadline.check("sorting workspace files")?;
    files.sort();
    deadline.check("completing workspace scan")?;
    Ok(files)
}

pub(crate) fn should_skip_index_path(workspace_root: &Path, path: &Path) -> bool {
    path.strip_prefix(workspace_root)
        .ok()
        .is_some_and(|relative_path| {
            relative_path.components().any(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .is_some_and(should_skip_dir_name)
            })
        })
}

fn walk_workspace(
    workspace_root: &Path,
    path: &Path,
    files: &mut Vec<PathBuf>,
    limits: WorkspaceScanLimits,
    deadline: &WorkspaceScanDeadline,
) -> Result<()> {
    deadline.check("workspace traversal")?;
    let symlink_metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect workspace path {}", path.display()))?;
    let is_symlink = symlink_metadata.file_type().is_symlink();
    let metadata = if is_symlink {
        fs::metadata(path)
            .with_context(|| format!("failed to inspect workspace path {}", path.display()))?
    } else {
        symlink_metadata
    };

    if path != workspace_root && is_symlink && metadata.is_dir() {
        return Ok(());
    }

    if !path_is_inside_workspace(workspace_root, path)? {
        return Ok(());
    }

    if metadata.is_dir() {
        if should_skip_dir(path) {
            return Ok(());
        }

        let mut entries = fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort();

        for entry_path in entries {
            walk_workspace(workspace_root, &entry_path, files, limits, deadline)?;
        }
        return Ok(());
    }

    if detect_language(path).is_ok() {
        validate_source_file_size(path, limits)?;
        if files.len() >= limits.max_files {
            bail!(
                "workspace scan file limit exceeded at {}: max_files={}",
                path.display(),
                limits.max_files,
            );
        }
        files.push(path.to_path_buf());
    }

    Ok(())
}

pub(crate) fn should_skip_dir_name(name: &str) -> bool {
    SKIPPED_WORKSPACE_DIR_NAMES
        .iter()
        .any(|skipped| name.eq_ignore_ascii_case(skipped))
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(should_skip_dir_name)
}
