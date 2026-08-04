use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};

use crate::index_schema::{
    load_indexed_files_metadata_with_deadline, load_symbol_index_workspace_root_with_deadline,
    open_symbol_index_read_only, require_current_symbol_index_schema, require_symbol_index_tables,
    validate_symbol_index_analysis_provenance, validate_symbol_index_schema_version,
};
use crate::index_store::{
    load_file_states, load_file_states_with_deadline, load_indexed_symbols_grouped_by_file,
    load_indexed_symbols_grouped_by_file_with_deadline, load_resolved_symbols,
    load_resolved_symbols_with_deadline,
};
use crate::language::{normalize_path, parse_document, parse_document_with_timeout};
use crate::model::SymbolMeta;
use crate::source_overlay::normalize_source_overrides_for_workspace;
use crate::symbol_dependency::{
    RefreshResolutionInputs, assign_symbol_ids, assign_symbol_ids_with_deadline,
    materialize_resolved_symbol_rows, refresh_resolved_symbol_subgraph,
};
use crate::symbol_extractor::{
    index_symbols_from_document, index_symbols_from_document_with_deadline,
};
use crate::symbol_index_workspace::expanded_refresh_file_paths;
use crate::symbol_map::resolved_symbol_map;
use crate::workspace_scan::{MAX_WORKSPACE_SCAN_FILES, WorkspaceScanDeadline, WorkspaceScanLimits};

use super::freshness::{ensure_symbol_index_fresh_with_deadline, validate_indexed_file_count};
use super::paths::validate_persisted_index_paths_with_overrides_and_deadline;

pub(crate) fn load_symbol_index(db_path: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    load_symbol_index_internal(db_path, None)
}

pub(crate) fn load_symbol_index_with_timeout(
    db_path: &Path,
    timeout_ms: Option<u64>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let limits = WorkspaceScanLimits {
        timeout_ms,
        ..WorkspaceScanLimits::default()
    };
    let deadline = WorkspaceScanDeadline::new(limits)?;
    load_symbol_index_internal(db_path, Some(&deadline))
}

fn load_symbol_index_internal(
    db_path: &Path,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    if let Some(deadline) = deadline {
        deadline.check("loading indexed symbols")?;
    }
    if !db_path.exists() {
        return Err(anyhow!("symbol index {} does not exist", db_path.display()));
    }

    let connection = open_symbol_index_read_only(db_path)?;
    require_symbol_index_tables(&connection, db_path)?;
    let indexed_files = load_indexed_files_metadata_with_deadline(&connection, deadline)?;
    validate_symbol_index_schema_version(&connection, db_path)?;
    require_current_symbol_index_schema(&connection, db_path)?;
    validate_symbol_index_analysis_provenance(&connection, db_path)?;
    match deadline {
        Some(deadline) => {
            load_indexed_symbols_grouped_by_file_with_deadline(&connection, deadline)?
        }
        None => load_indexed_symbols_grouped_by_file(&connection)?,
    };
    if let Some(deadline) = deadline {
        deadline.check("loading indexed symbols")?;
    }
    let file_states = match deadline {
        Some(deadline) => load_file_states_with_deadline(&connection, Some(deadline))?,
        None => load_file_states(&connection)?,
    };
    if let Some(deadline) = deadline {
        deadline.check("loading indexed file states")?;
    }
    let resolved_symbols = match deadline {
        Some(deadline) => load_resolved_symbols_with_deadline(&connection, Some(deadline))?,
        None => load_resolved_symbols(&connection)?,
    };
    if let Some(deadline) = deadline {
        deadline.check("loading resolved symbols")?;
    }
    validate_indexed_file_count(indexed_files, file_states.len())?;
    let workspace_root =
        load_symbol_index_workspace_root_with_deadline(&connection, db_path, deadline)?;
    validate_persisted_index_paths_with_overrides_and_deadline(
        &workspace_root,
        &file_states,
        &resolved_symbols.0,
        None,
        deadline,
    )?;
    ensure_symbol_index_fresh_with_deadline(
        db_path,
        &workspace_root,
        &file_states,
        None,
        deadline,
    )?;
    if let Some(deadline) = deadline {
        deadline.check("validating indexed symbol freshness")?;
    }
    Ok(resolved_symbols)
}

