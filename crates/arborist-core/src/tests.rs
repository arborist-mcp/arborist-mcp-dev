use std::fs;
use std::path::Path;

use super::{
    DEFAULT_TREE_QUERY_MAX_BYTES, MAX_PATCH_ANALYSIS_TIMEOUT_MS, MAX_PATCH_PREVIEW_TIMEOUT_MS,
    MAX_PATCH_TIMEOUT_MS, MAX_SEMANTIC_SKELETON_TIMEOUT_MS, Position, TraceDirection,
    VirtualFileSystem, execute_tree_query, execute_tree_query_from_path,
    execute_tree_query_with_limit, export_patch_diagnostics_sarif,
    export_patch_diagnostics_sarif_with_timeout, get_semantic_skeleton,
    get_semantic_skeleton_from_path, get_semantic_skeleton_from_path_with_timeout,
    get_semantic_skeleton_with_timeout, list_symbols, list_symbols_context,
    list_symbols_context_from_index, list_symbols_discovery_context,
    list_symbols_discovery_context_from_index, list_symbols_filtered, list_symbols_from_index,
    list_symbols_from_index_filtered, list_symbols_neighborhood_context,
    list_symbols_neighborhood_context_from_index, patch_ast_node, patch_ast_node_at_position,
    patch_ast_node_at_position_from_path, patch_ast_node_at_position_from_path_with_timeout,
    patch_ast_node_at_position_with_timeout, patch_ast_node_from_path,
    patch_ast_node_from_path_with_timeout, patch_ast_node_with_timeout,
    preview_patch_ast_node_at_position_from_path_with_timeout,
    preview_patch_ast_node_at_position_with_timeout, preview_patch_ast_node_from_path,
    preview_patch_ast_node_from_path_with_timeout, preview_patch_ast_node_with_timeout,
    read_symbol, read_symbol_at_position, read_symbol_at_position_from_index, read_symbol_context,
    read_symbol_context_from_index, read_symbol_discovery_context,
    read_symbol_discovery_context_at_position,
    read_symbol_discovery_context_at_position_from_index,
    read_symbol_discovery_context_at_position_with_source,
    read_symbol_discovery_context_from_index, read_symbol_from_index,
    read_symbol_neighborhood_context, read_symbol_neighborhood_context_from_index,
    rebuild_symbol_index, refresh_symbol_index, refresh_symbol_index_for_file,
    replay_patch_evidence_against_trace, replay_patch_evidence_against_trace_with_timeout,
    search_symbols, search_symbols_context, search_symbols_context_filtered_with_timeout,
    search_symbols_context_from_index, search_symbols_discovery_context,
    search_symbols_discovery_context_filtered_with_timeout,
    search_symbols_discovery_context_from_index, search_symbols_filtered,
    search_symbols_filtered_with_timeout, search_symbols_from_index,
    search_symbols_from_index_filtered, search_symbols_from_index_filtered_with_timeout,
    search_symbols_neighborhood_context, search_symbols_neighborhood_context_filtered_with_timeout,
    search_symbols_neighborhood_context_from_index, trace_symbol_graph,
    trace_symbol_graph_at_position, trace_symbol_graph_at_position_from_index,
    trace_symbol_graph_at_position_with_source, trace_symbol_graph_from_index,
    trace_symbol_neighborhood, trace_symbol_neighborhood_at_position,
    trace_symbol_neighborhood_at_position_from_index, trace_symbol_neighborhood_from_index,
    validate_patch_commit_with_trace, validate_patch_commit_with_trace_with_timeout,
    validate_patch_trace_validation_result, validate_patch_with_discovery_context,
    validate_patch_with_discovery_context_at_position,
    validate_patch_with_discovery_context_from_path, validate_patch_with_graph_context,
    validate_patch_with_graph_context_from_path, validate_patch_with_neighborhood_context,
    validate_patch_with_neighborhood_context_from_path, validate_patch_with_trace_context,
    validate_patch_with_trace_context_at_position, validate_patch_with_trace_context_from_path,
    validate_trace_backed_patch_result, validate_trace_patch_evidence_replay_result,
};
mod c_patching;
mod c_symbol_graph;
mod index_refresh;
mod javascript_patching;
mod language_invariants;
mod patch_bindings;
mod patch_preview_timeout;
mod patch_replay;
mod patch_timeout;
mod path_entrypoints;
mod persisted_index;
mod python_overloads;
mod query_parity;
mod skeleton;
mod source_overlay;
mod support;
mod trace_regressions;
mod trace_semantics;
mod tree_query;
mod workspace_edit_preview;

use support::temporary_dir;

#[test]
fn rejects_empty_file_paths() {
    let source = "def top_level(value: int) -> int:\n    return value\n";

    let error = get_semantic_skeleton(Path::new(""), source, 1, &[])
        .expect_err("empty file paths should be rejected");

    assert!(error.to_string().contains("path"));
    assert!(error.to_string().contains("empty"));
}

#[test]
fn rejects_blank_patch_targets() {
    let source = "def top_level():\n    return 1\n";

    let error = patch_ast_node(
        Path::new("sample.py"),
        source,
        " \t",
        "def top_level():\n    return 2\n",
        None,
    )
    .expect_err("blank patch targets should be rejected");

    assert!(error.to_string().contains("semantic target"));
    assert!(error.to_string().contains("blank"));
}

#[test]
fn rejects_blank_patch_replacements() {
    let source = "def top_level():\n    return 1\n";

    let error = patch_ast_node(Path::new("sample.py"), source, "top_level", " \t", None)
        .expect_err("blank patch replacements should be rejected");

    assert!(error.to_string().contains("new_code"));
    assert!(error.to_string().contains("blank"));
}
