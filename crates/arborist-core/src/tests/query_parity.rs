use std::fs;
use std::path::Path;

use super::support::temporary_dir;
use super::{
    Position, TraceDirection, VirtualFileSystem, list_symbols, list_symbols_context,
    list_symbols_context_from_index, list_symbols_discovery_context,
    list_symbols_discovery_context_from_index, list_symbols_filtered, list_symbols_from_index,
    list_symbols_from_index_filtered, list_symbols_neighborhood_context,
    list_symbols_neighborhood_context_from_index, patch_ast_node_at_position, read_symbol,
    read_symbol_at_position, read_symbol_at_position_from_index, read_symbol_context,
    read_symbol_context_from_index, read_symbol_discovery_context,
    read_symbol_discovery_context_at_position,
    read_symbol_discovery_context_at_position_from_index,
    read_symbol_discovery_context_at_position_with_source,
    read_symbol_discovery_context_from_index, read_symbol_from_index,
    read_symbol_neighborhood_context, read_symbol_neighborhood_context_from_index,
    rebuild_symbol_index, refresh_symbol_index, search_symbols, search_symbols_context,
    search_symbols_context_from_index, search_symbols_discovery_context,
    search_symbols_discovery_context_from_index, search_symbols_filtered,
    search_symbols_from_index, search_symbols_from_index_filtered,
    search_symbols_neighborhood_context, search_symbols_neighborhood_context_from_index,
    trace_symbol_graph_at_position, trace_symbol_graph_at_position_from_index,
    trace_symbol_graph_at_position_with_source, trace_symbol_graph_from_index,
    trace_symbol_neighborhood, trace_symbol_neighborhood_at_position,
    trace_symbol_neighborhood_at_position_from_index, trace_symbol_neighborhood_from_index,
    validate_patch_with_discovery_context_at_position,
    validate_patch_with_trace_context_at_position,
};
use crate::language::normalize_path;
#[test]
fn rebuilds_and_reads_persisted_symbol_index() {
    let workspace_root = Path::new("../../tests/fixtures");
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");

    let stats = rebuild_symbol_index(workspace_root, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 4);
    assert!(stats.indexed_symbols >= 3);
    assert_eq!(stats.reused_files, 0);

    let repeat_stats = rebuild_symbol_index(workspace_root, &db_path).unwrap();
    assert_eq!(repeat_stats.indexed_files, 4);
    assert_eq!(repeat_stats.rebuilt_files, 0);
    assert_eq!(repeat_stats.reused_files, 4);

    let trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.parameters, vec!["value: int".to_string()]);
    assert_eq!(trace.symbol.return_type.as_deref(), Some("int"));
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.parameters == vec!["value: int".to_string()])
    );
}

#[test]
fn refreshes_changed_added_and_deleted_workspace_files_incrementally() {
    let dir = temporary_dir();
    let changed = dir.join("changed.py");
    let deleted = dir.join("deleted.py");
    let added = dir.join("added.py");
    let db_path = dir.join("symbols.db");
    fs::write(&changed, "def before() -> int:\n    return 1\n").unwrap();
    fs::write(&deleted, "def removed() -> int:\n    return 2\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    fs::write(&changed, "def after() -> int:\n    return 3\n").unwrap();
    fs::remove_file(&deleted).unwrap();
    fs::write(&added, "def created() -> int:\n    return 4\n").unwrap();

    let stats = refresh_symbol_index(&dir, &db_path).unwrap();

    assert_eq!(stats.indexed_files, 2);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 0);
    assert_eq!(
        search_symbols_from_index(&db_path, "after", 10)
            .unwrap()
            .matches
            .len(),
        1
    );
    assert_eq!(
        search_symbols_from_index(&db_path, "created", 10)
            .unwrap()
            .matches
            .len(),
        1
    );
    assert!(
        search_symbols_from_index(&db_path, "before", 10)
            .unwrap()
            .matches
            .is_empty()
    );
    assert!(
        search_symbols_from_index(&db_path, "removed", 10)
            .unwrap()
            .matches
            .is_empty()
    );
}

#[test]
fn rebuild_symbol_index_normalizes_workspace_and_db_paths() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let nested = workspace.join("child");
    let helper = workspace.join("helper.py");
    let caller = workspace.join("caller.py");

    fs::create_dir_all(&nested).unwrap();
    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let workspace_with_segments = nested.join("..");
    let db_path_with_segments = nested.join("..").join("symbols.db");
    let stats = rebuild_symbol_index(&workspace_with_segments, &db_path_with_segments).unwrap();

    assert_eq!(stats.indexed_files, 2);
    assert!(!stats.db_path.contains("/../"));

    let trace =
        trace_symbol_graph_from_index(&db_path_with_segments, "orchestrate", TraceDirection::Both)
            .unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "helper");
    assert!(!trace.symbol.file_path.contains("/../"));
}