pub(crate) fn load_symbol_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    load_symbol_index_with_overrides_internal(db_path, file_overrides, None)
}

pub(crate) fn load_symbol_index_with_overrides_with_timeout(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    timeout_ms: Option<u64>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let limits = WorkspaceScanLimits {
        timeout_ms,
        ..WorkspaceScanLimits::default()
    };
    let deadline = WorkspaceScanDeadline::new(limits)?;
    load_symbol_index_with_overrides_internal(db_path, file_overrides, Some(&deadline))
}

fn load_symbol_index_with_overrides_internal(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    if let Some(deadline) = deadline {
        deadline.check("loading indexed symbol overrides")?;
    }
    if !db_path.exists() {
        return Err(anyhow!("symbol index {} does not exist", db_path.display()));
    }

    let connection = open_symbol_index_read_only(db_path)?;
    require_symbol_index_tables(&connection, db_path)?;
    validate_symbol_index_schema_version(&connection, db_path)?;
    require_current_symbol_index_schema(&connection, db_path)?;
    validate_symbol_index_analysis_provenance(&connection, db_path)?;
    let workspace_root =
        load_symbol_index_workspace_root_with_deadline(&connection, db_path, deadline)?;
    let file_overrides = normalize_source_overrides_for_workspace(
        &workspace_root,
        file_overrides,
        "indexed workspace",
    )?;

    let mut grouped_symbols = match deadline {
        Some(deadline) => {
            load_indexed_symbols_grouped_by_file_with_deadline(&connection, deadline)?
        }
        None => load_indexed_symbols_grouped_by_file(&connection)?,
    };
    if let Some(deadline) = deadline {
        deadline.check("loading indexed symbols")?;
    }
    let persisted_file_states = match deadline {
        Some(deadline) => load_file_states_with_deadline(&connection, Some(deadline))?,
        None => load_file_states(&connection)?,
    };
    if let Some(deadline) = deadline {
        deadline.check("loading indexed file states")?;
    }
    let (resolved_symbols, persisted_indexed_files) = match deadline {
        Some(deadline) => load_resolved_symbols_with_deadline(&connection, Some(deadline))?,
        None => load_resolved_symbols(&connection)?,
    };
    if let Some(deadline) = deadline {
        deadline.check("loading resolved symbols")?;
    }
    validate_indexed_file_count(persisted_indexed_files, persisted_file_states.len())?;
    validate_persisted_index_paths_with_overrides_and_deadline(
        &workspace_root,
        &persisted_file_states,
        &resolved_symbols,
        Some(&file_overrides),
        deadline,
    )?;
    ensure_symbol_index_fresh_with_deadline(
        db_path,
        &workspace_root,
        &persisted_file_states,
        Some(&file_overrides),
        deadline,
    )?;
    let unbounded_deadline = WorkspaceScanDeadline::new(WorkspaceScanLimits::default())?;
    let refresh_deadline = deadline.unwrap_or(&unbounded_deadline);
    let mut changed_file_paths = BTreeSet::new();
    for override_path in file_overrides.keys() {
        refresh_deadline.check("expanding indexed override dependents")?;
        for refresh_path in expanded_refresh_file_paths(
            &workspace_root,
            Path::new(override_path),
            WorkspaceScanLimits::default(),
            refresh_deadline,
        )? {
            changed_file_paths.insert(normalize_path(&refresh_path));
        }
    }

    let mut added_file_paths = BTreeSet::new();
    let old_changed_symbols = changed_file_paths
        .iter()
        .flat_map(|path| grouped_symbols.get(path).into_iter().flatten().cloned())
        .collect::<Vec<_>>();

    for (override_path, override_source) in &file_overrides {
        if let Some(deadline) = deadline {
            deadline.check("parsing indexed symbol overrides")?;
        }
        let override_path = Path::new(override_path);

        let document = match deadline {
            Some(deadline) => parse_document_with_timeout(
                override_path,
                override_source,
                deadline.remaining_timeout_micros("parsing indexed symbol overrides")?,
            )?,
            None => parse_document(override_path, override_source)?,
        };
        let symbols = match deadline {
            Some(deadline) => index_symbols_from_document_with_deadline(
                override_path,
                override_source,
                &document,
                Some(deadline),
            )?,
            None => index_symbols_from_document(override_path, override_source, &document)?,
        };
        let normalized_path = normalize_path(override_path);
        if !persisted_file_states.contains_key(&normalized_path) {
            added_file_paths.insert(normalized_path.clone());
        }
        grouped_symbols.insert(normalized_path.clone(), symbols);
        changed_file_paths.insert(normalized_path);
    }

    let mut raw_symbols = grouped_symbols
        .into_values()
        .flat_map(|symbols| symbols.into_iter())
        .collect::<Vec<_>>();
    if let Some(deadline) = deadline {
        deadline.check("assigning indexed override symbols")?;
    }
    if let Some(deadline) = deadline {
        assign_symbol_ids_with_deadline(&mut raw_symbols, deadline)?;
    } else {
        assign_symbol_ids(&mut raw_symbols)?;
    }
    if let Some(deadline) = deadline {
        deadline.check("assigning indexed override symbols")?;
    }

    let old_resolved_map = resolved_symbol_map(&resolved_symbols);
    let new_changed_symbols = raw_symbols
        .iter()
        .filter(|symbol| changed_file_paths.contains(&symbol.file_path))
        .cloned()
        .collect::<Vec<_>>();
    let source_file_paths = persisted_file_states
        .keys()
        .chain(file_overrides.keys())
        .map(PathBuf::from)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let (resolved_map, _) = refresh_resolved_symbol_subgraph(
        &raw_symbols,
        &old_resolved_map,
        &old_changed_symbols,
        &new_changed_symbols,
        &changed_file_paths,
        RefreshResolutionInputs {
            source_file_paths: &source_file_paths,
            file_overrides: Some(&file_overrides),
            deadline,
        },
    )?;
    if let Some(deadline) = deadline {
        deadline.check("resolving indexed override symbols")?;
    }
    let indexed_files = persisted_indexed_files.saturating_add(added_file_paths.len());
    validate_indexed_overlay_file_count(persisted_indexed_files, added_file_paths.len())?;

    let materialized = materialize_resolved_symbol_rows(&raw_symbols, &resolved_map);
    if let Some(deadline) = deadline {
        deadline.check("materializing indexed override symbols")?;
    }
    Ok((materialized, indexed_files))
}

