use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::language::detect_language;

const SKIPPED_WORKSPACE_DIR_NAMES: &[&str] = &[
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

pub const DEFAULT_WORKSPACE_MAX_FILES: usize = 20_000;

#[derive(Debug, Clone, Copy)]
pub struct WorkspaceScanLimits {
    pub max_files: usize,
}

impl Default for WorkspaceScanLimits {
    fn default() -> Self {
        Self {
            max_files: DEFAULT_WORKSPACE_MAX_FILES,
        }
    }
}

pub(crate) fn collect_source_files(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    collect_source_files_with_limits(workspace_root, WorkspaceScanLimits::default())
}

pub(crate) fn collect_source_files_with_limits(
    workspace_root: &Path,
    limits: WorkspaceScanLimits,
) -> Result<Vec<PathBuf>> {
    validate_workspace_scan_limits(limits)?;
    let mut files = Vec::new();
    walk_workspace(workspace_root, &mut files, limits)?;
    files.sort();
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

fn validate_workspace_scan_limits(limits: WorkspaceScanLimits) -> Result<()> {
    if limits.max_files == 0 {
        bail!("invalid workspace scan max_files: value must be greater than zero");
    }
    Ok(())
}

fn walk_workspace(
    path: &Path,
    files: &mut Vec<PathBuf>,
    limits: WorkspaceScanLimits,
) -> Result<()> {
    if path.is_dir() {
        if should_skip_dir(path) {
            return Ok(());
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            walk_workspace(&entry.path(), files, limits)?;
        }
        return Ok(());
    }

    if detect_language(path).is_ok() {
        if files.len() >= limits.max_files {
            bail!(
                "workspace scan file limit exceeded: max_files={}",
                limits.max_files
            );
        }
        files.push(path.to_path_buf());
    }

    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(should_skip_dir_name)
}

fn should_skip_dir_name(name: &str) -> bool {
    SKIPPED_WORKSPACE_DIR_NAMES
        .iter()
        .any(|skipped| name.eq_ignore_ascii_case(skipped))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        SKIPPED_WORKSPACE_DIR_NAMES, WorkspaceScanLimits, collect_source_files_with_limits,
        should_skip_dir_name, should_skip_index_path,
    };

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn recognizes_skipped_workspace_directory_names() {
        for name in SKIPPED_WORKSPACE_DIR_NAMES {
            assert!(
                should_skip_dir_name(name),
                "{name} should be skipped during workspace indexing"
            );
            assert!(
                should_skip_dir_name(&name.to_ascii_uppercase()),
                "{name} should be skipped case-insensitively during workspace indexing"
            );
        }

        for name in ["src", "venv-tools", "node_modules_backup", "targeted"] {
            assert!(
                !should_skip_dir_name(name),
                "{name} should not be skipped by partial name matching"
            );
        }
    }

    #[test]
    fn recognizes_skipped_workspace_path_segments() {
        let workspace = temporary_dir();
        let source_path = workspace.join("src").join("helper.py");
        let venv_path = workspace.join(".venv").join("installed.py");
        let similarly_named_path = workspace.join("venv-tools").join("helper.py");
        let sibling_workspace_path = workspace
            .parent()
            .unwrap()
            .join("other-workspace")
            .join(".venv")
            .join("installed.py");

        assert!(!should_skip_index_path(&workspace, &source_path));
        assert!(should_skip_index_path(&workspace, &venv_path));
        assert!(!should_skip_index_path(&workspace, &similarly_named_path));
        assert!(!should_skip_index_path(&workspace, &sibling_workspace_path));
    }

    #[test]
    fn collect_source_files_rejects_workspace_file_limit_overflow() {
        let workspace = temporary_dir();
        fs::write(workspace.join("a.py"), "def a():\n    return 1\n").unwrap();
        fs::write(workspace.join("b.py"), "def b():\n    return 2\n").unwrap();

        let error =
            collect_source_files_with_limits(&workspace, WorkspaceScanLimits { max_files: 1 })
                .expect_err("workspace scans should reject more files than max_files");

        assert!(error.to_string().contains("file limit exceeded"));
        assert!(error.to_string().contains("max_files=1"));
    }

    #[test]
    fn collect_source_files_skips_ignored_dirs_before_file_limit() {
        let workspace = temporary_dir();
        let skipped = workspace.join(".venv");
        fs::create_dir_all(&skipped).unwrap();
        fs::write(skipped.join("ignored.py"), "def ignored():\n    return 1\n").unwrap();
        fs::write(workspace.join("kept.py"), "def kept():\n    return 2\n").unwrap();

        let files =
            collect_source_files_with_limits(&workspace, WorkspaceScanLimits { max_files: 1 })
                .unwrap();

        assert_eq!(files, vec![workspace.join("kept.py")]);
    }

    fn temporary_dir() -> PathBuf {
        let suffix = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(format!("arborist-workspace-scan-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