#[test]
fn searches_symbols_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &helper,
            "def helper(value: int) -> int:\n    \"\"\"Increment a value.\"\"\"\n    return value + 1\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let live = search_symbols(&dir, "helper", 10).unwrap();
    assert_eq!(live.query, "helper");
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.total_matches, 1);
    assert!(!live.truncated);
    assert_eq!(live.matches.len(), 1);
    assert_eq!(live.matches[0].semantic_path, "helper");
    assert_eq!(live.match_details.len(), 1);
    assert_eq!(live.match_details[0].symbol_id, "helper");
    assert_eq!(live.match_details[0].score, 1000);
    assert!(
        live.match_details[0]
            .matched_fields
            .contains(&"base_name".to_string())
    );
    assert_eq!(live.matches[0].parameters, vec!["value: int".to_string()]);
    assert_eq!(live.matches[0].return_type.as_deref(), Some("int"));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_from_index(&db_path, "helper", 10).unwrap();
    assert_eq!(persisted.query, "helper");
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.total_matches, 1);
    assert!(!persisted.truncated);
    assert_eq!(persisted.matches.len(), 1);
    assert_eq!(persisted.matches[0].semantic_path, "helper");
    assert_eq!(persisted.match_details[0].symbol_id, "helper");
    assert_eq!(
        persisted.matches[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn search_symbols_prefers_exact_matches_and_honors_limit() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let helper_tools = dir.join("helper_tools.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(
        &helper_tools,
        "def helper_tool() -> int:\n    return 2\n\ndef helper_secondary() -> int:\n    return 3\n",
    )
    .unwrap();

    let live = search_symbols(&dir, "helper", 2).unwrap();
    assert_eq!(live.total_matches, 3);
    assert!(live.truncated);
    assert_eq!(live.matches.len(), 2);
    assert_eq!(live.matches[0].semantic_path, "helper");
    assert_eq!(live.match_details[0].score, 1000);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_from_index(&db_path, "helper", 1).unwrap();
    assert_eq!(persisted.total_matches, 3);
    assert!(persisted.truncated);
    assert_eq!(persisted.matches.len(), 1);
    assert_eq!(persisted.matches[0].semantic_path, "helper");
}

#[test]
fn search_symbols_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &helper,
        Some("def renamed_helper() -> int:\n    return 1\n"),
    )
    .unwrap();

    let results = vfs.search_symbols(&dir, "renamed_helper", 10).unwrap();
    assert_eq!(results.total_matches, 1);
    assert!(!results.truncated);
    assert_eq!(results.matches.len(), 1);
    assert_eq!(results.matches[0].semantic_path, "renamed_helper");
    assert_eq!(results.match_details[0].symbol_id, "renamed_helper");

    let old_name = vfs.search_symbols(&dir, "helper", 10).unwrap();
    assert_eq!(old_name.matches[0].semantic_path, "renamed_helper");
    assert!(
        !old_name
            .matches
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn search_symbols_filters_by_file_path_and_node_kind() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let helper_class = dir.join("helper_types.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(
        &helper_class,
        "class Helper:\n    pass\n\n\ndef helper_factory() -> Helper:\n    return Helper()\n",
    )
    .unwrap();

    let live = search_symbols_filtered(&dir, "helper", 10, Some("types"), Some("class_definition"))
        .unwrap();
    assert_eq!(live.total_matches, 1);
    assert_eq!(live.matches.len(), 1);
    assert_eq!(live.matches[0].semantic_path, "Helper");
    assert_eq!(live.matches[0].node_kind, "class_definition");
    assert!(live.matches[0].file_path.ends_with("helper_types.py"));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_from_index_filtered(
        &db_path,
        "helper",
        10,
        Some("types"),
        Some("class_definition"),
    )
    .unwrap();
    assert_eq!(persisted.total_matches, 1);
    assert_eq!(persisted.matches.len(), 1);
    assert_eq!(persisted.matches[0].semantic_path, "Helper");
    assert_eq!(persisted.matches[0].node_kind, "class_definition");
}

#[test]
fn search_symbols_filtered_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&db_path, "").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&helper, Some("class RenamedHelper:\n    pass\n"))
        .unwrap();

    let filtered = vfs
        .search_symbols_filtered(
            &dir,
            "helper",
            10,
            Some("helper.py"),
            Some("class_definition"),
        )
        .unwrap();
    assert_eq!(filtered.total_matches, 1);
    assert_eq!(filtered.matches.len(), 1);
    assert_eq!(filtered.matches[0].semantic_path, "RenamedHelper");
    assert_eq!(filtered.matches[0].node_kind, "class_definition");
}

#[test]
fn searches_symbol_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    \"\"\"Increment a value.\"\"\"\n    return value + 1\n";
    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let live = search_symbols_context(&dir, "helper", 10).unwrap();
    assert_eq!(live.search.query, "helper");
    assert_eq!(live.search.indexed_files, 2);
    assert_eq!(live.search.total_matches, 1);
    assert_eq!(live.search.matches.len(), 1);
    assert_eq!(live.reads.len(), 1);
    assert_eq!(live.search.matches[0].semantic_path, "helper");
    assert_eq!(live.reads[0].symbol.semantic_path, "helper");
    assert_eq!(live.reads[0].source, helper_source.trim_end_matches('\n'));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_context_from_index(&db_path, "helper", 10).unwrap();
    assert_eq!(persisted.search.query, "helper");
    assert_eq!(persisted.search.indexed_files, 2);
    assert_eq!(persisted.search.total_matches, 1);
    assert_eq!(persisted.search.matches.len(), 1);
    assert_eq!(persisted.reads.len(), 1);
    assert_eq!(persisted.search.matches[0].semantic_path, "helper");
    assert_eq!(persisted.reads[0].symbol.semantic_path, "helper");
    assert_eq!(
        persisted.reads[0].source,
        helper_source.trim_end_matches('\n')
    );
}

