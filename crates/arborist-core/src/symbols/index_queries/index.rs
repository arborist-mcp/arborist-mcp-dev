use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::Connection;

use crate::deadline::DeadlineCheck;
use crate::index_schema::{
    load_indexed_files_metadata_with_deadline, require_current_symbol_index_schema_with_deadline,
    require_symbol_index_tables_with_deadline,
    validate_symbol_index_analysis_provenance_with_deadline,
    validate_symbol_index_schema_version_with_deadline,
    validate_symbol_index_workspace_with_deadline,
};
use crate::index_store::{
    SymbolRefreshPersistence, load_file_states_with_deadline,
    load_indexed_symbols_grouped_by_file_with_deadline, load_resolved_symbols_with_deadline,
    persist_symbol_index, persist_symbol_refresh,
};
use crate::language::{
    detect_language, ensure_path_inside_workspace, normalize_absolute_path, normalize_path,
    parse_document_with_timeout, read_source,
};
use crate::model::SymbolIndexStats;
use crate::symbol_dependency::{
    RefreshResolutionInputs, assign_symbol_ids_with_deadline, materialize_resolved_symbol_rows,
    refresh_resolved_symbol_subgraph,
};
use crate::symbol_extractor::index_symbols_from_document_with_deadline;
use crate::symbol_index_state::{
    resolve_persisted_file_path, source_fingerprint,
    validate_persisted_index_paths_with_overrides_and_deadline,
};
use crate::symbol_index_workspace::{
    expanded_refresh_file_paths, resolve_workspace_symbols_incremental_with_deadline,
};
use crate::symbol_map::resolved_symbol_map;
use crate::workspace_scan::{
    WorkspaceScanDeadline, WorkspaceScanLimits, should_skip_index_path, validate_source_file_size,
};

pub fn rebuild_symbol_index(workspace_root: &Path, db_path: &Path) -> Result<SymbolIndexStats> {
    rebuild_symbol_index_with_limits(workspace_root, db_path, WorkspaceScanLimits::default())
}

pub fn refresh_symbol_index(workspace_root: &Path, db_path: &Path) -> Result<SymbolIndexStats> {
    refresh_symbol_index_with_limits(workspace_root, db_path, WorkspaceScanLimits::default())
}

pub fn refresh_symbol_index_with_limits(
    workspace_root: &Path,
    db_path: &Path,
    limits: WorkspaceScanLimits,
) -> Result<SymbolIndexStats> {
    rebuild_symbol_index_with_limits(workspace_root, db_path, limits)
}

pub fn rebuild_symbol_index_with_limits(
    workspace_root: &Path,
    db_path: &Path,
    limits: WorkspaceScanLimits,
) -> Result<SymbolIndexStats> {
    let deadline = WorkspaceScanDeadline::new(limits)?;
    rebuild_symbol_index_with_deadline(workspace_root, db_path, limits, &deadline)
}

fn rebuild_symbol_index_with_deadline(
    workspace_root: &Path,
    db_path: &Path,
    limits: WorkspaceScanLimits,
    deadline: &WorkspaceScanDeadline,
) -> Result<SymbolIndexStats> {
    deadline.check("preparing symbol index")?;
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let db_path = normalize_absolute_path(db_path)?;
    if db_path.exists() {
        let connection = Connection::open(&db_path)?;
        require_symbol_index_tables_with_deadline(
            &connection,
            &db_path,
            Some(deadline as &dyn DeadlineCheck),
        )?;
        validate_symbol_index_workspace_with_deadline(
            &connection,
            &workspace_root,
            &db_path,
            Some(deadline),
        )?;
        load_indexed_files_metadata_with_deadline(&connection, Some(deadline))?;
        validate_symbol_index_schema_version_with_deadline(
            &connection,
            &db_path,
            Some(deadline as &dyn DeadlineCheck),
        )?;
        require_current_symbol_index_schema_with_deadline(
            &connection,
            &db_path,
            Some(deadline as &dyn DeadlineCheck),
        )?;
        validate_symbol_index_analysis_provenance_with_deadline(
            &connection,
            &db_path,
            Some(deadline as &dyn DeadlineCheck),
        )?;
    }
    let (raw_symbols, resolved_symbols, file_states, indexed_files, rebuilt_files, reused_files) =
        resolve_workspace_symbols_incremental_with_deadline(
            &workspace_root,
            &db_path,
            limits,
            deadline,
        )?;
    deadline.check("persisting symbol index")?;
    persist_symbol_index(
        &db_path,
        &workspace_root,
        &raw_symbols,
        &resolved_symbols,
        &file_states,
        indexed_files,
        Some(deadline),
    )?;

    let result = SymbolIndexStats {
        db_path: normalize_path(&db_path),
        indexed_files,
        indexed_symbols: resolved_symbols.len(),
        rebuilt_files,
        reused_files,
    };
    result.validate_public_output()?;
    Ok(result)
}

