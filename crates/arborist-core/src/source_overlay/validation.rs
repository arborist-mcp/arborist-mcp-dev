use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, bail};

use crate::deadline::DeadlineCheck;
use crate::language;
use crate::workspace_scan::should_skip_index_path;

pub(crate) fn normalize_source_overrides_for_workspace(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    workspace_description: &str,
) -> Result<BTreeMap<String, String>> {
    normalize_source_overrides_for_workspace_with_deadline(
        workspace_root,
        file_overrides,
        workspace_description,
        None,
    )
}

pub(crate) fn normalize_source_overrides_for_workspace_with_deadline(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    workspace_description: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeMap<String, String>> {
    if let Some(deadline) = deadline {
        deadline.check("normalizing source overlays")?;
    }

    let mut normalized_overrides = BTreeMap::new();
    let mut duplicate_keys = BTreeSet::new();

    for (file_path, source) in file_overrides {
        if let Some(deadline) = deadline {
            deadline.check("normalizing source overlays")?;
        }
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
        language::validate_source_size(&file_path, source)?;

        let normalized_path = language::normalize_path(&file_path);
        let duplicate_key = language::path_identity(&normalized_path);
        if !duplicate_keys.insert(duplicate_key) {
            bail!(
                "source overlay contains duplicate file path {}",
                normalized_path
            );
        }
        normalized_overrides.insert(normalized_path, source.clone());
    }

    if let Some(deadline) = deadline {
        deadline.check("normalizing source overlays")?;
    }
    Ok(normalized_overrides)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::time::{Duration, Instant};

    use super::{
        normalize_source_overrides_for_workspace,
        normalize_source_overrides_for_workspace_with_deadline,
    };
    use crate::language::normalize_absolute_path;
    use crate::workspace_scan::WorkspaceScanDeadline;

    #[test]
    fn rejects_expired_deadline_before_normalizing_source_overlays() {
        let workspace = normalize_absolute_path(&env::current_dir().unwrap()).unwrap();
        let file_path = workspace.join("deadline_overlay.py");
        let overrides = BTreeMap::from([(file_path.to_string_lossy().into_owned(), String::new())]);
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = normalize_source_overrides_for_workspace_with_deadline(
            &workspace,
            &overrides,
            "workspace",
            Some(&deadline),
        )
        .expect_err("expired deadline should stop source-overlay normalization");

        assert!(error.to_string().contains("normalizing source overlays"));
    }

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

    #[test]
    fn rejects_oversized_normalized_overlay_source() {
        let workspace = normalize_absolute_path(&env::current_dir().unwrap()).unwrap();
        let file_path = workspace.join("oversized_overlay.py");
        let source = "x".repeat((crate::language::MAX_SOURCE_FILE_BYTES + 1) as usize);
        let overrides = BTreeMap::from([(file_path.to_string_lossy().into_owned(), source)]);

        let error = normalize_source_overrides_for_workspace(&workspace, &overrides, "workspace")
            .expect_err("oversized source overlays should be rejected during normalization");
        assert!(error.to_string().contains("source text too large"));
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

    #[cfg(windows)]
    #[test]
    fn rejects_duplicate_overlay_paths_that_differ_by_unicode_case() {
        let workspace = normalize_absolute_path(&env::current_dir().unwrap()).unwrap();
        let first = workspace.join("Überlay_Case.py");
        let second = workspace.join("überlay_case.py");
        let overrides = BTreeMap::from([
            (first.to_string_lossy().into_owned(), "a".to_string()),
            (second.to_string_lossy().into_owned(), "b".to_string()),
        ]);

        let error = normalize_source_overrides_for_workspace(&workspace, &overrides, "workspace")
            .expect_err("Unicode case-only duplicate overlay paths should be rejected on Windows");
        assert!(error.to_string().contains("duplicate file path"));
    }
}