#[test]
fn search_symbols_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_source = "def renamed_helper() -> int:\n    return 1\n";
    vfs.open_file(&helper, Some(renamed_source)).unwrap();

    let results = vfs
        .search_symbols_context(&dir, "renamed_helper", 10)
        .unwrap();
    assert_eq!(results.search.total_matches, 1);
    assert_eq!(results.search.matches.len(), 1);
    assert_eq!(results.reads.len(), 1);
    assert_eq!(results.search.matches[0].semantic_path, "renamed_helper");
    assert_eq!(results.reads[0].symbol.semantic_path, "renamed_helper");
    assert_eq!(
        results.reads[0].source,
        renamed_source.trim_end_matches('\n')
    );

    let old_name = vfs.search_symbols_context(&dir, "helper", 10).unwrap();
    assert_eq!(old_name.search.matches[0].semantic_path, "renamed_helper");
    assert_eq!(old_name.reads[0].symbol.semantic_path, "renamed_helper");
    assert!(
        !old_name
            .search
            .matches
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn searches_symbol_neighborhood_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    let orchestrator_symbol = "def orchestrate(value: int) -> int:\n    return helper(value)\n";
    let entry_symbol = "def entrypoint(value: int) -> int:\n    return orchestrate(value)\n";

    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let live =
        search_symbols_neighborhood_context(&dir, "helper", 10, TraceDirection::Callers, 2, 10)
            .unwrap();
    assert_eq!(live.search.query, "helper");
    assert_eq!(live.search.indexed_files, 3);
    assert_eq!(live.search.total_matches, 1);
    assert_eq!(live.search.matches.len(), 1);
    assert_eq!(live.contexts.len(), 1);
    assert_eq!(live.contexts[0].neighborhood.nodes.len(), 3);
    assert_eq!(live.contexts[0].reads.len(), 3);
    assert_eq!(live.contexts[0].reads[0].symbol.semantic_path, "helper");
    assert_eq!(
        live.contexts[0].reads[0].source,
        helper_source.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[0].reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[0].reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_neighborhood_context_from_index(
        &db_path,
        "helper",
        10,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.search.query, "helper");
    assert_eq!(persisted.search.indexed_files, 3);
    assert_eq!(persisted.search.total_matches, 1);
    assert_eq!(persisted.contexts.len(), 1);
    assert_eq!(persisted.contexts[0].neighborhood.nodes.len(), 3);
    assert_eq!(persisted.contexts[0].reads[0].symbol.symbol_id, "helper");
    assert_eq!(
        persisted.contexts[0].reads[1].symbol.symbol_id,
        "orchestrate"
    );
    assert_eq!(
        persisted.contexts[0].reads[2].symbol.symbol_id,
        "entrypoint"
    );
}

#[test]
fn search_symbols_neighborhood_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_orchestrator = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    let renamed_orchestrator_symbol =
        "def orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&orchestrator, Some(renamed_orchestrator))
        .unwrap();

    let results = vfs
        .search_symbols_neighborhood_context(
            &dir,
            "renamed_helper",
            10,
            TraceDirection::Callers,
            2,
            10,
        )
        .unwrap();
    assert_eq!(results.search.total_matches, 1);
    assert_eq!(results.contexts.len(), 1);
    assert_eq!(
        results.contexts[0].neighborhood.symbol.semantic_path,
        "renamed_helper"
    );
    assert_eq!(results.contexts[0].reads.len(), 2);
    assert_eq!(
        results.contexts[0].reads[0].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        results.contexts[0].reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
}

#[test]
fn searches_symbol_discovery_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    let orchestrator_symbol = "def orchestrate(value: int) -> int:\n    return helper(value)\n";
    let entry_symbol = "def entrypoint(value: int) -> int:\n    return orchestrate(value)\n";

    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let live = search_symbols_discovery_context(&dir, "helper", 10, TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(live.search.query, "helper");
    assert_eq!(live.search.indexed_files, 3);
    assert_eq!(live.search.total_matches, 1);
    assert_eq!(live.search.matches.len(), 1);
    assert_eq!(live.reads.len(), 1);
    assert_eq!(live.contexts.len(), 1);
    assert_eq!(live.reads[0].symbol.semantic_path, "helper");
    assert_eq!(live.reads[0].source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.contexts[0].neighborhood.nodes.len(), 3);
    assert_eq!(
        live.contexts[0].reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[0].reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = search_symbols_discovery_context_from_index(
        &db_path,
        "helper",
        10,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.search.query, "helper");
    assert_eq!(persisted.search.indexed_files, 3);
    assert_eq!(persisted.search.total_matches, 1);
    assert_eq!(persisted.reads.len(), 1);
    assert_eq!(persisted.contexts.len(), 1);
    assert_eq!(persisted.reads[0].symbol.symbol_id, "helper");
    assert_eq!(persisted.contexts[0].reads[0].symbol.symbol_id, "helper");
    assert_eq!(
        persisted.contexts[0].reads[1].symbol.symbol_id,
        "orchestrate"
    );
    assert_eq!(
        persisted.contexts[0].reads[2].symbol.symbol_id,
        "entrypoint"
    );
}

#[test]
fn search_symbols_discovery_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_orchestrator = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    let renamed_orchestrator_symbol =
        "def orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&orchestrator, Some(renamed_orchestrator))
        .unwrap();

    let results = vfs
        .search_symbols_discovery_context(
            &dir,
            "renamed_helper",
            10,
            TraceDirection::Callers,
            2,
            10,
        )
        .unwrap();
    assert_eq!(results.search.total_matches, 1);
    assert_eq!(results.reads.len(), 1);
    assert_eq!(results.contexts.len(), 1);
    assert_eq!(results.reads[0].symbol.semantic_path, "renamed_helper");
    assert_eq!(
        results.reads[0].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        results.contexts[0].reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
}

#[test]
fn lists_symbols_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let live = list_symbols(&dir, 10).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.total_symbols, 2);
    assert!(!live.truncated);
    assert_eq!(live.symbols.len(), 2);
    assert_eq!(live.symbols[0].semantic_path, "orchestrate");
    assert_eq!(live.symbols[1].semantic_path, "helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = list_symbols_from_index(&db_path, 10).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.total_symbols, 2);
    assert!(!persisted.truncated);
    assert_eq!(persisted.symbols.len(), 2);
    assert_eq!(persisted.symbols[1].semantic_path, "helper");
}

#[test]
fn list_symbols_filters_and_honors_limit() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let helper_types = dir.join("helper_types.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(
        &helper_types,
        "class Helper:\n    pass\n\ndef helper_factory() -> Helper:\n    return Helper()\n",
    )
    .unwrap();

    let live = list_symbols_filtered(&dir, 1, Some("types"), Some("class_definition")).unwrap();
    assert_eq!(live.total_symbols, 1);
    assert!(!live.truncated);
    assert_eq!(live.symbols.len(), 1);
    assert_eq!(live.symbols[0].semantic_path, "Helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        list_symbols_from_index_filtered(&db_path, 1, Some("helper"), Some("function_definition"))
            .unwrap();
    assert_eq!(persisted.total_symbols, 2);
    assert!(persisted.truncated);
    assert_eq!(persisted.symbols.len(), 1);
    assert_eq!(persisted.symbols[0].semantic_path, "helper");
}

#[test]
fn list_symbols_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&helper, Some("class RenamedHelper:\n    pass\n"))
        .unwrap();

    let listed = vfs
        .list_symbols_filtered(&dir, 10, Some("helper.py"), Some("class_definition"))
        .unwrap();
    assert_eq!(listed.total_symbols, 1);
    assert_eq!(listed.symbols.len(), 1);
    assert_eq!(listed.symbols[0].semantic_path, "RenamedHelper");
    assert_eq!(listed.symbols[0].node_kind, "class_definition");
}