fn validate_indexed_overlay_file_count(persisted_files: usize, added_files: usize) -> Result<()> {
    let indexed_files = persisted_files.saturating_add(added_files);
    if indexed_files > MAX_WORKSPACE_SCAN_FILES && added_files > 0 {
        bail!(
            "workspace scan exceeded max_files while adding indexed source overlays: max_files={MAX_WORKSPACE_SCAN_FILES}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{load_symbol_index_internal, validate_indexed_overlay_file_count};
    use crate::workspace_scan::{MAX_WORKSPACE_SCAN_FILES, WorkspaceScanDeadline};

    #[test]
    fn indexed_overlays_respect_workspace_file_limit() {
        validate_indexed_overlay_file_count(MAX_WORKSPACE_SCAN_FILES - 1, 1)
            .expect("an overlay that reaches the limit should be accepted");
        let error = validate_indexed_overlay_file_count(MAX_WORKSPACE_SCAN_FILES, 1)
            .expect_err("an added overlay beyond the limit should be rejected");
        assert!(error.to_string().contains("max_files"));
    }

    #[test]
    fn indexed_overlay_count_saturates_corrupt_metadata() {
        validate_indexed_overlay_file_count(usize::MAX, 1)
            .expect_err("corrupt oversized metadata must fail closed");
        assert_eq!(usize::MAX.saturating_add(1), usize::MAX);
    }

    #[test]
    fn timed_index_loading_rejects_expired_deadline_before_opening_database() {
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = load_symbol_index_internal(
            std::path::Path::new("missing-symbol-index.db"),
            Some(&deadline),
        )
        .expect_err("expired index loading should fail before database access");
        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }
}
