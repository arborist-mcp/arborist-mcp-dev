use std::path::Path;

use anyhow::Result;

use crate::deadline::DeadlineCheck;

use crate::language::{
    ensure_path_inside_workspace, normalize_absolute_path, parse_document_with_timeout,
    validate_source_size,
};
use crate::model::{
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, TraceDirection,
};
use crate::symbol_dependency::{assign_symbol_ids, symbol_meta_from_indexed};
use crate::symbol_extractor::index_symbols_from_document;
use crate::symbol_index_workspace::load_live_workspace_symbols;
use crate::symbol_query_execution::list_from_symbols_with_deadline;
use crate::symbol_query_execution::{
    list_context_from_symbols, list_discovery_context_from_symbols, list_from_symbols,
    list_neighborhood_context_from_symbols,
};
use crate::symbol_trace::TraceQueryDeadline;
use crate::workspace_scan::should_skip_index_path;

pub fn list_symbols(workspace_root: &Path, limit: usize) -> Result<SymbolListResult> {
    list_symbols_filtered(workspace_root, limit, None, None)
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_in_file_with_source_filtered_with_timeout(
    workspace_root: &Path,
    file_path: &Path,
    source: &str,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<SymbolListResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;

    if should_skip_index_path(&workspace_root, &file_path) {
        return list_from_symbols_with_deadline(
            &[],
            0,
            limit,
            file_path_contains,
            node_kind,
            &deadline,
        );
    }

    validate_source_size(&file_path, source)?;
    deadline.check("single-file symbol parsing")?;
    let document = parse_document_with_timeout(
        &file_path,
        source,
        deadline
            .remaining_timeout_micros("single-file symbol parsing")?
            .unwrap_or(0),
    )?;
    deadline.check("single-file symbol extraction")?;
    let mut raw_symbols = index_symbols_from_document(&file_path, source, &document)?;
    assign_symbol_ids(&mut raw_symbols)?;
    deadline.check("single-file symbol listing")?;
    let symbols = raw_symbols
        .iter()
        .map(symbol_meta_from_indexed)
        .collect::<Vec<_>>();

    list_from_symbols_with_deadline(&symbols, 1, limit, file_path_contains, node_kind, &deadline)
}

pub fn list_symbols_context(
    workspace_root: &Path,
    limit: usize,
) -> Result<SymbolListContextResult> {
    list_symbols_context_filtered(workspace_root, limit, None, None)
}

pub fn list_symbols_discovery_context(
    workspace_root: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolListDiscoveryContextResult> {
    list_symbols_discovery_context_filtered(
        workspace_root,
        limit,
        direction,
        max_depth,
        max_nodes,
        None,
        None,
    )
}

pub fn list_symbols_neighborhood_context(
    workspace_root: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<SymbolListNeighborhoodContextResult> {
    list_symbols_neighborhood_context_filtered(
        workspace_root,
        limit,
        direction,
        max_depth,
        max_nodes,
        None,
        None,
    )
}

pub fn list_symbols_filtered(
    workspace_root: &Path,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    list_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
    )
}

pub fn list_symbols_context_filtered(
    workspace_root: &Path,
    limit: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    list_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        file_path_contains,
        node_kind,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_discovery_context_filtered(
    workspace_root: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListDiscoveryContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    list_discovery_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn list_symbols_neighborhood_context_filtered(
    workspace_root: &Path,
    limit: usize,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> Result<SymbolListNeighborhoodContextResult> {
    let (resolved_symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
    list_neighborhood_context_from_symbols(
        &resolved_symbols,
        indexed_files,
        limit,
        direction,
        max_depth,
        max_nodes,
        file_path_contains,
        node_kind,
        None,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::list_symbols_in_file_with_source_filtered_with_timeout;

    #[test]
    fn single_file_listing_does_not_scan_other_workspace_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("arborist-single-file-list-{unique}"));
        let target = workspace.join("target.py");
        let unrelated = workspace.join("unrelated.py");
        fs::create_dir_all(&workspace).expect("temporary workspace should be created");
        fs::write(&target, "def target():\n    return 1\n")
            .expect("target source should be written");
        fs::write(&unrelated, "def broken(:\n").expect("unrelated source should be written");

        let result = list_symbols_in_file_with_source_filtered_with_timeout(
            &workspace,
            &target,
            "def target():\n    return 2\n",
            10,
            None,
            None,
            None,
        )
        .expect("single-file listing should not parse unrelated files");

        assert_eq!(result.indexed_files, 1);
        assert_eq!(result.total_symbols, 1);
        assert_eq!(result.symbols[0].semantic_path, "target");
        assert_eq!(
            result.symbols[0].file_path,
            target.to_string_lossy().replace('\\', "/")
        );

        fs::remove_dir_all(&workspace).expect("temporary workspace should be removed");
    }

    #[test]
    fn oversized_inline_source_rejected_before_parse_work() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let workspace =
            std::env::temp_dir().join(format!("arborist-single-file-list-oversized-{unique}"));
        let target = workspace.join("target.py");
        fs::create_dir_all(&workspace).expect("temporary workspace should be created");
        fs::write(&target, "def target():\n    return 1\n").expect("target should be written");

        let oversized = "x".repeat(crate::language::MAX_SOURCE_FILE_BYTES as usize + 1);
        let error = list_symbols_in_file_with_source_filtered_with_timeout(
            &workspace, &target, &oversized, 10, None, None, None,
        )
        .expect_err("oversized inline sources should be rejected before parsing");

        assert!(error.to_string().contains("source text too large"));

        fs::remove_dir_all(&workspace).expect("temporary workspace should be removed");
    }
}