#[test]
fn lists_symbol_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    \"\"\"Increment a value.\"\"\"\n    return value + 1\n";
    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let live = list_symbols_context(&dir, 10).unwrap();
    assert_eq!(live.list.indexed_files, 2);
    assert_eq!(live.list.total_symbols, 2);
    assert_eq!(live.list.symbols.len(), 2);
    assert_eq!(live.reads.len(), 2);
    assert_eq!(live.list.symbols[0].semantic_path, "orchestrate");
    assert_eq!(live.reads[0].symbol.semantic_path, "orchestrate");
    assert_eq!(live.list.symbols[1].semantic_path, "helper");
    assert_eq!(live.reads[1].symbol.semantic_path, "helper");
    assert_eq!(live.reads[1].source, helper_source.trim_end_matches('\n'));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = list_symbols_context_from_index(&db_path, 10).unwrap();
    assert_eq!(persisted.list.indexed_files, 2);
    assert_eq!(persisted.list.total_symbols, 2);
    assert_eq!(persisted.list.symbols.len(), 2);
    assert_eq!(persisted.reads.len(), 2);
    assert_eq!(persisted.list.symbols[0].semantic_path, "orchestrate");
    assert_eq!(persisted.reads[0].symbol.semantic_path, "orchestrate");
    assert_eq!(persisted.list.symbols[1].semantic_path, "helper");
    assert_eq!(persisted.reads[1].symbol.semantic_path, "helper");
    assert_eq!(
        persisted.reads[1].source,
        helper_source.trim_end_matches('\n')
    );
}

#[test]
fn list_symbols_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_source = "class RenamedHelper:\n    pass\n";
    vfs.open_file(&helper, Some(renamed_source)).unwrap();

    let listed = vfs
        .list_symbols_context_filtered(&dir, 10, Some("helper.py"), Some("class_definition"))
        .unwrap();
    assert_eq!(listed.list.total_symbols, 1);
    assert_eq!(listed.list.symbols.len(), 1);
    assert_eq!(listed.reads.len(), 1);
    assert_eq!(listed.list.symbols[0].semantic_path, "RenamedHelper");
    assert_eq!(listed.reads[0].symbol.semantic_path, "RenamedHelper");
    assert_eq!(
        listed.reads[0].source,
        renamed_source.trim_end_matches('\n')
    );
}

#[test]
fn lists_symbol_neighborhood_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    let orchestrator_symbol = "def orchestrate(value: int) -> int:\n    return helper(value)\n";
    let entry_symbol = "def entrypoint(value: int) -> int:\n    return orchestrate(value)\n";

    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let live = list_symbols_neighborhood_context(&dir, 10, TraceDirection::Callers, 2, 10).unwrap();
    assert_eq!(live.list.indexed_files, 3);
    assert_eq!(live.list.total_symbols, 3);
    assert_eq!(live.list.symbols.len(), 3);
    assert_eq!(live.contexts.len(), 3);
    assert_eq!(live.list.symbols[0].semantic_path, "orchestrate");
    assert_eq!(
        live.contexts[0].neighborhood.symbol.semantic_path,
        "orchestrate"
    );
    assert_eq!(live.contexts[0].reads.len(), 2);
    assert_eq!(
        live.contexts[0].reads[0].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(live.contexts[1].neighborhood.symbol.semantic_path, "helper");
    assert_eq!(live.contexts[1].reads.len(), 3);
    assert_eq!(
        live.contexts[1].reads[0].source,
        helper_source.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[1].reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[1].reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[2].neighborhood.symbol.semantic_path,
        "entrypoint"
    );
    assert_eq!(live.contexts[2].reads.len(), 1);
    assert_eq!(
        live.contexts[2].reads[0].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        list_symbols_neighborhood_context_from_index(&db_path, 10, TraceDirection::Callers, 2, 10)
            .unwrap();
    assert_eq!(persisted.list.indexed_files, 3);
    assert_eq!(persisted.list.total_symbols, 3);
    assert_eq!(persisted.contexts.len(), 3);
    assert_eq!(
        persisted.contexts[0].neighborhood.symbol.semantic_path,
        "orchestrate"
    );
    assert_eq!(
        persisted.contexts[1].neighborhood.symbol.semantic_path,
        "helper"
    );
    assert_eq!(
        persisted.contexts[2].neighborhood.symbol.semantic_path,
        "entrypoint"
    );
    assert_eq!(persisted.contexts[1].reads.len(), 3);
    assert_eq!(persisted.contexts[1].reads[0].symbol.symbol_id, "helper");
    assert_eq!(
        persisted.contexts[1].reads[1].symbol.symbol_id,
        "orchestrate"
    );
    assert_eq!(
        persisted.contexts[1].reads[2].symbol.symbol_id,
        "entrypoint"
    );
}

#[test]
fn list_symbols_neighborhood_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_orchestrator = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    let renamed_orchestrator_symbol =
        "def orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&orchestrator, Some(renamed_orchestrator))
        .unwrap();

    let listed = vfs
        .list_symbols_neighborhood_context(&dir, 10, TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(listed.list.total_symbols, 2);
    assert_eq!(listed.contexts.len(), 2);
    assert_eq!(listed.list.symbols[1].semantic_path, "renamed_helper");
    assert_eq!(
        listed.contexts[1].neighborhood.symbol.semantic_path,
        "renamed_helper"
    );
    assert_eq!(listed.contexts[1].reads.len(), 2);
    assert_eq!(
        listed.contexts[1].reads[0].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        listed.contexts[1].reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
}

#[test]
fn lists_symbol_discovery_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    let orchestrator_symbol = "def orchestrate(value: int) -> int:\n    return helper(value)\n";
    let entry_symbol = "def entrypoint(value: int) -> int:\n    return orchestrate(value)\n";

    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let live = list_symbols_discovery_context(&dir, 10, TraceDirection::Callers, 2, 10).unwrap();
    assert_eq!(live.list.indexed_files, 3);
    assert_eq!(live.list.total_symbols, 3);
    assert_eq!(live.list.symbols.len(), 3);
    assert_eq!(live.reads.len(), 3);
    assert_eq!(live.contexts.len(), 3);
    assert_eq!(live.list.symbols[0].semantic_path, "orchestrate");
    assert_eq!(live.reads[0].symbol.semantic_path, "orchestrate");
    assert_eq!(
        live.reads[0].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(live.contexts[1].neighborhood.symbol.semantic_path, "helper");
    assert_eq!(live.contexts[1].reads.len(), 3);
    assert_eq!(
        live.contexts[1].reads[0].source,
        helper_source.trim_end_matches('\n')
    );
    assert_eq!(
        live.contexts[2].reads[0].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        list_symbols_discovery_context_from_index(&db_path, 10, TraceDirection::Callers, 2, 10)
            .unwrap();
    assert_eq!(persisted.list.indexed_files, 3);
    assert_eq!(persisted.list.total_symbols, 3);
    assert_eq!(persisted.reads.len(), 3);
    assert_eq!(persisted.contexts.len(), 3);
    assert_eq!(persisted.reads[1].symbol.symbol_id, "helper");
    assert_eq!(persisted.contexts[1].reads[0].symbol.symbol_id, "helper");
    assert_eq!(
        persisted.contexts[1].reads[1].symbol.symbol_id,
        "orchestrate"
    );
    assert_eq!(
        persisted.contexts[1].reads[2].symbol.symbol_id,
        "entrypoint"
    );
}

#[test]
fn list_symbols_discovery_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_orchestrator = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    let renamed_orchestrator_symbol =
        "def orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&orchestrator, Some(renamed_orchestrator))
        .unwrap();

    let listed = vfs
        .list_symbols_discovery_context(&dir, 10, TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(listed.list.total_symbols, 2);
    assert_eq!(listed.reads.len(), 2);
    assert_eq!(listed.contexts.len(), 2);
    assert_eq!(listed.reads[1].symbol.semantic_path, "renamed_helper");
    assert_eq!(
        listed.reads[1].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        listed.contexts[1].reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
}

#[test]
fn reads_symbol_source_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let live = read_symbol(&dir, "helper").unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.start_point.row, 0);
    assert!(live.end_point.row >= live.start_point.row);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = read_symbol_from_index(&db_path, "helper").unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.source, helper_source.trim_end_matches('\n'));
}

#[test]
fn read_symbol_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_source = "def renamed_helper() -> int:\n    return 2\n";
    vfs.open_file(&helper, Some(renamed_source)).unwrap();

    let result = vfs.read_symbol(&dir, "renamed_helper").unwrap();
    assert_eq!(result.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.source, renamed_source.trim_end_matches('\n'));
    assert_eq!(result.start_point.row, 0);
}

