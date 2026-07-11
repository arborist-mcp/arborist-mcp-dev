use std::fs;

use super::support::{normalize_string_path, temporary_dir};
use crate::{
    Position, SymbolQueryContext, TraceDirection, list_symbols_from_index_with_source_filtered,
    read_symbol_context_from_index_with_source, rebuild_symbol_index,
    search_symbols_from_index_with_source_filtered,
    trace_symbol_graph_at_position_from_index_with_source,
    trace_symbol_graph_from_index_with_source, validate_patch_with_trace_context_from_index,
    validate_patch_with_trace_context_from_path,
};

#[test]
fn validates_trace_context_from_index_with_unsaved_source_overlay() {
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
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n";
    let result = validate_patch_with_trace_context_from_index(
        &db_path,
        &caller,
        source,
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        None,
        TraceDirection::Both,
    )
    .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.patch.file, normalize_string_path(&caller));
    assert!(result.trace_error.is_none());
    assert_eq!(
        result.trace_validation.as_ref().map(|value| value.allowed),
        Some(true)
    );
    let trace = result.trace.expect("trace should be present");
    assert_eq!(trace.symbol.semantic_path, "orchestrate");
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn traces_symbol_graph_from_index_with_unsaved_source_overlay() {
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
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller,
        source,
        "orchestrate",
        TraceDirection::Both,
    )
    .unwrap();

    assert_eq!(trace.symbol.semantic_path, "orchestrate");
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    assert!(
        fs::read_to_string(&caller)
            .unwrap()
            .contains("return value + 1")
    );
}

#[test]
fn trace_symbol_graph_at_position_from_index_with_unsaved_source_overlay() {
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
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let trace = trace_symbol_graph_at_position_from_index_with_source(
        &db_path,
        &caller,
        source,
        &Position { row: 3, column: 5 },
        TraceDirection::Both,
    )
    .unwrap();

    assert_eq!(trace.symbol.semantic_path, "orchestrate");
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn reads_symbol_context_from_index_with_unsaved_source_overlay() {
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
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let context = read_symbol_context_from_index_with_source(
        &db_path,
        &caller,
        source,
        "helper",
        TraceDirection::Callers,
    )
    .unwrap();

    assert_eq!(context.read.symbol.semantic_path, "helper");
    assert_eq!(context.trace.symbol.semantic_path, "helper");
    assert_eq!(context.trace.callers.len(), 1);
    assert_eq!(context.trace.callers[0].semantic_path, "orchestrate");
}

#[test]
fn searches_symbols_from_index_with_unsaved_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source =
        "def helper() -> int:\n    return 1\n\n\ndef helper_alias() -> int:\n    return helper()\n";
    let results = search_symbols_from_index_with_source_filtered(
        &db_path,
        &helper,
        source,
        "helper_alias",
        10,
        None,
        None,
    )
    .unwrap();

    assert_eq!(results.total_matches, 1);
    assert_eq!(results.matches.len(), 1);
    assert_eq!(results.matches[0].semantic_path, "helper_alias");
    assert!(
        fs::read_to_string(&helper)
            .unwrap()
            .contains("def helper() -> int")
    );
    assert!(
        !fs::read_to_string(&helper)
            .unwrap()
            .contains("helper_alias")
    );
}

#[test]
fn lists_symbols_from_index_with_unsaved_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source =
        "def helper() -> int:\n    return 1\n\n\ndef helper_alias() -> int:\n    return helper()\n";
    let listed =
        list_symbols_from_index_with_source_filtered(&db_path, &helper, source, 10, None, None)
            .unwrap();

    assert_eq!(listed.total_symbols, 2);
    assert_eq!(listed.symbols.len(), 2);
    assert!(
        listed
            .symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );
}

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

#[test]
fn symbol_query_context_rejects_workspace_overlay_outside_workspace() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let outside = dir.join("outside.py");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&outside, "def outside() -> int:\n    return 1\n").unwrap();

    let error = SymbolQueryContext::workspace(&workspace)
        .unwrap()
        .with_source_overlay(&outside, "def outside() -> int:\n    return 2\n")
        .expect_err("workspace contexts should reject overlays outside the workspace");

    assert!(error.to_string().contains("outside workspace"));
}

#[test]
fn rejects_trace_context_file_outside_workspace() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let outside = dir.join("outside.py");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        &outside,
        "def top_level(value: int) -> int:\n    return value\n",
    )
    .unwrap();

    let error = validate_patch_with_trace_context_from_path(
        &workspace,
        &outside,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 1\n",
        None,
        TraceDirection::Both,
    )
    .expect_err("trace context should reject files outside the workspace");

    assert!(error.to_string().contains("outside workspace"));
}
