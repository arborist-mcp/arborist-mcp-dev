use std::fs;

use rusqlite::Connection;

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
    let impact = result.impact.expect("impact should be available");
    assert_eq!(impact.affected_symbol_count, 1);
    assert_eq!(impact.added_callees.len(), 1);
    assert_eq!(impact.added_callees[0].semantic_path, "helper");
    assert!(impact.removed_callees.is_empty());
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
fn traces_cpp_member_calls_from_index_with_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; using Alias = Counter; int local_caller(int value) { Alias current{}; return current.adjust(value); } int postfix_const_caller(int value) { Alias const current{}; return current.adjust(value); } int static_caller(int value) { static Alias current{}; return current.adjust(value); } int auto_caller(int value) { auto current = Alias{}; return current.adjust(value); } int auto_direct_list_caller(int value) { auto current{Alias{}}; return current.adjust(value); } int deduced_pointer_caller(int value) { auto current = new Alias{}; return current->adjust(value); } int parenthesized_deduced_pointer_caller(int value) { auto current = new Alias(); return current->adjust(value); } int default_deduced_pointer_caller(int value) { auto current = new Alias; return current->adjust(value); } int pointee_const_deduced_pointer_caller(int value) { auto current = new const Alias{}; return current->adjust(value); } int postfix_pointee_const_deduced_pointer_caller(int value) { auto current = new Alias const{}; return current->adjust(value); } int make_unique_caller(int value) { auto current = std::make_unique<Alias>(); return current->adjust(value); } int make_shared_caller(int value) { auto current = std::make_shared<Alias>(); return current->adjust(value); } int unique_pointer_caller(int value) { std::unique_ptr<Alias> current; return current->adjust(value); } int shared_pointer_caller(int value) { std::shared_ptr<Alias> current; return current->adjust(value); } int const_unique_pointer_caller(int value) { std::unique_ptr<const Alias> current; return current->adjust(value); } int const_deduced_pointer_caller(int value) { const auto current = new Alias{}; return current->adjust(value); } int auto_pointer_caller(int value) { auto* current = new Alias{}; return current->adjust(value); } int const_auto_pointer_caller(int value) { const auto* current = new Alias{}; return current->adjust(value); } int const_auto_caller(int value) { const auto current = Alias{}; return current.adjust(value); } int parameter_caller(const Alias& current, int value) { return current.adjust(value); } int postfix_const_parameter_caller(Alias const& current, int value) { return current.adjust(value); } int rvalue_reference_caller(Alias&& current, int value) { return current.adjust(value); } int moved_rvalue_reference_caller(Alias&& current, int value) { return std::move(current).adjust(value); } int pointer_caller(Alias* current, int value) { return current->adjust(value); } int const_pointer_caller(Alias* const current, int value) { return current->adjust(value); } int postfix_const_pointer_caller(Alias const* current, int value) { return current->adjust(value); } int pointer_reference_caller(Alias* const& current, int value) { return current->adjust(value); } int dereference_caller(Alias* current, int value) { return (*current).adjust(value); } int range_caller() { for (Alias current : values) { return current.adjust(1); } return 0; } int moved_caller(Alias& current, int value) { return std::move(current).adjust(value); } }\n";

    for (caller, expected_callee) in [
        ("api::local_caller", "api::Counter::adjust(int) &"),
        (
            "api::postfix_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::static_caller", "api::Counter::adjust(int) &"),
        ("api::auto_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_direct_list_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::deduced_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::parenthesized_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::default_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::pointee_const_deduced_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_pointee_const_deduced_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::make_unique_caller", "api::Counter::adjust(int) &"),
        ("api::make_shared_caller", "api::Counter::adjust(int) &"),
        ("api::unique_pointer_caller", "api::Counter::adjust(int) &"),
        ("api::shared_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_unique_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::auto_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_auto_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::parameter_caller", "api::Counter::adjust(int) const &"),
        (
            "api::postfix_const_parameter_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::rvalue_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_rvalue_reference_caller",
            "api::Counter::adjust(int) &&",
        ),
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        ("api::const_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::postfix_const_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::pointer_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::range_caller", "api::Counter::adjust(int) &"),
        ("api::moved_caller", "api::Counter::adjust(int) &&"),
    ] {
        let trace = trace_symbol_graph_from_index_with_source(
            &db_path,
            &source_path,
            source,
            caller,
            TraceDirection::Both,
        )
        .unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
            "{caller}",
        );
    }

    assert!(
        fs::read_to_string(&source_path)
            .unwrap()
            .contains("return value")
    );
}