#[test]
fn reads_symbol_at_position_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let position = Position { row: 0, column: 5 };
    let live = read_symbol_at_position(&dir, &helper, &position).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.start_point.row, 0);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = read_symbol_at_position_from_index(&db_path, &helper, &position).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.source, helper_source.trim_end_matches('\n'));
    assert_eq!(persisted.start_point.row, 0);
}

#[test]
fn traces_symbol_graph_at_position_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let position = Position { row: 0, column: 5 };
    let live =
        trace_symbol_graph_at_position(&dir, &helper, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "orchestrate");
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &helper,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "orchestrate");
}

#[test]
fn traces_symbol_neighborhood_at_position_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let position = Position { row: 0, column: 5 };
    let live = trace_symbol_neighborhood_at_position(
        &dir,
        &helper,
        &position,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.nodes.len(), 3);
    assert_eq!(live.nodes[1].symbol.semantic_path, "orchestrate");
    assert_eq!(live.nodes[2].symbol.semantic_path, "entrypoint");
    assert_eq!(live.edges.len(), 2);
    assert!(!live.truncated);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_neighborhood_at_position_from_index(
        &db_path,
        &helper,
        &position,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.nodes.len(), 3);
    assert_eq!(persisted.edges.len(), 2);
    assert!(!persisted.truncated);
}

#[test]
fn read_symbol_at_position_resolves_decorator_lines() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");

    fs::write(
            &helper,
            "def decorator(func):\n    return func\n\n@decorator\ndef helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let result = read_symbol_at_position(&dir, &helper, &Position { row: 3, column: 1 })
        .expect("decorator line should resolve to the decorated symbol");

    assert_eq!(result.symbol.semantic_path, "helper");
    assert_eq!(
        result.symbol.signature.as_deref(),
        Some("@decorator\ndef helper(value: int) -> int:")
    );
    assert!(result.source.starts_with("@decorator\ndef helper"));
    assert_eq!(result.start_point.row, 3);
}

#[test]
fn reads_c_symbol_at_position_for_declaration_and_definition_exactly() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let db_path = dir.join("symbols.db");

    let declaration_source = "int helper(int value);\n";
    let definition_source =
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n";
    fs::write(&header, declaration_source).unwrap();
    fs::write(&source, definition_source).unwrap();

    let declaration_live =
        read_symbol_at_position(&dir, &header, &Position { row: 0, column: 4 }).unwrap();
    let definition_live =
        read_symbol_at_position(&dir, &source, &Position { row: 2, column: 4 }).unwrap();

    assert_eq!(declaration_live.symbol.node_kind, "declaration");
    assert_eq!(
        declaration_live.source,
        declaration_source.trim_end_matches('\n')
    );
    assert_eq!(definition_live.symbol.node_kind, "function_definition");
    assert_eq!(
        definition_live.source,
        "int helper(int value) {\n    return value + 1;\n}"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let declaration_persisted =
        read_symbol_at_position_from_index(&db_path, &header, &Position { row: 0, column: 4 })
            .unwrap();
    let definition_persisted =
        read_symbol_at_position_from_index(&db_path, &source, &Position { row: 2, column: 4 })
            .unwrap();

    assert_eq!(declaration_persisted.symbol.node_kind, "declaration");
    assert_eq!(
        declaration_persisted.source,
        declaration_source.trim_end_matches('\n')
    );
    assert_eq!(definition_persisted.symbol.node_kind, "function_definition");
    assert_eq!(
        definition_persisted.source,
        "int helper(int value) {\n    return value + 1;\n}"
    );
}