fn refresh_source(
    refresh_path: &Path,
    normalized_refresh_path: &str,
    refresh_path_overrides: &BTreeMap<String, String>,
) -> Result<String> {
    refresh_path_overrides
        .get(normalized_refresh_path)
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(refresh_path))
}

pub fn refresh_symbol_index_for_file(
    workspace_root: &Path,
    db_path: &Path,
    file_path: &Path,
) -> Result<SymbolIndexStats> {
    refresh_symbol_index_for_file_with_limits(
        workspace_root,
        db_path,
        file_path,
        WorkspaceScanLimits::default(),
    )
}

pub fn refresh_symbol_index_for_file_with_limits(
    workspace_root: &Path,
    db_path: &Path,
    file_path: &Path,
    limits: WorkspaceScanLimits,
) -> Result<SymbolIndexStats> {
    let deadline = WorkspaceScanDeadline::new(limits)?;
    deadline.check("preparing file refresh")?;
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;

    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    detect_language(&file_path)?;

    if !db_path.exists() {
        return rebuild_symbol_index_with_deadline(&workspace_root, &db_path, limits, &deadline);
    }

    let connection = Connection::open(&db_path)?;
    require_symbol_index_tables_with_deadline(
        &connection,
        &db_path,
        Some(&deadline as &dyn DeadlineCheck),
    )?;
    validate_symbol_index_workspace_with_deadline(
        &connection,
        &workspace_root,
        &db_path,
        Some(&deadline),
    )?;
    load_indexed_files_metadata_with_deadline(&connection, Some(&deadline))?;
    validate_symbol_index_schema_version_with_deadline(
        &connection,
        &db_path,
        Some(&deadline as &dyn DeadlineCheck),
    )?;
    require_current_symbol_index_schema_with_deadline(
        &connection,
        &db_path,
        Some(&deadline as &dyn DeadlineCheck),
    )?;
    validate_symbol_index_analysis_provenance_with_deadline(
        &connection,
        &db_path,
        Some(&deadline as &dyn DeadlineCheck),
    )?;

    let old_resolved_symbols = load_resolved_symbols_with_deadline(&connection, Some(&deadline))?.0;
    deadline.check("loading existing resolved symbols")?;
    let old_resolved_map = resolved_symbol_map(&old_resolved_symbols);
    let mut grouped_symbols =
        load_indexed_symbols_grouped_by_file_with_deadline(&connection, &deadline)?;
    deadline.check("loading existing indexed symbols")?;
    let mut file_states = load_file_states_with_deadline(&connection, Some(&deadline))?;
    deadline.check("loading existing indexed file states")?;
    let refresh_file_path = resolve_persisted_file_path(&file_path, &file_states)?;
    let refresh_paths = if should_skip_index_path(&workspace_root, &refresh_file_path) {
        vec![refresh_file_path]
    } else {
        expanded_refresh_file_paths(&workspace_root, &refresh_file_path, limits, &deadline)?
    };
    let mut refresh_path_overrides = BTreeMap::new();
    for refresh_path in &refresh_paths {
        deadline.check("reading changed refresh sources")?;
        if !refresh_path.exists() || should_skip_index_path(&workspace_root, refresh_path) {
            continue;
        }
        validate_source_file_size(refresh_path, limits)?;
        let source = read_source(refresh_path)?;
        let normalized_refresh_path = normalize_path(refresh_path);
        if file_states
            .get(&normalized_refresh_path)
            .is_some_and(|stored_fingerprint| *stored_fingerprint != source_fingerprint(&source))
        {
            refresh_path_overrides.insert(normalized_refresh_path, source);
        }
    }
    validate_persisted_index_paths_with_overrides_and_deadline(
        &workspace_root,
        &file_states,
        &old_resolved_symbols,
        Some(&refresh_path_overrides),
        Some(&deadline),
    )?;
    let old_symbol_ids = grouped_symbols
        .values()
        .flatten()
        .map(|symbol| {
            (
                (
                    symbol.file_path.clone(),
                    symbol.semantic_path.clone(),
                    symbol.node_kind.clone(),
                    symbol.byte_range,
                ),
                symbol.symbol_id.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut old_changed_symbols = Vec::new();
    let mut changed_file_paths = BTreeSet::new();
    let mut rebuilt_files = 0;

    for refresh_path in &refresh_paths {
        deadline.check("refreshing indexed files")?;
        let normalized_refresh_path = normalize_path(refresh_path);
        let skip_refresh_path = should_skip_index_path(&workspace_root, refresh_path);
        let had_indexed_state = file_states.contains_key(&normalized_refresh_path)
            || grouped_symbols.contains_key(&normalized_refresh_path);
        old_changed_symbols.extend(
            grouped_symbols
                .get(&normalized_refresh_path)
                .cloned()
                .unwrap_or_default(),
        );

        if refresh_path.exists() && !skip_refresh_path {
            validate_source_file_size(refresh_path, limits)?;
            let source = refresh_source(
                refresh_path,
                &normalized_refresh_path,
                &refresh_path_overrides,
            )?;
            let document = parse_document_with_timeout(
                refresh_path,
                &source,
                deadline.remaining_timeout_micros("parsing refreshed files")?,
            )?;
            deadline.check("extracting refreshed symbols")?;
            let fresh_symbols = index_symbols_from_document_with_deadline(
                refresh_path,
                &source,
                &document,
                Some(&deadline),
            )?;

            file_states.insert(normalized_refresh_path.clone(), source_fingerprint(&source));
            grouped_symbols.insert(normalized_refresh_path.clone(), fresh_symbols);
            rebuilt_files += 1;
        } else {
            file_states.remove(&normalized_refresh_path);
            grouped_symbols.remove(&normalized_refresh_path);
            if had_indexed_state {
                rebuilt_files += 1;
            }
        }
        changed_file_paths.insert(normalized_refresh_path);
    }

    let mut raw_symbols = grouped_symbols
        .values()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    deadline.check("assigning refreshed symbol identities")?;
    assign_symbol_ids_with_deadline(&mut raw_symbols, &deadline)?;
    deadline.check("assigning refreshed symbol identities")?;
    let identity_changed_paths = raw_symbols
        .iter()
        .filter_map(|symbol| {
            let key = (
                symbol.file_path.clone(),
                symbol.semantic_path.clone(),
                symbol.node_kind.clone(),
                symbol.byte_range,
            );
            old_symbol_ids
                .get(&key)
                .is_some_and(|old_id| old_id != &symbol.symbol_id)
                .then(|| symbol.file_path.clone())
        })
        .collect::<BTreeSet<_>>();
    for identity_changed_path in identity_changed_paths {
        if changed_file_paths.insert(identity_changed_path.clone()) {
            old_changed_symbols.extend(
                grouped_symbols
                    .get(&identity_changed_path)
                    .cloned()
                    .unwrap_or_default(),
            );
        }
    }
    let new_changed_symbols = raw_symbols
        .iter()
        .filter(|symbol| changed_file_paths.contains(&symbol.file_path))
        .cloned()
        .collect::<Vec<_>>();
    deadline.check("resolving refreshed symbols")?;
    let source_file_paths = file_states.keys().map(PathBuf::from).collect::<Vec<_>>();
    let (resolved_map, impacted_paths) = refresh_resolved_symbol_subgraph(
        &raw_symbols,
        &old_resolved_map,
        &old_changed_symbols,
        &new_changed_symbols,
        &changed_file_paths,
        RefreshResolutionInputs {
            source_file_paths: &source_file_paths,
            file_overrides: None,
            deadline: Some(&deadline),
        },
    )?;
    deadline.check("resolving refreshed symbols")?;
    let resolved_symbols = materialize_resolved_symbol_rows(&raw_symbols, &resolved_map);
    deadline.check("materializing refreshed symbols")?;
    let indexed_files = file_states.len();
    let reused_files = indexed_files.saturating_sub(rebuilt_files);

    deadline.check("persisting symbol index")?;
    persist_symbol_refresh(SymbolRefreshPersistence {
        db_path: &db_path,
        workspace_root: &workspace_root,
        raw_symbols: &raw_symbols,
        symbols: &resolved_symbols,
        resolved_symbols_by_id: &resolved_map,
        file_states: &file_states,
        changed_file_paths: &changed_file_paths,
        impacted_paths: &impacted_paths,
        indexed_files,
        deadline: Some(&deadline),
    })?;

    let result = SymbolIndexStats {
        db_path: normalize_path(&db_path),
        indexed_files,
        indexed_symbols: resolved_symbols.len(),
        rebuilt_files,
        reused_files,
    };
    result.validate_public_output()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::refresh_source;

    #[test]
    fn refresh_source_uses_captured_override_without_rereading_path() {
        let overrides = BTreeMap::from([("missing.py".to_string(), "cached source".to_string())]);

        let source = refresh_source(Path::new("missing.py"), "missing.py", &overrides)
            .expect("captured source should not require the path to remain readable");

        assert_eq!(source, "cached source");
    }
}
