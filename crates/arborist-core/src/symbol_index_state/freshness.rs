use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, bail};

use crate::language::read_source;
use crate::model::SymbolIndexHealth;
use crate::workspace_scan::WorkspaceScanDeadline;

use super::fingerprints::source_fingerprint;
use super::paths::{symbol_index_freshness_issues, unindexed_workspace_files};

pub(crate) fn inspect_symbol_index_freshness(
    health: &mut SymbolIndexHealth,
    file_states: &BTreeMap<String, u64>,
    deadline: &WorkspaceScanDeadline,
) -> Result<()> {
    let mut fresh_files = 0;
    for (file_path, stored_fingerprint) in file_states {
        deadline.check("inspecting indexed file freshness")?;
        let path = Path::new(file_path);
        if !path.exists() {
            health.missing_files.push(file_path.clone());
            health
                .issues
                .push(format!("indexed file is missing: {file_path}"));
            continue;
        }

        match read_source(path) {
            Ok(source) => {
                let current_fingerprint = source_fingerprint(&source);
                if current_fingerprint == *stored_fingerprint {
                    fresh_files += 1;
                } else {
                    health.stale_files.push(file_path.clone());
                    health
                        .issues
                        .push(format!("indexed file is stale: {file_path}"));
                }
            }
            Err(error) => {
                health.unreadable_files.push(file_path.clone());
                health
                    .issues
                    .push(format!("failed to read indexed file {file_path}: {error}"));
            }
        }
    }
    health.fresh_file_count = Some(fresh_files);
    Ok(())
}

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