#[test]
fn patches_python_symbol_at_position_from_decorator_line() {
    let file = temporary_dir().join("helper.py");
    fs::write(
            &file,
            "def decorator(func):\n    return func\n\n@decorator\ndef helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let result = patch_ast_node_at_position(
        &file,
        &fs::read_to_string(&file).unwrap(),
        &Position { row: 3, column: 1 },
        "def helper(value: int) -> int:\n    return value + 2\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert_eq!(result.resolved_path, "helper");
    assert_eq!(result.resolved_symbol_id, "helper");
    assert!(
        result
            .validation
            .syntax_errors
            .iter()
            .any(|issue| issue.kind == "decorator_guard")
    );
    assert!(result.updated_source.contains("return value + 2"));
}

#[test]
fn patches_c_symbols_at_position_exactly() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let patched_declaration = patch_ast_node_at_position(
        &header,
        &fs::read_to_string(&header).unwrap(),
        &Position { row: 0, column: 4 },
        "long helper(long value);",
        None,
    )
    .unwrap();
    assert!(patched_declaration.applied);
    assert_eq!(patched_declaration.resolved_path, "helper");
    assert_eq!(
        patched_declaration.updated_source,
        "long helper(long value);\n"
    );

    let patched_definition = patch_ast_node_at_position(
        &source,
        &fs::read_to_string(&source).unwrap(),
        &Position { row: 2, column: 4 },
        "int helper(int value) {\n    return value + 2;\n}\n",
        None,
    )
    .unwrap();
    assert!(patched_definition.applied);
    assert_eq!(patched_definition.resolved_path, "helper");
    assert!(
        patched_definition
            .resolved_symbol_id
            .ends_with("helper.h::helper")
    );
    assert!(
        patched_definition
            .updated_source
            .contains("return value + 2;")
    );
    assert!(
        patched_definition
            .updated_source
            .contains("#include \"helper.h\"")
    );
}

#[test]
fn reads_symbol_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let live = read_symbol_context(&dir, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.read.indexed_files, 2);
    assert_eq!(live.trace.indexed_files, 2);
    assert_eq!(live.read.symbol.semantic_path, "helper");
    assert_eq!(live.trace.symbol.semantic_path, "helper");
    assert_eq!(live.read.source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.trace.callers.len(), 1);
    assert_eq!(live.trace.callers[0].semantic_path, "orchestrate");
    assert!(live.trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        read_symbol_context_from_index(&db_path, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.read.indexed_files, 2);
    assert_eq!(persisted.trace.indexed_files, 2);
    assert_eq!(persisted.read.symbol.symbol_id, "helper");
    assert_eq!(persisted.trace.symbol.symbol_id, "helper");
    assert_eq!(persisted.read.source, helper_source.trim_end_matches('\n'));
    assert_eq!(persisted.trace.callers.len(), 1);
    assert_eq!(persisted.trace.callers[0].semantic_path, "orchestrate");
}

#[test]
fn read_symbol_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_caller = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&caller, Some(renamed_caller)).unwrap();

    let result = vfs
        .read_symbol_context(&dir, "renamed_helper", TraceDirection::Callers)
        .unwrap();
    assert_eq!(result.read.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.trace.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.read.source, renamed_helper.trim_end_matches('\n'));
    assert_eq!(result.trace.callers.len(), 1);
    assert_eq!(result.trace.callers[0].semantic_path, "orchestrate");
}

#[test]
fn trace_symbol_graph_at_position_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_caller = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&caller, Some(renamed_caller)).unwrap();

    let result = vfs
        .trace_symbol_graph_at_position(
            &dir,
            &helper,
            &Position { row: 0, column: 5 },
            TraceDirection::Callers,
        )
        .unwrap();
    assert_eq!(result.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.callers.len(), 1);
    assert_eq!(result.callers[0].semantic_path, "orchestrate");
}

#[test]
fn trace_symbol_graph_at_position_with_source_normalizes_path_without_writing_disk() {
    let dir = temporary_dir();
    let nested = dir.join("child");
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let caller_alias = nested.join("..").join("caller.py");

    fs::create_dir_all(&nested).unwrap();
    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let result = trace_symbol_graph_at_position_with_source(
            &dir,
            &caller_alias,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
            &Position { row: 3, column: 5 },
            TraceDirection::Both,
        )
        .unwrap();

    assert!(!caller.exists());
    assert_eq!(result.symbol.semantic_path, "orchestrate");
    assert_eq!(result.symbol.file_path, normalize_path(&caller));
    assert!(
        result
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn read_symbol_context_at_position_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_caller = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&caller, Some(renamed_caller)).unwrap();

    let result = vfs
        .read_symbol_context_at_position(
            &dir,
            &helper,
            &Position { row: 0, column: 5 },
            TraceDirection::Callers,
        )
        .unwrap();
    assert_eq!(result.read.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.trace.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.read.source, renamed_helper.trim_end_matches('\n'));
    assert_eq!(result.trace.callers.len(), 1);
    assert_eq!(result.trace.callers[0].semantic_path, "orchestrate");
}

#[test]
fn validates_patch_with_discovery_context_at_position_in_one_call() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let entry = dir.join("entry.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from caller import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let result = validate_patch_with_discovery_context_at_position(
        &dir,
        &helper,
        "def helper(value: int) -> int:\n    return value + 2\n",
        &Position { row: 0, column: 5 },
        "def helper(value: int) -> int:\n    return value + 2\n",
        None,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.trace_target, "helper");
    assert_eq!(result.patch.resolved_path, "helper");
    assert_eq!(
        result.trace.as_ref().unwrap().callers[0].semantic_path,
        "orchestrate"
    );
    assert_eq!(result.read.as_ref().unwrap().symbol.semantic_path, "helper");
    assert_eq!(result.neighborhood_context.as_ref().unwrap().reads.len(), 3);
    assert_eq!(
        result.neighborhood_context.as_ref().unwrap().reads[1]
            .symbol
            .semantic_path,
        "orchestrate"
    );
}

#[test]
fn validates_patch_with_trace_context_at_position_in_one_call() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let result = validate_patch_with_trace_context_at_position(
            &dir,
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
            &Position { row: 3, column: 5 },
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
            None,
            TraceDirection::Both,
        )
        .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.trace_target, "orchestrate");
    assert_eq!(result.patch.resolved_path, "orchestrate");
    assert_eq!(
        result.trace.as_ref().unwrap().symbol.semantic_path,
        "orchestrate"
    );
    assert_eq!(
        result.trace.as_ref().unwrap().callees[0].semantic_path,
        "helper"
    );
    assert!(result.trace_validation.as_ref().unwrap().allowed);
}

