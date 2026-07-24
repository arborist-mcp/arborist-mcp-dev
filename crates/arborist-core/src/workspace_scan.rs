mod limits;
mod walker;
pub use limits::{DEFAULT_WORKSPACE_MAX_FILES, MAX_WORKSPACE_SCAN_TIMEOUT_MS, WorkspaceScanLimits};
pub(crate) use limits::{
    WorkspaceScanDeadline, validate_source_file_size, validate_workspace_scan_limits,
};
#[cfg(test)]
pub(crate) use walker::{SKIPPED_WORKSPACE_DIR_NAMES, should_skip_dir_name};
pub(crate) use walker::{
    collect_source_files, collect_source_files_with_deadline, collect_source_files_with_limits,
    should_skip_index_path,
};

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        DEFAULT_WORKSPACE_MAX_FILES, MAX_WORKSPACE_SCAN_TIMEOUT_MS, SKIPPED_WORKSPACE_DIR_NAMES,
        WorkspaceScanDeadline, WorkspaceScanLimits, collect_source_files_with_limits,
        should_skip_dir_name, should_skip_index_path, validate_workspace_scan_limits,
    };
    use std::time::{Duration, Instant};

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
            collect_source_files_with_limits(&workspace, WorkspaceScanLimits::with_max_files(1))
                .expect_err("workspace scans should reject more files than max_files");

        assert!(error.to_string().contains("file limit exceeded"));
        assert!(error.to_string().contains("max_files=1"));
    }

    #[test]
    fn collect_source_files_reports_deterministic_limit_overflow_path() {
        let workspace = temporary_dir();
        fs::write(workspace.join("b.py"), "def b():\n    return 2\n").unwrap();
        fs::write(workspace.join("a.py"), "def a():\n    return 1\n").unwrap();

        let error =
            collect_source_files_with_limits(&workspace, WorkspaceScanLimits::with_max_files(1))
                .expect_err("workspace scans should reject more files than max_files");

        assert!(error.to_string().contains("b.py"));
    }

    #[test]
    fn collect_source_files_skips_ignored_dirs_before_file_limit() {
        let workspace = temporary_dir();
        let skipped = workspace.join(".venv");
        fs::create_dir_all(&skipped).unwrap();
        fs::write(skipped.join("ignored.py"), "def ignored():\n    return 1\n").unwrap();
        fs::write(workspace.join("kept.py"), "def kept():\n    return 2\n").unwrap();

        let files =
            collect_source_files_with_limits(&workspace, WorkspaceScanLimits::with_max_files(1))
                .unwrap();

        assert_eq!(files, vec![workspace.join("kept.py")]);
    }

    #[test]
    fn collect_source_files_skips_symlink_directory_escape() {
        let root = temporary_dir();
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(workspace.join("kept.py"), "def kept():\n    return 1\n").unwrap();
        fs::write(outside.join("secret.py"), "def secret():\n    return 2\n").unwrap();

        if !try_symlink_dir(&outside, &workspace.join("linked")) {
            let _ = fs::remove_dir_all(root);
            return;
        }

        let files =
            collect_source_files_with_limits(&workspace, WorkspaceScanLimits::with_max_files(2))
                .unwrap();

        assert_eq!(files, vec![workspace.join("kept.py")]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn collect_source_files_accepts_symlink_workspace_root() {
        let root = temporary_dir();
        let workspace = root.join("workspace");
        let workspace_link = root.join("workspace-link");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("kept.py"), "def kept():\n    return 1\n").unwrap();

        if !try_symlink_dir(&workspace, &workspace_link) {
            let _ = fs::remove_dir_all(root);
            return;
        }

        let files = collect_source_files_with_limits(
            &workspace_link,
            WorkspaceScanLimits::with_max_files(1),
        )
        .unwrap();

        assert_eq!(files, vec![workspace_link.join("kept.py")]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn collect_source_files_rejects_source_file_size_overflow() {
        let workspace = temporary_dir();
        let oversized = workspace.join("huge.py");
        fs::write(&oversized, "def huge():\n    return 'too much'\n").unwrap();

        let error = collect_source_files_with_limits(
            &workspace,
            WorkspaceScanLimits {
                max_files: 10,
                max_file_bytes: Some(8),
                timeout_ms: None,
            },
        )
        .expect_err("workspace scans should reject source files larger than max_file_bytes");

        assert!(error.to_string().contains("source file too large"));
        assert!(error.to_string().contains("max_file_bytes=8"));
        assert!(error.to_string().contains("huge.py"));
    }

    #[test]
    fn collect_source_files_applies_size_limits_to_symlinked_sources() {
        let root = temporary_dir();
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        let target = workspace.join("target.py");
        let link = workspace.join("linked.py");
        fs::write(&target, "def target():\n    return 'too much'\n").unwrap();

        if !try_symlink_file(&target, &link) {
            let _ = fs::remove_dir_all(root);
            return;
        }

        let error = collect_source_files_with_limits(
            &workspace,
            WorkspaceScanLimits {
                max_files: 10,
                max_file_bytes: Some(8),
                timeout_ms: None,
            },
        )
        .expect_err("symlinked source files must honor max_file_bytes");

        assert!(error.to_string().contains("source file too large"));
        assert!(error.to_string().contains("max_file_bytes=8"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validates_workspace_scan_timeout_bounds() {
        assert!(
            validate_workspace_scan_limits(WorkspaceScanLimits {
                timeout_ms: Some(0),
                ..WorkspaceScanLimits::default()
            })
            .is_err()
        );
        assert!(
            validate_workspace_scan_limits(WorkspaceScanLimits {
                timeout_ms: Some(MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1),
                ..WorkspaceScanLimits::default()
            })
            .is_err()
        );
    }

    #[test]
    fn max_file_bytes_builder_preserves_default_limits() {
        let limits = WorkspaceScanLimits::with_max_file_bytes(128);
        assert_eq!(limits.max_file_bytes, Some(128));
        assert_eq!(limits.max_files, DEFAULT_WORKSPACE_MAX_FILES);
        assert_eq!(limits.timeout_ms, None);
    }

    #[test]
    fn deadline_reports_expired_workspace_scan_budget() {
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = deadline
            .check("test phase")
            .expect_err("expired workspace scan deadline should fail");
        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
        assert!(error.to_string().contains("timeout_ms=1"));
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

    #[cfg(unix)]
    fn try_symlink_dir(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn try_symlink_dir(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }

    #[cfg(unix)]
    fn try_symlink_file(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn try_symlink_file(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
}