#[test]
fn traces_cpp_custom_deleter_unique_pointer_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; struct Deleter {}; using Alias = Counter; int caller(int value) { std::unique_ptr<Alias, Deleter> current; return current->adjust(value); } int dereference_caller(int value) { std::unique_ptr<Alias, Deleter> current; return (*std::move(current)).adjust(value); } int get_caller(int value) { std::unique_ptr<const Alias, Deleter> current; return current.get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::get_caller", "api::Counter::adjust(int) const &"),
    ] {
        let trace = trace_symbol_graph_from_index_with_source(
            &db_path,
            &source_path,
            source,
            caller,
            TraceDirection::Both,
        )
        .unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
            "{caller}",
        );
    }
}

#[test]
fn traces_cpp_standard_wrapper_member_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; using Alias = Counter; int optional_value_caller(int value) { std::optional<Alias> current; return current.value().adjust(value); } int auto_unique_caller(int value) { auto current = std::unique_ptr<Alias>{}; return current->adjust(value); } int auto_optional_arrow_caller(int value) { auto current = std::optional<Alias>{}; return current->adjust(value); } int nested_optional_unique_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return (*current)->adjust(value); } int nested_optional_unique_value_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return current.value()->adjust(value); } int ref_factory_caller(int value) { Alias target{}; return std::ref(target).get().adjust(value); } int cref_factory_caller(int value) { Alias target{}; return std::cref(target).get().adjust(value); } int moved_optional_arrow_caller(int value) { std::optional<Alias> current; return std::move(current)->adjust(value); } int optional_dereference_caller(int value) { std::optional<Alias> current; return (*current).adjust(value); } int moved_optional_dereference_caller(int value) { std::optional<Alias> current; return (*std::move(current)).adjust(value); } int const_optional_arrow_caller(int value) { std::optional<Alias> current; return std::as_const(current)->adjust(value); } int const_optional_dereference_caller(int value) { const std::optional<Alias> current{}; return (*current).adjust(value); } int const_reference_wrapper_caller(int value) { const Alias target{}; std::reference_wrapper<const Alias> current(target); return current.get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::optional_value_caller", "api::Counter::adjust(int) &"),
        ("api::auto_unique_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_unique_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_unique_value_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::ref_factory_caller", "api::Counter::adjust(int) &"),
        (
            "api::cref_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_optional_dereference_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::const_optional_arrow_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_optional_dereference_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_reference_wrapper_caller",
            "api::Counter::adjust(int) const &",
        ),
    ] {
        let trace = trace_symbol_graph_from_index_with_source(
            &db_path,
            &source_path,
            source,
            caller,
            TraceDirection::Both,
        )
        .unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
            "{caller}",
        );
    }
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
fn index_overlay_counts_new_unsaved_files_in_indexed_file_totals() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let new_file = dir.join("helper_alias.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(
            &new_file,
            "from helper import helper\n\n\ndef helper_alias() -> int:\n    return helper()\n",
        )
        .unwrap();

    let listed = context.list_symbols(10, None, None).unwrap();

    assert_eq!(listed.indexed_files, 2);
    assert_eq!(listed.total_symbols, 2);
    assert!(
        listed
            .symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );
    assert!(!new_file.exists());
}

#[test]
fn index_overlay_accepts_new_disk_file_when_source_is_overridden() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let new_file = dir.join("helper_alias.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&new_file, "def stale_alias() -> int:\n    return 0\n").unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(
            &new_file,
            "from helper import helper\n\n\ndef helper_alias() -> int:\n    return helper()\n",
        )
        .unwrap();

    let listed = context.list_symbols(10, None, None).unwrap();

    assert_eq!(listed.indexed_files, 2);
    assert!(
        listed
            .symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );
    assert!(
        listed
            .symbols
            .iter()
            .all(|symbol| symbol.semantic_path != "stale_alias")
    );
}

#[test]
fn index_overlay_rejects_inconsistent_indexed_file_counts() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let source = "def helper() -> int:\n    return 1\n";

    fs::write(&helper, source).unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE metadata SET value = '2' WHERE key = 'indexed_files'",
            [],
        )
        .unwrap();
    drop(connection);

    let error = search_symbols_from_index_with_source_filtered(
        &db_path, &helper, source, "helper", 10, None, None,
    )
    .expect_err("source overlays should reject inconsistent persisted file counts");

    assert!(error.to_string().contains("indexed_files metadata 2"));
    assert!(error.to_string().contains("file_state entries 1"));
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