#[test]
fn reads_symbol_discovery_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    let orchestrator_source = "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let orchestrator_symbol = "def orchestrate(value: int) -> int:\n    return helper(value)\n";
    let entry_source = "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n";
    let entry_symbol = "def entrypoint(value: int) -> int:\n    return orchestrate(value)\n";

    fs::write(&helper, helper_source).unwrap();
    fs::write(&orchestrator, orchestrator_source).unwrap();
    fs::write(&entry, entry_source).unwrap();

    let live =
        read_symbol_discovery_context(&dir, "helper", TraceDirection::Callers, 2, 10).unwrap();
    assert_eq!(live.read.indexed_files, 3);
    assert_eq!(live.trace.indexed_files, 3);
    assert_eq!(live.neighborhood_context.neighborhood.indexed_files, 3);
    assert_eq!(live.read.symbol.semantic_path, "helper");
    assert_eq!(live.trace.symbol.semantic_path, "helper");
    assert_eq!(
        live.neighborhood_context.neighborhood.symbol.semantic_path,
        "helper"
    );
    assert_eq!(live.read.source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.trace.callers.len(), 1);
    assert_eq!(live.trace.callers[0].semantic_path, "orchestrate");
    assert_eq!(live.neighborhood_context.reads.len(), 3);
    assert_eq!(
        live.neighborhood_context.reads[0].source,
        helper_source.trim_end_matches('\n')
    );
    assert_eq!(
        live.neighborhood_context.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.neighborhood_context.reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = read_symbol_discovery_context_from_index(
        &db_path,
        "helper",
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.read.indexed_files, 3);
    assert_eq!(persisted.trace.indexed_files, 3);
    assert_eq!(persisted.neighborhood_context.neighborhood.indexed_files, 3);
    assert_eq!(persisted.read.symbol.symbol_id, "helper");
    assert_eq!(persisted.trace.symbol.symbol_id, "helper");
    assert_eq!(
        persisted.neighborhood_context.neighborhood.symbol.symbol_id,
        "helper"
    );
    assert_eq!(persisted.read.source, helper_source.trim_end_matches('\n'));
    assert_eq!(
        persisted.neighborhood_context.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        persisted.neighborhood_context.reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );
}

#[test]
fn read_symbol_discovery_context_at_position_with_source_normalizes_path_without_writing_disk() {
    let dir = temporary_dir();
    let nested = dir.join("child");
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let caller_alias = nested.join("..").join("caller.py");
    let entry = dir.join("entry.py");

    fs::create_dir_all(&nested).unwrap();
    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &entry,
            "from caller import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let result = read_symbol_discovery_context_at_position_with_source(
            &dir,
            &caller_alias,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
            &Position { row: 3, column: 5 },
            TraceDirection::Both,
            2,
            10,
        )
        .unwrap();

    assert!(!caller.exists());
    assert_eq!(result.read.symbol.semantic_path, "orchestrate");
    assert_eq!(result.read.symbol.file_path, normalize_path(&caller));
    assert_eq!(result.trace.symbol.file_path, normalize_path(&caller));
    assert!(
        result
            .neighborhood_context
            .reads
            .iter()
            .any(|read| read.symbol.semantic_path == "helper")
    );
}

