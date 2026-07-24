use super::*;

#[test]
fn symbol_query_context_applies_multiple_workspace_overlays_without_writing_disk() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&caller, "def orchestrate() -> int:\n    return 0\n").unwrap();

    let helper_source =
        "def helper() -> int:\n    return 1\n\n\ndef helper_alias() -> int:\n    return helper()\n";
    let caller_source = "from helper import helper_alias\n\n\ndef orchestrate() -> int:\n    return helper_alias()\n";
    let context = SymbolQueryContext::workspace(&dir)
        .unwrap()
        .with_source_overlay(&helper, helper_source)
        .unwrap()
        .with_source_overlay(&caller, caller_source)
        .unwrap();

    let search = context
        .search_symbols("helper_alias", 10, None, None)
        .unwrap();
    assert_eq!(search.total_matches, 1);

    let trace = context
        .trace_symbol_graph("orchestrate", TraceDirection::Callees)
        .unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );

    let listed = context.list_symbols(10, None, None).unwrap();
    assert!(
        listed
            .symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );
    assert!(
        !fs::read_to_string(&helper)
            .unwrap()
            .contains("helper_alias")
    );
    assert!(
        !fs::read_to_string(&caller)
            .unwrap()
            .contains("helper_alias")
    );
}

#[test]
fn symbol_query_context_applies_multiple_index_overlays() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&caller, "def orchestrate() -> int:\n    return 0\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let helper_source =
        "def helper() -> int:\n    return 1\n\n\ndef helper_alias() -> int:\n    return helper()\n";
    let caller_source = "from helper import helper_alias\n\n\ndef orchestrate() -> int:\n    return helper_alias()\n";
    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(&helper, helper_source)
        .unwrap()
        .with_source_overlay(&caller, caller_source)
        .unwrap();

    let search = context
        .search_symbols("helper_alias", 10, None, None)
        .unwrap();
    assert_eq!(search.total_matches, 1);

    let trace = context
        .trace_symbol_graph("orchestrate", TraceDirection::Callees)
        .unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );

    let listed = context.list_symbols(10, None, None).unwrap();
    assert!(
        listed
            .symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );
}
