use super::*;

use crate::{
    list_symbols_context_from_index_with_source_filtered,
    list_symbols_from_index_with_source_filtered, read_symbol_context_from_index_with_source,
    read_symbol_from_index_with_source, read_symbol_neighborhood_context_from_index_with_source,
    search_symbols_context_from_index_with_source_filtered,
    search_symbols_from_index_with_source_filtered, trace_symbol_graph_from_index_with_source,
};

#[test]
fn dirty_vfs_list_matches_persisted_index_with_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let dirty = "class RenamedHelper:\n    pass\n";
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&helper, Some(dirty)).unwrap();

    let from_vfs = vfs
        .list_symbols_filtered(&dir, 10, Some("helper.py"), Some("class_definition"))
        .unwrap();
    let from_index = list_symbols_from_index_with_source_filtered(
        &db_path,
        &helper,
        dirty,
        10,
        Some("helper.py"),
        Some("class_definition"),
    )
    .unwrap();

    assert_eq!(from_vfs.total_symbols, 1);
    assert_eq!(from_index.total_symbols, 1);
    assert_eq!(from_vfs.symbols.len(), from_index.symbols.len());
    assert_eq!(
        from_vfs.symbols[0].semantic_path,
        from_index.symbols[0].semantic_path
    );
    assert_eq!(from_vfs.symbols[0].semantic_path, "RenamedHelper");
    assert_eq!(
        from_vfs.symbols[0].node_kind,
        from_index.symbols[0].node_kind
    );
    // Disk must stay clean.
    assert!(fs::read_to_string(&helper).unwrap().contains("def helper"));
    assert!(
        !fs::read_to_string(&helper)
            .unwrap()
            .contains("RenamedHelper")
    );
}

#[test]
fn dirty_vfs_search_and_read_match_persisted_index_with_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let dirty =
        "def helper() -> int:\n    return 1\n\n\ndef helper_alias() -> int:\n    return helper()\n";
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&helper, Some(dirty)).unwrap();

    let vfs_search = vfs
        .search_symbols_filtered(&dir, "helper_alias", 10, None, None)
        .unwrap();
    let index_search = search_symbols_from_index_with_source_filtered(
        &db_path,
        &helper,
        dirty,
        "helper_alias",
        10,
        None,
        None,
    )
    .unwrap();
    assert_eq!(vfs_search.total_matches, 1);
    assert_eq!(index_search.total_matches, 1);
    assert_eq!(
        vfs_search.matches[0].semantic_path,
        index_search.matches[0].semantic_path
    );
    assert_eq!(vfs_search.matches[0].semantic_path, "helper_alias");

    let vfs_read = vfs.read_symbol(&dir, "helper_alias").unwrap();
    let index_read =
        read_symbol_from_index_with_source(&db_path, &helper, dirty, "helper_alias").unwrap();
    assert_eq!(
        vfs_read.symbol.semantic_path,
        index_read.symbol.semantic_path
    );
    assert_eq!(vfs_read.source, index_read.source);
    assert!(vfs_read.source.contains("def helper_alias"));
}

