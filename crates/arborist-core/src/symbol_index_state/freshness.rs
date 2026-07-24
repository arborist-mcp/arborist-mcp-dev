use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, bail};

use super::paths::{symbol_index_freshness_issues, unindexed_workspace_files};

pub(crate) fn ensure_symbol_index_fresh(
    db_path: &Path,
    workspace_root: &Path,
    file_states: &BTreeMap<String, u64>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<()> {
    let mut issues = symbol_index_freshness_issues(file_states, file_overrides);
    issues.extend(
        unindexed_workspace_files(workspace_root, file_states, file_overrides, None)?
            .into_iter()
            .map(|file_path| format!("workspace source file is not indexed: {file_path}")),
    );
    if issues.is_empty() {
        return Ok(());
    }

    bail!(
        "symbol index {} is stale; refresh_symbol_index_for_file or rebuild_symbol_index before querying: {}",
        db_path.display(),
        issues.join("; ")
    );
}

pub(crate) fn validate_indexed_file_count(
    indexed_files: usize,
    file_state_entries: usize,
) -> Result<()> {
    if indexed_files != file_state_entries {
        bail!(
            "indexed_files metadata {indexed_files} does not match file_state entries {file_state_entries}"
        );
    }
    Ok(())
}