#[test]
fn reads_symbol_discovery_context_at_position_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    let orchestrator_symbol = "def orchestrate(value: int) -> int:\n    return helper(value)\n";
    let entry_symbol = "def entrypoint(value: int) -> int:\n    return orchestrate(value)\n";

    fs::write(&helper, helper_source).unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let position = Position { row: 0, column: 5 };
    let live = read_symbol_discovery_context_at_position(
        &dir,
        &helper,
        &position,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(live.read.indexed_files, 3);
    assert_eq!(live.trace.indexed_files, 3);
    assert_eq!(live.neighborhood_context.neighborhood.indexed_files, 3);
    assert_eq!(live.read.symbol.semantic_path, "helper");
    assert_eq!(live.trace.callers[0].semantic_path, "orchestrate");
    assert_eq!(live.neighborhood_context.reads.len(), 3);
    assert_eq!(live.read.source, helper_source.trim_end_matches('\n'));
    assert_eq!(
        live.neighborhood_context.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        live.neighborhood_context.reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = read_symbol_discovery_context_at_position_from_index(
        &db_path,
        &helper,
        &position,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.read.indexed_files, 3);
    assert_eq!(persisted.trace.indexed_files, 3);
    assert_eq!(persisted.neighborhood_context.neighborhood.indexed_files, 3);
    assert_eq!(persisted.read.symbol.symbol_id, "helper");
    assert_eq!(
        persisted.neighborhood_context.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        persisted.neighborhood_context.reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );
}

#[test]
fn read_symbol_discovery_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_orchestrator = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    let renamed_orchestrator_symbol =
        "def orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&orchestrator, Some(renamed_orchestrator))
        .unwrap();

    let result = vfs
        .read_symbol_discovery_context(&dir, "renamed_helper", TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(result.read.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.trace.symbol.semantic_path, "renamed_helper");
    assert_eq!(
        result
            .neighborhood_context
            .neighborhood
            .symbol
            .semantic_path,
        "renamed_helper"
    );
    assert_eq!(result.read.source, renamed_helper.trim_end_matches('\n'));
    assert_eq!(result.trace.callers.len(), 1);
    assert_eq!(result.trace.callers[0].semantic_path, "orchestrate");
    assert_eq!(result.neighborhood_context.reads.len(), 3);
    assert_eq!(
        result.neighborhood_context.reads[0].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        result.neighborhood_context.reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        result.neighborhood_context.reads[0].symbol.semantic_path,
        "renamed_helper"
    );
    assert_eq!(
        result.neighborhood_context.reads[1].symbol.semantic_path,
        "orchestrate"
    );
    assert_eq!(
        result.neighborhood_context.reads[2].symbol.semantic_path,
        "entrypoint"
    );
}

#[test]
fn reads_symbol_neighborhood_context_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    let helper_source = "def helper(value: int) -> int:\n    return value + 1\n";
    let orchestrator_source = "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let orchestrator_symbol = "def orchestrate(value: int) -> int:\n    return helper(value)\n";
    let entry_source = "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n";
    let entry_symbol = "def entrypoint(value: int) -> int:\n    return orchestrate(value)\n";

    fs::write(&helper, helper_source).unwrap();
    fs::write(&orchestrator, orchestrator_source).unwrap();
    fs::write(&entry, entry_source).unwrap();

    let live =
        read_symbol_neighborhood_context(&dir, "helper", TraceDirection::Callers, 2, 10).unwrap();
    assert_eq!(live.neighborhood.indexed_files, 3);
    assert_eq!(live.neighborhood.nodes.len(), 3);
    assert_eq!(live.reads.len(), 3);
    assert_eq!(live.reads[0].symbol.semantic_path, "helper");
    assert_eq!(live.reads[0].source, helper_source.trim_end_matches('\n'));
    assert_eq!(live.reads[1].symbol.semantic_path, "orchestrate");
    assert_eq!(
        live.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(live.reads[2].symbol.semantic_path, "entrypoint");
    assert_eq!(live.reads[2].source, entry_symbol.trim_end_matches('\n'));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = read_symbol_neighborhood_context_from_index(
        &db_path,
        "helper",
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();
    assert_eq!(persisted.neighborhood.indexed_files, 3);
    assert_eq!(persisted.neighborhood.nodes.len(), 3);
    assert_eq!(persisted.reads.len(), 3);
    assert_eq!(persisted.reads[0].symbol.symbol_id, "helper");
    assert_eq!(persisted.reads[1].symbol.symbol_id, "orchestrate");
    assert_eq!(persisted.reads[2].symbol.symbol_id, "entrypoint");
    assert_eq!(
        persisted.reads[0].source,
        helper_source.trim_end_matches('\n')
    );
    assert_eq!(
        persisted.reads[1].source,
        orchestrator_symbol.trim_end_matches('\n')
    );
    assert_eq!(
        persisted.reads[2].source,
        entry_symbol.trim_end_matches('\n')
    );
}

#[test]
fn read_symbol_neighborhood_context_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_orchestrator = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    let renamed_orchestrator_symbol =
        "def orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&orchestrator, Some(renamed_orchestrator))
        .unwrap();

    let truncated = vfs
        .read_symbol_neighborhood_context(&dir, "renamed_helper", TraceDirection::Callers, 2, 2)
        .unwrap();
    assert_eq!(
        truncated.neighborhood.symbol.semantic_path,
        "renamed_helper"
    );
    assert_eq!(truncated.neighborhood.nodes.len(), 2);
    assert_eq!(truncated.reads.len(), 2);
    assert_eq!(
        truncated.reads[0].source,
        renamed_helper.trim_end_matches('\n')
    );
    assert_eq!(
        truncated.reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
    assert!(truncated.neighborhood.truncated);

    let full = vfs
        .read_symbol_neighborhood_context(&dir, "renamed_helper", TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(full.neighborhood.nodes.len(), 3);
    assert_eq!(full.reads.len(), 3);
    assert_eq!(full.reads[0].symbol.semantic_path, "renamed_helper");
    assert_eq!(full.reads[1].symbol.semantic_path, "orchestrate");
    assert_eq!(full.reads[2].symbol.semantic_path, "entrypoint");
    assert_eq!(full.reads[0].source, renamed_helper.trim_end_matches('\n'));
    assert_eq!(
        full.reads[1].source,
        renamed_orchestrator_symbol.trim_end_matches('\n')
    );
    assert!(!full.neighborhood.truncated);
}

#[test]
fn traces_symbol_neighborhood_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let live = trace_symbol_neighborhood(&dir, "helper", TraceDirection::Callers, 2, 10).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.nodes.len(), 3);
    assert_eq!(live.nodes[0].symbol.semantic_path, "helper");
    assert_eq!(live.nodes[0].depth, 0);
    assert_eq!(live.nodes[1].symbol.semantic_path, "orchestrate");
    assert_eq!(live.nodes[1].depth, 1);
    assert_eq!(live.nodes[2].symbol.semantic_path, "entrypoint");
    assert_eq!(live.nodes[2].depth, 2);
    assert_eq!(live.edges.len(), 2);
    assert_eq!(live.edges[0].from_symbol_id, "orchestrate");
    assert_eq!(live.edges[0].to_symbol_id, "helper");
    assert_eq!(live.edges[1].from_symbol_id, "entrypoint");
    assert_eq!(live.edges[1].to_symbol_id, "orchestrate");
    assert!(!live.truncated);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_neighborhood_from_index(&db_path, "helper", TraceDirection::Callers, 2, 10)
            .unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.nodes.len(), 3);
    assert_eq!(persisted.edges.len(), 2);
    assert_eq!(persisted.nodes[2].symbol.semantic_path, "entrypoint");
    assert!(!persisted.truncated);
}

#[test]
fn trace_symbol_neighborhood_respects_max_nodes_and_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let orchestrator = dir.join("graph_a.py");
    let entry = dir.join("graph_c.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &orchestrator,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from graph_a import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_orchestrator = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&orchestrator, Some(renamed_orchestrator))
        .unwrap();

    let truncated = vfs
        .trace_symbol_neighborhood(&dir, "renamed_helper", TraceDirection::Callers, 2, 2)
        .unwrap();
    assert_eq!(truncated.symbol.semantic_path, "renamed_helper");
    assert_eq!(truncated.nodes.len(), 2);
    assert_eq!(truncated.nodes[1].symbol.semantic_path, "orchestrate");
    assert_eq!(truncated.edges.len(), 1);
    assert!(truncated.truncated);

    let full = vfs
        .trace_symbol_neighborhood(&dir, "renamed_helper", TraceDirection::Callers, 2, 10)
        .unwrap();
    assert_eq!(full.nodes.len(), 3);
    assert_eq!(full.nodes[0].symbol.semantic_path, "renamed_helper");
    assert_eq!(full.nodes[1].symbol.semantic_path, "orchestrate");
    assert_eq!(full.nodes[2].symbol.semantic_path, "entrypoint");
    assert_eq!(full.nodes[2].depth, 2);
    assert!(!full.truncated);
}
