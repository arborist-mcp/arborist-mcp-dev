use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, bail};

use crate::language;
use crate::workspace_scan::should_skip_index_path;

pub(crate) fn normalize_source_overrides_for_workspace(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    workspace_description: &str,
) -> Result<BTreeMap<String, String>> {
    let mut normalized_overrides = BTreeMap::new();
    let mut duplicate_keys = BTreeSet::new();

    for (file_path, source) in file_overrides {
        let file_path = language::normalize_absolute_path(Path::new(file_path))?;
        if !language::path_is_inside_workspace(workspace_root, &file_path)? {
            bail!(
                "source overlay file {} is outside {workspace_description} {}",
                file_path.display(),
                workspace_root.display()
            );
        }
        if should_skip_index_path(workspace_root, &file_path) {
            bail!(
                "source overlay file {} is inside an ignored workspace directory",
                file_path.display()
            );
        }
        if let Err(error) = language::detect_language(&file_path) {
            bail!(
                "source overlay file {} is not a supported source file: {error}",
                file_path.display()
            );
        }

        let normalized_path = language::normalize_path(&file_path);
        let duplicate_key = if cfg!(windows) {
            normalized_path.to_ascii_lowercase()
        } else {
            normalized_path.clone()
        };
        if !duplicate_keys.insert(duplicate_key) {
            bail!(
                "source overlay contains duplicate file path {}",
                normalized_path
            );
        }
        normalized_overrides.insert(normalized_path, source.clone());
    }

    Ok(normalized_overrides)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;

    use super::normalize_source_overrides_for_workspace;
    use crate::language::normalize_absolute_path;

    #[test]
    fn rejects_duplicate_normalized_overlay_paths() {
        let workspace = normalize_absolute_path(&env::current_dir().unwrap()).unwrap();
        let first = workspace.join("overlay_duplicate.py");
        let second = workspace.join(".").join("overlay_duplicate.py");
        let overrides = BTreeMap::from([
            (first.to_string_lossy().into_owned(), "a".to_string()),
            (second.to_string_lossy().into_owned(), "b".to_string()),
        ]);

        let error = normalize_source_overrides_for_workspace(&workspace, &overrides, "workspace")
            .expect_err("duplicate normalized overlay paths should be rejected");
        assert!(error.to_string().contains("duplicate file path"));
    }

    #[cfg(windows)]
    #[test]
    fn rejects_duplicate_overlay_paths_that_only_differ_by_case() {
        let workspace = normalize_absolute_path(&env::current_dir().unwrap()).unwrap();
        let first = workspace.join("Overlay_Case.py");
        let second = workspace.join("overlay_case.py");
        let overrides = BTreeMap::from([
            (first.to_string_lossy().into_owned(), "a".to_string()),
            (second.to_string_lossy().into_owned(), "b".to_string()),
        ]);

        let error = normalize_source_overrides_for_workspace(&workspace, &overrides, "workspace")
            .expect_err("case-only duplicate overlay paths should be rejected on Windows");
        assert!(error.to_string().contains("duplicate file path"));
    }
}
