use std::collections::BTreeMap;
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

        normalized_overrides.insert(language::normalize_path(&file_path), source.clone());
    }

    Ok(normalized_overrides)
}