#[test]
fn dirty_vfs_trace_matches_persisted_index_with_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return value\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let dirty_caller = "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&caller, Some(dirty_caller)).unwrap();

    let vfs_trace = vfs
        .trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both)
        .unwrap();
    let index_trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller,
        dirty_caller,
        "orchestrate",
        TraceDirection::Both,
    )
    .unwrap();

    assert_eq!(vfs_trace.symbol.semantic_path, "orchestrate");
    assert_eq!(
        vfs_trace.symbol.semantic_path,
        index_trace.symbol.semantic_path
    );
    assert_eq!(vfs_trace.callees.len(), index_trace.callees.len());
    assert!(
        vfs_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    assert!(
        index_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    assert!(
        !fs::read_to_string(&caller)
            .unwrap()
            .contains("from helper import helper")
    );
}

#[test]
fn dirty_vfs_read_and_list_context_match_persisted_index_with_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return value\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let dirty_caller = "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&caller, Some(dirty_caller)).unwrap();

    let vfs_context = vfs
        .read_symbol_context(&dir, "orchestrate", TraceDirection::Both)
        .unwrap();
    let index_context = read_symbol_context_from_index_with_source(
        &db_path,
        &caller,
        dirty_caller,
        "orchestrate",
        TraceDirection::Both,
    )
    .unwrap();
    assert_eq!(
        vfs_context.read.symbol.semantic_path,
        index_context.read.symbol.semantic_path
    );
    assert_eq!(
        vfs_context.trace.callees.len(),
        index_context.trace.callees.len()
    );
    assert!(
        vfs_context
            .trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    assert!(
        index_context
            .trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    assert_eq!(vfs_context.read.source, index_context.read.source);
    assert!(vfs_context.read.source.contains("return helper(value)"));

    // Single-file list-context parity keeps reads aligned 1:1 with symbols.
    let renamed = "class RenamedHelper:\n    pass\n";
    vfs.open_file(&helper, Some(renamed)).unwrap();
    let vfs_list_context = vfs
        .list_symbols_context_filtered(&dir, 10, Some("helper.py"), Some("class_definition"))
        .unwrap();
    let index_list_context = list_symbols_context_from_index_with_source_filtered(
        &db_path,
        &helper,
        renamed,
        10,
        Some("helper.py"),
        Some("class_definition"),
    )
    .unwrap();
    assert_eq!(vfs_list_context.list.total_symbols, 1);
    assert_eq!(index_list_context.list.total_symbols, 1);
    assert_eq!(
        vfs_list_context.list.symbols[0].semantic_path,
        index_list_context.list.symbols[0].semantic_path
    );
    assert_eq!(
        vfs_list_context.list.symbols[0].semantic_path,
        "RenamedHelper"
    );
    assert_eq!(
        vfs_list_context.reads[0].symbol.semantic_path,
        index_list_context.reads[0].symbol.semantic_path
    );
    assert_eq!(
        vfs_list_context.reads[0].source,
        index_list_context.reads[0].source
    );
    assert_eq!(
        vfs_list_context.reads[0].source,
        renamed.trim_end_matches('\n')
    );
}

#[test]
fn dirty_vfs_neighborhood_and_search_context_match_persisted_index_with_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return value\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let dirty_caller = "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&caller, Some(dirty_caller)).unwrap();

    let vfs_neighborhood = vfs
        .read_symbol_neighborhood_context(&dir, "orchestrate", TraceDirection::Both, 2, 32)
        .unwrap();
    let index_neighborhood = read_symbol_neighborhood_context_from_index_with_source(
        &db_path,
        &caller,
        dirty_caller,
        "orchestrate",
        TraceDirection::Both,
        2,
        32,
    )
    .unwrap();
    assert_eq!(
        vfs_neighborhood.neighborhood.symbol.semantic_path,
        index_neighborhood.neighborhood.symbol.semantic_path
    );
    assert_eq!(
        vfs_neighborhood.neighborhood.nodes.len(),
        index_neighborhood.neighborhood.nodes.len()
    );
    assert!(
        vfs_neighborhood
            .neighborhood
            .nodes
            .iter()
            .any(|node| node.symbol.semantic_path == "helper")
    );
    assert!(
        index_neighborhood
            .neighborhood
            .nodes
            .iter()
            .any(|node| node.symbol.semantic_path == "helper")
    );

    let vfs_search_context = vfs
        .search_symbols_context_filtered(&dir, "orchestrate", 10, None, None)
        .unwrap();
    let index_search_context = search_symbols_context_from_index_with_source_filtered(
        &db_path,
        &caller,
        dirty_caller,
        "orchestrate",
        10,
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        vfs_search_context.search.total_matches,
        index_search_context.search.total_matches
    );
    assert_eq!(
        vfs_search_context.search.matches[0].semantic_path,
        index_search_context.search.matches[0].semantic_path
    );
    assert_eq!(
        vfs_search_context.reads[0].source,
        index_search_context.reads[0].source
    );
    assert!(
        vfs_search_context.reads[0]
            .source
            .contains("return helper(value)")
    );
}
