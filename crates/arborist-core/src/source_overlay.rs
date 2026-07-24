use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language;

mod validation;

pub(crate) use validation::normalize_source_overrides_for_workspace;

pub(crate) fn source_override_for_path(
    path: &Path,
    source: &str,
) -> Result<(PathBuf, BTreeMap<String, String>)> {
    let path = language::normalize_absolute_path(path)?;
    let mut overrides = BTreeMap::new();
    overrides.insert(language::normalize_path(&path), source.to_string());
    Ok((path, overrides))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::language::{ensure_path_inside_workspace, normalize_absolute_path};

    #[test]
    fn ensure_path_inside_workspace_accepts_regular_workspace_child() {
        let root = temporary_dir("regular-child");
        let workspace = root.join("workspace");
        let child = workspace.join("pkg").join("mod.py");
        fs::create_dir_all(child.parent().unwrap()).unwrap();
        fs::write(&child, "def helper():\n    return 1\n").unwrap();

        let workspace = normalize_absolute_path(&workspace).unwrap();
        let child = normalize_absolute_path(&child).unwrap();

        ensure_path_inside_workspace(&workspace, &child).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ensure_path_inside_workspace_rejects_symlink_escape() {
        let root = temporary_dir("symlink-escape");
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let outside_file = outside.join("secret.py");
        fs::write(&outside_file, "def secret():\n    return 1\n").unwrap();

        let link = workspace.join("linked");
        if !try_symlink_dir(&outside, &link) {
            let _ = fs::remove_dir_all(root);
            return;
        }
        let escaped = link.join("secret.py");
        let workspace = normalize_absolute_path(&workspace).unwrap();
        let escaped = normalize_absolute_path(&escaped).unwrap();

        let error = ensure_path_inside_workspace(&workspace, &escaped)
            .expect_err("symlinked paths that resolve outside the workspace should be rejected");
        assert!(error.to_string().contains("outside workspace"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ensure_path_inside_workspace_rejects_symlink_parent_for_missing_child() {
        let root = temporary_dir("symlink-missing-child");
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let link = workspace.join("linked");
        if !try_symlink_dir(&outside, &link) {
            let _ = fs::remove_dir_all(root);
            return;
        }
        let escaped = link.join("new_file.py");
        let workspace = normalize_absolute_path(&workspace).unwrap();
        let escaped = normalize_absolute_path(&escaped).unwrap();

        let error = ensure_path_inside_workspace(&workspace, &escaped)
            .expect_err("missing files under symlinked parents should resolve the parent first");
        assert!(error.to_string().contains("outside workspace"));
        fs::remove_dir_all(root).unwrap();
    }

    fn temporary_dir(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("arborist-source-overlay-{label}-{unique}"));
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
}
