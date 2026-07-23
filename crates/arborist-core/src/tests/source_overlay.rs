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
fn index_source_overlay_skips_byte_range_validation_against_stale_disk_source() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(
        &caller,
        "from helper import helper\n\n\ndef orchestrate() -> int:\n    return helper()\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    fs::write(&caller, "def stale():\n    return 0\n").unwrap();
    let source = "from helper import helper\n\n\ndef orchestrate() -> int:\n    return helper()\n";
    let trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller,
        source,
        "orchestrate",
        TraceDirection::Both,
    )
    .expect("source overlays must replace stale disk content before range validation");

    assert_eq!(trace.symbol.semantic_path, "orchestrate");
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
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

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; using Alias = Counter; int optional_value_caller(int value) { std::optional<Alias> current; return current.value().adjust(value); } int auto_optional_value_caller(int value) { std::optional<Alias> current; auto current_value = current.value(); return current_value.adjust(value); } int const_auto_optional_value_caller(int value) { std::optional<Alias> current; const auto current_value = current.value(); return current_value.adjust(value); } int copied_const_source_optional_value_caller(int value) { const std::optional<Alias> current{}; auto current_value = current.value(); return current_value.adjust(value); } int auto_optional_dereference_caller(int value) { std::optional<Alias> current; auto current_value = *current; return current_value.adjust(value); } int const_auto_optional_dereference_caller(int value) { std::optional<Alias> current; const auto current_value = *current; return current_value.adjust(value); } int copied_const_source_optional_dereference_caller(int value) { const std::optional<Alias> current{}; auto current_value = *current; return current_value.adjust(value); } int auto_unique_caller(int value) { auto current = std::unique_ptr<Alias>{}; return current->adjust(value); } int auto_reference_alias_caller(int value) { Alias target{}; auto& current = target; return current.adjust(value); } int auto_const_reference_alias_caller(int value) { Alias target{}; const auto& current = target; return current.adjust(value); } int auto_postfix_const_reference_alias_caller(int value) { Alias target{}; auto const& current = target; return current.adjust(value); } int auto_forwarding_reference_alias_caller(int value) { const Alias target{}; auto&& current = target; return current.adjust(value); } int copy_list_caller(int value) { auto current = {Alias{}}; return current.adjust(value); } int auto_optional_arrow_caller(int value) { auto current = std::optional<Alias>{}; return current->adjust(value); } int nested_optional_unique_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return (*current)->adjust(value); } int nested_optional_unique_value_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return current.value()->adjust(value); } int ref_factory_caller(int value) { Alias target{}; return std::ref(target).get().adjust(value); } int parenthesized_ref_factory_caller(int value) { Alias target{}; return (std::ref(target)).get().adjust(value); } int cref_factory_caller(int value) { Alias target{}; return std::cref(target).get().adjust(value); } int ref_as_const_factory_caller(int value) { Alias target{}; return std::ref(std::as_const(target)).get().adjust(value); } int auto_ref_factory_caller(int value) { Alias target{}; auto current = std::ref(target); return current.get().adjust(value); } int auto_cref_factory_caller(int value) { Alias target{}; auto current = std::cref(target); return current.get().adjust(value); } int auto_ref_as_const_factory_caller(int value) { Alias target{}; auto current = std::ref(std::as_const(target)); return current.get().adjust(value); } int moved_optional_arrow_caller(int value) { std::optional<Alias> current; return std::move(current)->adjust(value); } int optional_dereference_caller(int value) { std::optional<Alias> current; return (*current).adjust(value); } int moved_optional_dereference_caller(int value) { std::optional<Alias> current; return (*std::move(current)).adjust(value); } int const_optional_arrow_caller(int value) { std::optional<Alias> current; return std::as_const(current)->adjust(value); } int const_optional_dereference_caller(int value) { const std::optional<Alias> current{}; return (*current).adjust(value); } int const_reference_wrapper_caller(int value) { const Alias target{}; std::reference_wrapper<const Alias> current(target); return current.get().adjust(value); } int auto_parenthesized_reference_wrapper_caller(int value) { Alias target{}; auto current = (std::reference_wrapper<Alias>(target)); return current.get().adjust(value); } int auto_addressof_caller(int value) { Alias current{}; auto pointer = std::addressof(current); return pointer->adjust(value); } int auto_const_addressof_caller(int value) { const Alias current{}; auto pointer = std::addressof(current); return pointer->adjust(value); } int auto_native_addressof_caller(int value) { Alias current{}; auto pointer = &current; return pointer->adjust(value); } int auto_const_native_addressof_caller(int value) { const Alias current{}; auto pointer = &current; return pointer->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::optional_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_optional_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_optional_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_optional_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_optional_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_optional_dereference_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_optional_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::auto_unique_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_reference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_reference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_postfix_const_reference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_forwarding_reference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
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
            "api::parenthesized_ref_factory_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::cref_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::ref_as_const_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_ref_factory_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_cref_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_ref_as_const_factory_caller",
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
        (
            "api::auto_parenthesized_reference_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::auto_addressof_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_const_addressof_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_native_addressof_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_native_addressof_caller",
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
    assert!(
        trace_symbol_graph_from_index_with_source(
            &db_path,
            &source_path,
            source,
            "api::copy_list_caller",
            TraceDirection::Both,
        )
        .unwrap()
        .callees
        .is_empty()
    );
}

#[test]
fn traces_cpp_expected_optional_error_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int arrow_caller(std::expected<Value, std::optional<Counter>> current, int value) { return current.error()->adjust(value); } int moved_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { return std::move(current).error().value().adjust(value); } int const_dereference_caller(const std::expected<Value, std::optional<Counter>> current, int value) { return (*current.error()).adjust(value); } int auto_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = current.error().value(); return error_value.adjust(value); } int const_auto_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { const auto error_value = current.error().value(); return error_value.adjust(value); } int copied_const_source_value_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = current.error().value(); return error_value.adjust(value); } int auto_pointer_value_caller(std::expected<Value, std::optional<std::shared_ptr<Counter>>> current, int value) { auto error_value = current.error().value(); return error_value->adjust(value); } int auto_dereference_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = *current.error(); return error_value.adjust(value); } int const_auto_dereference_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { const auto error_value = *current.error(); return error_value.adjust(value); } int copied_const_source_dereference_value_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = *current.error(); return error_value.adjust(value); } int value_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error_value = current.error().value(); return error_value.adjust(value); } int decltype_value_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { decltype(auto) error_value = current.error().value(); return error_value.adjust(value); } int const_value_alias_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto&& error_value = current.error().value(); return error_value.adjust(value); } int dereference_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error_value = *current.error(); return error_value.adjust(value); } int decltype_dereference_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { decltype(auto) error_value = *current.error(); return error_value.adjust(value); } int const_dereference_alias_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto&& error_value = *current.error(); return error_value.adjust(value); } int alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error = current.error(); return error->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::arrow_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &&"),
        (
            "api::const_dereference_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::auto_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_pointer_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_dereference_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_dereference_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_dereference_value_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_value_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_value_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_nested_standard_value_access_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int auto_value_caller(std::expected<Counter, int> current, int value) { auto current_value = current.value(); return current_value.adjust(value); } int const_auto_value_caller(std::expected<Counter, int> current, int value) { const auto current_value = current.value(); return current_value.adjust(value); } int copied_const_source_value_caller(const std::expected<Counter, int> current, int value) { auto current_value = current.value(); return current_value.adjust(value); } int nested_expected_value_caller(std::expected<std::expected<Counter, int>, int> current, int value) { return current.value().value().adjust(value); } int const_nested_expected_value_caller(const std::expected<std::expected<Counter, int>, int> current, int value) { return current.value().value().adjust(value); } int moved_nested_expected_value_caller(std::expected<std::expected<Counter, int>, int> current, int value) { return std::move(current).value().value().adjust(value); } int auto_nested_expected_value_caller(std::expected<std::expected<Counter, int>, int> current, int value) { auto current_value = current.value(); return current_value.value().adjust(value); } int auto_nested_expected_error_caller(std::expected<std::expected<int, Counter>, int> current, int value) { auto current_value = current.value(); return current_value.error().adjust(value); } int const_auto_nested_expected_error_caller(std::expected<std::expected<int, Counter>, int> current, int value) { const auto current_value = current.value(); return current_value.error().adjust(value); } int nested_optional_value_caller(std::expected<std::optional<Counter>, int> current, int value) { return current.value().value().adjust(value); } int const_nested_optional_value_caller(const std::expected<std::optional<Counter>, int> current, int value) { return current.value().value().adjust(value); } int moved_nested_optional_value_caller(std::expected<std::optional<Counter>, int> current, int value) { return std::move(current).value().value().adjust(value); } int auto_optional_value_caller(std::expected<std::optional<Counter>, int> current, int value) { auto current_value = current.value(); return current_value->adjust(value); } int const_auto_optional_value_caller(std::expected<std::optional<Counter>, int> current, int value) { const auto current_value = current.value(); return current_value->adjust(value); } int auto_pointer_value_caller(std::expected<std::shared_ptr<Counter>, int> current, int value) { auto current_value = current.value(); return current_value->adjust(value); } int get_copy_caller(std::expected<std::unique_ptr<Counter>, int> current, int value) { auto pointer = current.value().get(); return pointer->adjust(value); } int const_get_copy_caller(std::expected<std::shared_ptr<const Counter>, int> current, int value) { auto pointer = current.value().get(); return pointer->adjust(value); } int dereference_copy_caller(std::expected<std::unique_ptr<Counter>, int> current, int value) { auto target = *current.value(); return target.adjust(value); } int const_dereference_copy_caller(std::expected<std::shared_ptr<const Counter>, int> current, int value) { auto target = *current.value(); return target.adjust(value); } int dereference_alias_caller(std::expected<std::unique_ptr<Counter>, int> current, int value) { auto& target = *current.value(); return target.adjust(value); } int const_dereference_alias_caller(const std::expected<std::shared_ptr<Counter>, int> current, int value) { auto&& target = *current.value(); return target.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::auto_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_expected_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_nested_expected_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_nested_expected_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::auto_nested_expected_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_nested_expected_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_nested_expected_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::nested_optional_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_nested_optional_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_nested_optional_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::auto_optional_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_optional_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_pointer_value_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_get_copy_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::dereference_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_alias_caller",
            "api::Counter::adjust(int) &",
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
fn traces_cpp_expected_optional_smart_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected_optional_sp.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int error_value_get_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return current.error().value().get()->adjust(value); } int error_dereference_get_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return (*current.error()).get()->adjust(value); } int value_value_get_caller(std::expected<std::optional<std::shared_ptr<Counter>>, Value> current, int value) { return current.value().value().get()->adjust(value); } int value_dereference_get_caller(std::expected<std::optional<std::shared_ptr<Counter>>, Value> current, int value) { return (*current.value()).get()->adjust(value); } int error_value_arrow_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return current.error().value()->adjust(value); } int error_dereference_arrow_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return (*current.error())->adjust(value); } int const_error_pointee_caller(std::expected<Value, std::optional<std::shared_ptr<const Counter>>> current, int value) { return (*current.error()).get()->adjust(value); } int get_copy_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { auto pointer = current.error().value().get(); return pointer->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::error_value_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::error_dereference_get_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_value_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::value_dereference_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::error_value_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::error_dereference_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_error_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_optional_expected_nested_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("optional_expected_nested.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return current.value().value().adjust(value); } int dereference_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return (*current).value().adjust(value); } int value_error_caller(std::optional<std::expected<Value, Counter>> current, int value) { return current.value().error().adjust(value); } int arrow_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return current->value().adjust(value); } int smart_pointer_value_get_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return current.value().value().get()->adjust(value); } int smart_pointer_arrow_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return (*current).value()->adjust(value); } int nested_optional_value_arrow_caller(std::optional<std::expected<std::optional<Counter>, Value>> current, int value) { return (*current).value()->adjust(value); } int nested_optional_value_value_caller(std::optional<std::expected<std::optional<Counter>, Value>> current, int value) { return current.value().value().value().adjust(value); } int const_value_value_caller(const std::optional<std::expected<Counter, Value>> current, int value) { return current.value().value().adjust(value); } int const_arrow_error_caller(const std::optional<std::expected<Value, Counter>> current, int value) { return current->error().adjust(value); } int arrow_error_smart_pointer_get_caller(std::optional<std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return current->error().get()->adjust(value); } int arrow_error_smart_pointer_arrow_caller(std::optional<std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return current->error()->adjust(value); } int arrow_error_reference_wrapper_caller(std::optional<std::expected<Value, std::reference_wrapper<Counter>>> current, int value) { return current->error().get().adjust(value); } int arrow_error_weak_pointer_caller(std::optional<std::expected<Value, std::weak_ptr<Counter>>> current, int value) { return current->error().lock()->adjust(value); } int auto_arrow_error_caller(std::optional<std::expected<Value, Counter>> current, int value) { auto nested = current->error(); return nested.adjust(value); } int auto_const_arrow_error_caller(const std::optional<std::expected<Value, Counter>> current, int value) { auto nested = current->error(); return nested.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::dereference_value_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_error_caller", "api::Counter::adjust(int) &"),
        ("api::arrow_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::smart_pointer_value_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::smart_pointer_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_value_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_value_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_value_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_arrow_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::arrow_error_smart_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::arrow_error_smart_pointer_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::arrow_error_reference_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::arrow_error_weak_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_arrow_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_arrow_error_caller",
            "api::Counter::adjust(int) &",
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
fn traces_cpp_get_if_pointer_bindings_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("get_if_pointer_bindings.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int auto_get_if_caller(std::variant<Counter, Value> current, int value) { auto nested = std::get_if<Counter>(&current); return nested->adjust(value); } int decltype_auto_get_if_caller(std::variant<Counter, Value> current, int value) { decltype(auto) nested = std::get_if<Counter>(&current); return nested->adjust(value); } int auto_const_get_if_caller(const std::variant<Counter, Value> current, int value) { auto nested = std::get_if<const Counter>(&current); return nested->adjust(value); } int auto_dynamic_pointer_cast_caller(std::shared_ptr<Value> current, int value) { auto nested = std::dynamic_pointer_cast<Counter>(current); return nested->adjust(value); } int decltype_auto_dynamic_pointer_cast_caller(std::shared_ptr<Value> current, int value) { decltype(auto) nested = std::dynamic_pointer_cast<Counter>(current); return nested->adjust(value); } int auto_static_pointer_cast_caller(std::shared_ptr<Value> current, int value) { auto nested = std::static_pointer_cast<Counter>(current); return nested->adjust(value); } int auto_const_pointer_cast_caller(std::shared_ptr<const Counter> current, int value) { auto nested = std::const_pointer_cast<Counter>(current); return nested->adjust(value); } int auto_any_cast_pointer_caller(std::any current, int value) { auto nested = std::any_cast<Counter>(&current); return nested->adjust(value); } int auto_any_cast_value_caller(std::any current, int value) { auto nested = std::any_cast<Counter>(current); return nested.adjust(value); } int decltype_auto_any_cast_value_caller(std::any current, int value) { decltype(auto) nested = std::any_cast<Counter>(current); return nested.adjust(value); } int auto_variant_get_caller(std::variant<Counter, Value> current, int value) { auto nested = std::get<Counter>(current); return nested.adjust(value); } int decltype_auto_variant_get_caller(std::variant<Counter, Value> current, int value) { decltype(auto) nested = std::get<Counter>(current); return nested.adjust(value); } int auto_get_if_then_member_caller(std::variant<std::unique_ptr<Counter>, Value> current, int value) { auto nested = std::get_if<std::unique_ptr<Counter>>(&current); return (*nested)->adjust(value); } int decltype_auto_get_if_unique_caller(std::variant<std::unique_ptr<Counter>, Value> current, int value) { decltype(auto) nested = std::get_if<std::unique_ptr<Counter>>(&current); return (*nested)->adjust(value); } int direct_to_address_raw_caller(Counter* current, int value) { return std::to_address(current)->adjust(value); } int auto_to_address_raw_caller(Counter* current, int value) { auto nested = std::to_address(current); return nested->adjust(value); } int decltype_auto_to_address_smart_caller(std::unique_ptr<Counter> current, int value) { decltype(auto) nested = std::to_address(current); return nested->adjust(value); } int auto_to_address_const_smart_caller(std::unique_ptr<const Counter> current, int value) { auto nested = std::to_address(current); return nested->adjust(value); } int vector_front_caller(std::vector<Counter> current, int value) { return current.front().adjust(value); } int vector_back_caller(std::vector<Counter> current, int value) { return current.back().adjust(value); } int array_at_caller(std::array<Counter, 2> current, int value) { return current.at(0).adjust(value); } int span_const_front_caller(std::span<const Counter> current, int value) { return current.front().adjust(value); } int const_vector_back_caller(const std::vector<Counter> current, int value) { return current.back().adjust(value); } int auto_tuple_get_caller(std::tuple<Value, Counter> current, int value) { auto nested = std::get<1>(current); return nested.adjust(value); } int decltype_auto_tuple_get_caller(std::tuple<Value, Counter> current, int value) { decltype(auto) nested = std::get<1>(current); return nested.adjust(value); } int auto_const_pair_get_caller(const std::pair<Counter, Value> current, int value) { auto nested = std::get<0>(current); return nested.adjust(value); } int decltype_auto_const_pair_get_caller(const std::pair<Counter, Value> current, int value) { decltype(auto) nested = std::get<0>(current); return nested.adjust(value); } int auto_tuple_get_unique_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { auto nested = std::get<1>(current); return nested->adjust(value); } int decltype_auto_tuple_get_unique_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { decltype(auto) nested = std::get<1>(current); return nested->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::auto_get_if_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_auto_get_if_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_get_if_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_dynamic_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_dynamic_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_static_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_any_cast_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_any_cast_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_any_cast_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_get_if_then_member_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_get_if_unique_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::direct_to_address_raw_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_to_address_raw_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_to_address_smart_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_to_address_const_smart_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::vector_front_caller", "api::Counter::adjust(int) &"),
        ("api::vector_back_caller", "api::Counter::adjust(int) &"),
        ("api::array_at_caller", "api::Counter::adjust(int) &"),
        (
            "api::span_const_front_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_vector_back_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::auto_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_auto_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_pair_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_const_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_tuple_get_unique_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_tuple_get_unique_caller",
            "api::Counter::adjust(int) &",
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
fn traces_cpp_indexable_sequence_element_member_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexable_sequence_elements.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int vector_index_caller(std::vector<Counter> current, int value) { return current[0].adjust(value); } int vector_nested_index_caller(std::vector<Counter> current, std::array<int, 1> indexes, int value) { return current[indexes[0]].adjust(value); } int span_index_caller(std::span<const Counter> current, int value) { return current[0].adjust(value); } int array_index_caller(std::array<Counter, 2> current, int value) { return current[1].adjust(value); } int const_deque_index_caller(const std::deque<Counter> current, int value) { return current[0].adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::vector_index_caller", "api::Counter::adjust(int) &"),
        (
            "api::vector_nested_index_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::span_index_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::array_index_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_deque_index_caller",
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
fn traces_cpp_wrapped_sequence_receiver_categories_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_sequence_receiver_categories.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int moved_front_caller(std::vector<Counter> current, int value) { return std::move(current).front().adjust(value); } int const_back_caller(std::vector<Counter> current, int value) { return std::as_const(current).back().adjust(value); } int forwarded_subscript_caller(std::array<Counter, 2> current, int value) { return std::forward<std::array<Counter, 2>&&>(current)[0].adjust(value); } int moved_data_caller(std::span<Counter> current, int value) { return std::move(current).data()->adjust(value); } int const_data_caller(std::vector<Counter> current, int value) { return std::as_const(current).data()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::moved_front_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_back_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarded_subscript_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::moved_data_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_data_caller",
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
fn traces_cpp_wrapped_weak_pointer_lock_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_weak_pointer_lock.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int moved_caller(std::weak_ptr<Counter> current, int value) { return std::move(current).lock()->adjust(value); } int const_caller(std::weak_ptr<Counter> current, int value) { return std::as_const(current).lock()->adjust(value); } int forwarded_caller(std::weak_ptr<Counter> current, int value) { return std::forward<std::weak_ptr<Counter>&&>(current).lock()->adjust(value); } int const_pointee_caller(std::weak_ptr<const Counter> current, int value) { return std::move(current).lock()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
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
fn traces_cpp_wrapped_reference_wrapper_get_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_reference_wrapper_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int moved_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::move(current).get().adjust(value); } int const_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::as_const(current).get().adjust(value); } int forwarded_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::forward<std::reference_wrapper<Counter>&&>(current).get().adjust(value); } int const_pointee_caller(const Counter& target, int value) { std::reference_wrapper<const Counter> current(target); return std::move(current).get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
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
fn traces_cpp_wrapped_smart_pointer_get_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int moved_caller(std::shared_ptr<Counter> current, int value) { return std::move(current).get()->adjust(value); } int const_caller(std::shared_ptr<Counter> current, int value) { return std::as_const(current).get()->adjust(value); } int forwarded_caller(std::shared_ptr<Counter> current, int value) { return std::forward<std::shared_ptr<Counter>&&>(current).get()->adjust(value); } int const_pointee_caller(std::shared_ptr<const Counter> current, int value) { return std::move(current).get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
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
fn traces_cpp_direct_standard_pointer_cast_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("direct_standard_pointer_cast.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int get_if_caller(std::variant<Counter, Value> current, int value) { return std::get_if<Counter>(&current)->adjust(value); } int const_get_if_caller(std::variant<Counter, Value> current, int value) { return std::get_if<Counter>(std::addressof(std::as_const(current)))->adjust(value); } int any_cast_caller(std::any current, int value) { return std::any_cast<Counter>(&current)->adjust(value); } int const_any_cast_caller(std::any current, int value) { return std::any_cast<Counter>(std::addressof(std::as_const(current)))->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::get_if_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_get_if_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::any_cast_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_any_cast_caller",
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
fn traces_cpp_wrapped_indexed_get_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_wrappers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int moved_weak_caller(std::tuple<Value, std::weak_ptr<Counter>> current, int value) { return std::get<1>(std::move(current)).lock()->adjust(value); } int const_weak_caller(std::tuple<Value, std::weak_ptr<const Counter>> current, int value) { return std::get<1>(std::as_const(current)).lock()->adjust(value); } int forwarded_reference_caller(std::tuple<Value, std::reference_wrapper<Counter>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::reference_wrapper<Counter>>&&>(current)).get().adjust(value); } int const_reference_caller(std::tuple<Value, std::reference_wrapper<const Counter>> current, int value) { return std::get<1>(std::as_const(current)).get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::moved_weak_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_weak_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarded_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_reference_caller",
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
fn traces_cpp_wrapped_indexed_get_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int moved_smart_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int const_smart_caller(std::tuple<Value, std::shared_ptr<const Counter>> current, int value) { return std::get<1>(std::as_const(current))->adjust(value); } int forwarded_raw_caller(std::tuple<Value, Counter*> current, int value) { return std::get<1>(std::forward<std::tuple<Value, Counter*>&&>(current))->adjust(value); } int const_raw_caller(std::tuple<Value, const Counter*> current, int value) { return std::get<1>(std::as_const(current))->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::moved_smart_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_smart_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::forwarded_raw_caller", "api::Counter::adjust(int) &"),
        ("api::const_raw_caller", "api::Counter::adjust(int) const &"),
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
fn traces_cpp_wrapped_indexed_get_expected_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_expected_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int value_caller(std::tuple<Value, std::expected<std::shared_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::shared_ptr<Counter>, Value>>&&>(current)).value()->adjust(value); } int error_caller(std::tuple<Value, std::expected<Value, std::shared_ptr<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::error_caller", "api::Counter::adjust(int) const &"),
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
fn traces_cpp_wrapped_indexed_get_expected_raw_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_expected_raw_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_caller(std::tuple<Value, std::expected<Counter*, Value>> current, int value) { return std::get<1>(std::move(current)).value()->adjust(value); } int error_caller(std::tuple<Value, std::expected<Value, Counter*>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<Value, Counter*>>&&>(current)).error()->adjust(value); } int const_value_caller(std::tuple<Value, std::expected<const Counter*, Value>> current, int value) { return std::get<1>(std::as_const(current)).value()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::error_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_caller",
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
fn traces_cpp_wrapped_indexed_get_expected_optional_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_expected_optional_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int smart_value_caller(std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>> current, int value) { return std::get<1>(std::move(current)).value()->adjust(value); } int smart_error_caller(std::tuple<Value, std::expected<Value, std::optional<std::shared_ptr<const Counter>>>> current, int value) { return std::get<1>(std::as_const(current)).error()->adjust(value); } int raw_value_caller(std::tuple<Value, std::expected<std::optional<Counter*>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::optional<Counter*>, Value>>&&>(current)).value()->adjust(value); } int raw_error_caller(std::tuple<Value, std::expected<Value, std::optional<const Counter*>>> current, int value) { return std::get<1>(std::as_const(current)).error()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::smart_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::smart_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::raw_value_caller", "api::Counter::adjust(int) &"),
        ("api::raw_error_caller", "api::Counter::adjust(int) const &"),
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
fn traces_cpp_wrapped_indexed_get_expected_optional_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_expected_optional_wrappers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int weak_value_caller(std::tuple<Value, std::expected<std::optional<std::weak_ptr<Counter>>, Value>> current, int value) { return std::get<1>(std::move(current)).value()->lock()->adjust(value); } int reference_error_caller(std::tuple<Value, std::expected<Value, std::optional<std::reference_wrapper<const Counter>>>> current, int value) { return std::get<1>(std::as_const(current)).error()->get().adjust(value); } int smart_value_caller(std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>>&&>(current)).value()->get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::weak_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::reference_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::smart_value_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_wrapped_indexed_get_expected_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_expected_wrappers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int weak_value_caller(std::tuple<Value, std::expected<std::weak_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::move(current)).value().lock()->adjust(value); } int weak_error_caller(std::tuple<Value, std::expected<Value, std::weak_ptr<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error().lock()->adjust(value); } int reference_value_caller(std::tuple<Value, std::expected<std::reference_wrapper<Counter>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::reference_wrapper<Counter>, Value>>&&>(current)).value().get().adjust(value); } int reference_error_caller(std::tuple<Value, std::expected<Value, std::reference_wrapper<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error().get().adjust(value); } int smart_value_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::move(current)).value().get()->adjust(value); } int smart_error_get_caller(std::tuple<Value, std::expected<Value, std::shared_ptr<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error().get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::weak_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::weak_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::reference_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::reference_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::smart_value_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::smart_error_get_caller",
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
fn traces_cpp_contiguous_sequence_data_member_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("contiguous_sequence_data.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int inline_data_caller(std::vector<Counter> current, int value) { return current.data()->adjust(value); } int auto_data_caller(std::array<Counter, 2> current, int value) { auto pointer = current.data(); return pointer->adjust(value); } int decltype_auto_data_caller(std::vector<Counter> current, int value) { decltype(auto) pointer = current.data(); return pointer->adjust(value); } int const_span_data_caller(std::span<const Counter> current, int value) { auto pointer = current.data(); return pointer->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::inline_data_caller", "api::Counter::adjust(int) &"),
        ("api::auto_data_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_auto_data_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_span_data_caller",
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
fn traces_cpp_direct_indexed_tuple_get_member_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("direct_indexed_tuple_get.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_tuple_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(current).adjust(value); } int direct_const_pair_get_caller(const std::pair<Counter, Value> current, int value) { return std::get<0>(current).adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::direct_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::direct_const_pair_get_caller",
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
fn traces_cpp_indexed_get_receiver_categories_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_get_receiver_categories.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } int adjust(int value) const && { return value + 3; } }; int moved_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(std::move(current)).adjust(value); } int const_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(std::as_const(current)).adjust(value); } int forwarded_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(std::forward<std::tuple<Value, Counter>&&>(current)).adjust(value); } int decltype_auto_get_caller(std::tuple<Value, Counter> current, int value) { decltype(auto) nested = std::get<1>(std::move(current)); return nested.adjust(value); } int decltype_auto_moved_get_caller(std::tuple<Value, Counter> current, int value) { decltype(auto) nested = std::get<1>(std::move(current)); return std::move(nested).adjust(value); } int moved_optional_value_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(std::move(current)).value().adjust(value); } int moved_optional_arrow_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int moved_expected_value_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(std::move(current)).value().adjust(value); } int moved_expected_arrow_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int moved_expected_error_caller(std::tuple<Value, std::expected<Value, Counter>> current, int value) { return std::get<1>(std::move(current)).error().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::moved_get_caller", "api::Counter::adjust(int) &&"),
        ("api::const_get_caller", "api::Counter::adjust(int) const &"),
        ("api::forwarded_get_caller", "api::Counter::adjust(int) &&"),
        (
            "api::decltype_auto_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_moved_get_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_expected_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_expected_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_expected_error_caller",
            "api::Counter::adjust(int) &&",
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
fn traces_cpp_direct_indexed_tuple_get_smart_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("direct_indexed_tuple_get_smart_pointer.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } int const_shared_pair_get_caller(std::pair<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<0>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_indexed_tuple_get_smart_pointer_get_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(current).get()->adjust(value); } int const_shared_pair_get_caller(std::pair<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<0>(current).get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
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
            "{caller}"
        );
    }
}

#[test]
fn traces_cpp_direct_indexed_variant_get_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("direct_indexed_variant_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_variant_get_caller(std::variant<Value, Counter> current, int value) { return std::get<1>(current).adjust(value); } int const_variant_get_caller(const std::variant<Counter, Value> current, int value) { return std::get<0>(current).adjust(value); } int direct_typed_variant_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(current).adjust(value); } int const_typed_variant_get_caller(const std::variant<Counter, Value> current, int value) { return std::get<Counter>(current).adjust(value); } int typed_tuple_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<Counter>(current).adjust(value); } int typed_unique_variant_get_caller(std::variant<Value, std::unique_ptr<Counter>> current, int value) { return std::get<std::unique_ptr<Counter>>(current)->adjust(value); } int typed_const_shared_variant_get_caller(std::variant<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<std::shared_ptr<const Counter>>(current)->adjust(value); } int typed_raw_pointer_variant_get_caller(std::variant<Value, Counter*> current, int value) { return std::get<Counter*>(current)->adjust(value); } int typed_const_reference_variant_get_caller(std::variant<std::reference_wrapper<const Counter>, Value> current, int value) { return std::get<std::reference_wrapper<const Counter>>(current).get().adjust(value); } int typed_weak_pointer_variant_get_caller(std::variant<Value, std::weak_ptr<Counter>> current, int value) { return std::get<std::weak_ptr<Counter>>(current).lock()->adjust(value); } int typed_optional_variant_get_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(current)->adjust(value); } int typed_const_expected_variant_get_caller(const std::variant<std::expected<Counter, Value>, Value> current, int value) { return std::get<std::expected<Counter, Value>>(current)->adjust(value); } int invalid_missing_typed_variant_get_caller(std::variant<Value, Counter> current, int value) { return std::get<std::unique_ptr<Counter>>(current)->adjust(value); } int invalid_duplicate_typed_tuple_get_caller(std::tuple<Counter, Counter> current, int value) { return std::get<Counter>(current).adjust(value); } int auto_variant_get_caller(std::variant<Value, Counter> current, int value) { auto nested = std::get<1>(current); return nested.adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::direct_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::direct_typed_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_typed_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::typed_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::typed_unique_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_const_shared_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_raw_pointer_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_const_reference_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_weak_pointer_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_optional_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_const_expected_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_variant_get_caller",
            "api::Counter::adjust(int) &",
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
    for caller in [
        "api::invalid_missing_typed_variant_get_caller",
        "api::invalid_duplicate_typed_tuple_get_caller",
    ] {
        let trace = trace_symbol_graph_from_index_with_source(
            &db_path,
            &source_path,
            source,
            caller,
            TraceDirection::Both,
        )
        .unwrap();
        assert!(trace.callees.is_empty(), "{caller}");
    }
}

#[test]
fn traces_cpp_typed_get_standard_value_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("typed_get_standard_value.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } int adjust(int value) const && { return value + 3; } }; int optional_value_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(current).value().adjust(value); } int expected_value_caller(std::variant<Value, std::expected<Counter, Value>> current, int value) { return std::get<std::expected<Counter, Value>>(current).value().adjust(value); } int const_expected_error_caller(const std::variant<Value, std::expected<Value, Counter>> current, int value) { return std::get<std::expected<Value, Counter>>(current).error().adjust(value); } int moved_typed_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(std::move(current)).adjust(value); } int const_typed_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(std::as_const(current)).adjust(value); } int forwarded_typed_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(std::forward<std::variant<Value, Counter>&&>(current)).adjust(value); } int moved_optional_value_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(std::move(current)).value().adjust(value); } int moved_expected_error_caller(std::variant<Value, std::expected<Value, Counter>> current, int value) { return std::get<std::expected<Value, Counter>>(std::move(current)).error().adjust(value); } int moved_optional_arrow_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(std::move(current))->adjust(value); } int moved_expected_arrow_caller(std::variant<Value, std::expected<Counter, Value>> current, int value) { return std::get<std::expected<Counter, Value>>(std::move(current))->adjust(value); } int optional_unique_caller(std::variant<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<std::optional<std::unique_ptr<Counter>>>(current)->adjust(value); } int expected_const_shared_caller(std::variant<std::expected<std::shared_ptr<const Counter>, Value>, Value> current, int value) { return std::get<std::expected<std::shared_ptr<const Counter>, Value>>(current)->adjust(value); } int shared_get_caller(std::variant<Value, std::shared_ptr<Counter>> current, int value) { return std::get<std::shared_ptr<Counter>>(current).get()->adjust(value); } int const_shared_get_caller(std::variant<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<std::shared_ptr<const Counter>>(current).get()->adjust(value); } int moved_expected_value_get_caller(std::variant<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<std::expected<std::unique_ptr<Counter>, Value>>(std::move(current)).value().get()->adjust(value); } int const_expected_error_get_caller(std::variant<Value, std::expected<Value, std::shared_ptr<const Counter>>> current, int value) { return std::get<std::expected<Value, std::shared_ptr<const Counter>>>(std::as_const(current)).error().get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::optional_value_caller", "api::Counter::adjust(int) &"),
        ("api::expected_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_expected_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_typed_get_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::const_typed_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarded_typed_get_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_expected_error_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_expected_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::optional_unique_caller", "api::Counter::adjust(int) &"),
        (
            "api::expected_const_shared_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::shared_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_expected_value_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_expected_error_get_caller",
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
fn traces_cpp_typed_get_top_level_cv_spellings_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("typed_get_top_level_cv.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int postfix_const_caller(std::variant<Value, Counter const> current, int value) { return std::get<const Counter>(current).adjust(value); } int postfix_volatile_caller(std::variant<Value, volatile Counter> current, int value) { return std::get<Counter volatile>(current).adjust(value); } int get_if_postfix_const_caller(std::variant<Value, Counter const> current, int value) { return std::get_if<const Counter>(&current)->adjust(value); } int get_if_postfix_volatile_caller(std::variant<Value, volatile Counter> current, int value) { return std::get_if<Counter volatile>(&current)->adjust(value); } int pointer_const_caller(std::variant<Value, const Counter*> current, int value) { return std::get<Counter const*>(current)->adjust(value); } int const_pointer_caller(std::variant<Value, Counter* const> current, int value) { return std::get<Counter* const>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::postfix_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_volatile_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::get_if_postfix_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::get_if_postfix_volatile_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::pointer_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointer_caller",
            "api::Counter::adjust(int) &",
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
fn traces_cpp_typed_get_expected_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("typed_get_expected_wrappers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int weak_value_caller(std::variant<Value, std::expected<std::weak_ptr<Counter>, Value>> current, int value) { return std::get<std::expected<std::weak_ptr<Counter>, Value>>(std::move(current)).value().lock()->adjust(value); } int weak_error_caller(std::variant<Value, std::expected<Value, std::weak_ptr<const Counter>>> current, int value) { return std::get<std::expected<Value, std::weak_ptr<const Counter>>>(std::as_const(current)).error().lock()->adjust(value); } int reference_value_caller(std::variant<Value, std::expected<std::reference_wrapper<Counter>, Value>> current, int value) { return std::get<std::expected<std::reference_wrapper<Counter>, Value>>(std::forward<std::variant<Value, std::expected<std::reference_wrapper<Counter>, Value>>&&>(current)).value().get().adjust(value); } int reference_error_caller(std::variant<Value, std::expected<Value, std::reference_wrapper<const Counter>>> current, int value) { return std::get<std::expected<Value, std::reference_wrapper<const Counter>>>(std::as_const(current)).error().get().adjust(value); } int optional_weak_value_caller(std::variant<Value, std::expected<std::optional<std::weak_ptr<Counter>>, Value>> current, int value) { return std::get<std::expected<std::optional<std::weak_ptr<Counter>>, Value>>(std::move(current)).value()->lock()->adjust(value); } int optional_reference_error_caller(std::variant<Value, std::expected<Value, std::optional<std::reference_wrapper<const Counter>>>> current, int value) { return std::get<std::expected<Value, std::optional<std::reference_wrapper<const Counter>>>>(std::as_const(current)).error()->get().adjust(value); } int optional_smart_value_caller(std::variant<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>> current, int value) { return std::get<std::expected<std::optional<std::unique_ptr<Counter>>, Value>>(std::forward<std::variant<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>>&&>(current)).value()->get()->adjust(value); } int optional_smart_error_caller(std::variant<Value, std::expected<Value, std::optional<std::shared_ptr<const Counter>>>> current, int value) { return std::get<std::expected<Value, std::optional<std::shared_ptr<const Counter>>>>(std::as_const(current)).error()->adjust(value); } int moved_sequence_value_caller(std::variant<Value, std::expected<std::vector<Counter>, Value>> current, int value) { return std::get<std::expected<std::vector<Counter>, Value>>(std::move(current)).value().front().adjust(value); } int const_sequence_error_caller(std::variant<Value, std::expected<Value, std::deque<Counter>>> current, int value) { return std::get<std::expected<Value, std::deque<Counter>>>(std::as_const(current)).error().at(0).adjust(value); } int sequence_value_data_caller(std::variant<Value, std::expected<std::span<Counter>, Value>> current, int value) { return std::get<std::expected<std::span<Counter>, Value>>(current).value().data()->adjust(value); } int const_sequence_error_data_caller(std::variant<Value, std::expected<Value, std::array<Counter, 2>>> current, int value) { return std::get<std::expected<Value, std::array<Counter, 2>>>(std::as_const(current)).error().data()->adjust(value); } int auto_sequence_data_caller(std::variant<Value, std::expected<std::vector<Counter>, Value>> current, int value) { auto pointer = std::get<std::expected<std::vector<Counter>, Value>>(current).value().data(); return pointer->adjust(value); } int decltype_auto_const_sequence_data_caller(std::variant<Value, std::expected<Value, std::span<Counter>>> current, int value) { decltype(auto) pointer = std::get<std::expected<Value, std::span<Counter>>>(std::as_const(current)).error().data(); return pointer->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::weak_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::weak_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::reference_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::reference_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::optional_weak_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_reference_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::optional_smart_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_smart_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_sequence_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_sequence_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::sequence_value_data_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_sequence_error_data_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_sequence_data_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_const_sequence_data_caller",
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
fn binds_cpp_typed_get_expected_optional_wrappers_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("typed_get_expected_optional_wrapper_bindings.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int weak_caller(std::variant<Value, std::expected<std::optional<std::weak_ptr<Counter>>, Value>> current, int value) { decltype(auto) nested = std::get<std::expected<std::optional<std::weak_ptr<Counter>>, Value>>(current).value()->lock(); return nested->adjust(value); } int smart_caller(std::variant<Value, std::expected<Value, std::optional<std::shared_ptr<const Counter>>>> current, int value) { auto pointer = std::get<std::expected<Value, std::optional<std::shared_ptr<const Counter>>>>(current).error()->get(); return pointer->adjust(value); } int reference_caller(std::variant<Value, std::expected<std::optional<std::reference_wrapper<const Counter>>, Value>> current, int value) { decltype(auto) nested = std::get<std::expected<std::optional<std::reference_wrapper<const Counter>>, Value>>(current).value()->get(); return nested.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::weak_caller", "api::Counter::adjust(int) &"),
        ("api::smart_caller", "api::Counter::adjust(int) const &"),
        ("api::reference_caller", "api::Counter::adjust(int) const &"),
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
fn preserves_cpp_decltype_auto_typed_get_receiver_categories_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("decltype_auto_typed_get_receiver_categories.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int const_get_caller(const std::variant<Value, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(current); return nested.adjust(value); } int rvalue_reference_get_caller(std::variant<Value, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(std::move(current)); return nested.adjust(value); } int moved_get_caller(std::variant<Value, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(std::move(current)); return std::move(nested).adjust(value); } int optional_get_caller(const std::variant<Value, std::optional<Counter>> current, int value) { decltype(auto) nested = std::get<std::optional<Counter>>(current); return nested.value().adjust(value); } int typed_expected_weak_caller(std::variant<Value, std::expected<std::weak_ptr<Counter>, Value>> current, int value) { decltype(auto) nested = std::get<std::expected<std::weak_ptr<Counter>, Value>>(current).value().lock(); return nested->adjust(value); } int typed_expected_const_reference_caller(std::variant<Value, std::expected<Value, std::reference_wrapper<const Counter>>> current, int value) { decltype(auto) nested = std::get<std::expected<Value, std::reference_wrapper<const Counter>>>(current).error().get(); return nested.adjust(value); } int typed_expected_auto_value_caller(std::variant<Value, std::expected<Counter, Value>> current, int value) { auto nested = std::get<std::expected<Counter, Value>>(current).value(); return nested.adjust(value); } int typed_expected_decltype_auto_error_caller(const std::variant<Value, std::expected<Value, Counter>> current, int value) { decltype(auto) nested = std::get<std::expected<Value, Counter>>(current).error(); return nested.adjust(value); } int typed_expected_auto_optional_value_caller(std::variant<Value, std::expected<std::optional<Counter>, Value>> current, int value) { auto nested = std::get<std::expected<std::optional<Counter>, Value>>(current).value(); return nested->adjust(value); } int typed_expected_decltype_auto_sequence_error_caller(const std::variant<Value, std::expected<Value, std::vector<Counter>>> current, int value) { decltype(auto) nested = std::get<std::expected<Value, std::vector<Counter>>>(current).error().front(); return nested.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::const_get_caller", "api::Counter::adjust(int) const &"),
        (
            "api::rvalue_reference_get_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::moved_get_caller", "api::Counter::adjust(int) &&"),
        (
            "api::optional_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_expected_weak_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_expected_const_reference_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_expected_auto_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_expected_decltype_auto_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_expected_auto_optional_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_expected_decltype_auto_sequence_error_caller",
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
fn does_not_trace_invalid_cpp_typed_get_bindings_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("invalid_typed_get_bindings.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) { return value; } }; int missing_auto_caller(std::variant<Value, Counter> current, int value) { auto nested = std::get<std::unique_ptr<Counter>>(current); return nested->adjust(value); } int duplicate_decltype_auto_caller(std::tuple<Counter, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(current); return nested.adjust(value); } }\n";
    for caller in [
        "api::missing_auto_caller",
        "api::duplicate_decltype_auto_caller",
    ] {
        let trace = trace_symbol_graph_from_index_with_source(
            &db_path,
            &source_path,
            source,
            caller,
            TraceDirection::Both,
        )
        .unwrap();
        assert!(trace.callees.is_empty(), "{caller}");
    }
}

#[test]
fn traces_cpp_indexed_tuple_get_reference_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_reference_wrapper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::reference_wrapper<Counter>> current, int value) { return std::get<1>(current).get().adjust(value); } int const_pair_get_caller(std::pair<std::reference_wrapper<const Counter>, Value> current, int value) { return std::get<0>(current).get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_raw_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_raw_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, Counter*> current, int value) { return std::get<1>(current)->adjust(value); } int const_pair_get_caller(std::pair<const Counter*, Value> current, int value) { return std::get<0>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_optional_value_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_optional_value.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_value_pair_get_caller(std::pair<std::optional<const Counter>, Value> current, int value) { return std::get<0>(current).value().adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_value_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_value_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_value.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<const Counter, Value>, Value> current, int value) { return std::get<0>(current).value().adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_value_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_value_smart_pointer_arrow_calls_from_unsaved_source_overlay()
 {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_value_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int optional_unique_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int const_shared_pair_get_caller(std::pair<std::expected<std::shared_ptr<const Counter>, Value>, Value> current, int value) { return std::get<0>(current).value()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_indexed_tuple_get_expected_smart_pointer_get_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value().get()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::shared_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current).error().get()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value().get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_indexed_tuple_get_expected_raw_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_raw_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<Counter*, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<const Counter*, Value>, Value> current, int value) { return std::get<0>(current).value()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, Counter*>> current, int value) { return std::get<1>(current).error()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, const Counter*>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_optional_raw_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_optional_raw_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<Counter*>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<const Counter*>, Value>, Value> current, int value) { return std::get<0>(current).value()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<Counter*>>> current, int value) { return std::get<1>(current).error()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<const Counter*>>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_error_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_error.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::expected<Value, Counter>> current, int value) { return std::get<1>(current).error().adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Value, Counter>> current, int value) { return std::get<1>(current).error().adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, const Counter>, Value> current, int value) { return std::get<0>(current).error().adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_weak_pointer_lock_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_weak_pointer_lock.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::weak_ptr<Counter>> current, int value) { return std::get<1>(current).lock()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::weak_ptr<Counter>> current, int value) { return std::get<1>(current).lock()->adjust(value); } int const_pointee_pair_get_caller(std::pair<std::weak_ptr<const Counter>, Value> current, int value) { return std::get<0>(current).lock()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_optional_arrow_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_optional_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } int const_pointee_pair_get_caller(std::pair<std::optional<const Counter>, Value> current, int value) { return std::get<0>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointee_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_error_smart_pointer_arrow_calls_from_unsaved_source_overlay()
 {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_error_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current).error()->adjust(value); } int optional_shared_pair_get_caller(std::pair<std::expected<Value, std::optional<std::shared_ptr<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } int const_shared_pair_get_caller(std::pair<std::expected<Value, std::shared_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current).error()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_indexed_tuple_get_expected_weak_pointer_lock_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_weak_pointer_lock.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::weak_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value().lock()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::weak_ptr<const Counter>, Value>, Value> current, int value) { return std::get<0>(current).value().lock()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::weak_ptr<Counter>>> current, int value) { return std::get<1>(current).error().lock()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::weak_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current).error().lock()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_optional_weak_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_optional_weak_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::weak_ptr<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->lock()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<std::weak_ptr<const Counter>>, Value>, Value> current, int value) { return std::get<0>(current).value()->lock()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { return std::get<1>(current).error()->lock()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<std::weak_ptr<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->lock()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_optional_reference_wrapper_calls_from_unsaved_source_overlay()
 {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_optional_reference_wrapper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::reference_wrapper<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->get().adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<std::reference_wrapper<const Counter>>, Value>, Value> current, int value) { return std::get<0>(current).value()->get().adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { return std::get<1>(current).error()->get().adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<std::reference_wrapper<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_optional_smart_pointer_get_calls_from_unsaved_source_overlay()
 {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_optional_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::shared_ptr<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->get()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<std::unique_ptr<const Counter>>, Value>, Value> current, int value) { return std::get<0>(current).value()->get()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<std::shared_ptr<Counter>>>> current, int value) { return std::get<1>(current).error()->get()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<std::unique_ptr<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_sequence_element_access_calls_from_unsaved_source_overlay()
{
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_sequence_element_access.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::vector<Counter>, Value>> current, int value) { return std::get<1>(current).value()[0].adjust(value); } int moved_value_tuple_get_caller(std::tuple<Value, std::expected<std::vector<Counter>, Value>> current, int value) { return std::get<1>(std::move(current)).value().front().adjust(value); } int const_value_pair_get_caller(const std::pair<std::expected<std::vector<Counter>, Value>, Value> current, int value) { return std::get<0>(current).value().front().adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::deque<Counter>>> current, int value) { return std::get<1>(current).error().at(0).adjust(value); } int const_error_pair_get_caller(const std::pair<std::expected<Value, std::list<Counter>>, Value> current, int value) { return std::get<0>(current).error().back().adjust(value); } int value_data_tuple_get_caller(std::tuple<Value, std::expected<std::span<Counter>, Value>> current, int value) { return std::get<1>(current).value().data()->adjust(value); } int const_error_data_pair_get_caller(const std::pair<std::expected<Value, std::array<Counter, 2>>, Value> current, int value) { return std::get<0>(current).error().data()->adjust(value); } int wrapped_const_error_data_pair_get_caller(std::pair<std::expected<Value, std::array<Counter, 2>>, Value> current, int value) { return std::get<0>(std::as_const(current)).error().data()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::moved_value_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::value_data_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_error_data_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::wrapped_const_error_data_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_sequence_data_pointer_bindings_from_unsaved_source_overlay()
 {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_sequence_data_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int auto_value_caller(std::tuple<Value, std::expected<std::vector<Counter>, Value>> current, int value) { auto pointer = std::get<1>(current).value().data(); return pointer->adjust(value); } int decltype_auto_const_error_caller(const std::pair<std::expected<Value, std::span<Counter>>, Value> current, int value) { decltype(auto) pointer = std::get<0>(current).error().data(); return pointer->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::auto_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_auto_const_error_caller",
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
fn traces_cpp_indexed_tuple_get_expected_reference_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_reference_wrapper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::reference_wrapper<Counter>, Value>> current, int value) { return std::get<1>(current).value().get().adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::reference_wrapper<const Counter>, Value>, Value> current, int value) { return std::get<0>(current).value().get().adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::reference_wrapper<Counter>>> current, int value) { return std::get<1>(current).error().get().adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::reference_wrapper<const Counter>>, Value> current, int value) { return std::get<0>(current).error().get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_arrow_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current)->adjust(value); } int const_pointee_pair_get_caller(std::pair<std::expected<const Counter, Value>, Value> current, int value) { return std::get<0>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointee_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_optional_smart_pointer_arrow_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_optional_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current)->adjust(value); } int const_shared_pair_get_caller(std::pair<std::optional<std::shared_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current)->adjust(value); } int moved_tuple_get_caller(std::tuple<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int forwarded_tuple_get_caller(std::tuple<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::optional<std::unique_ptr<Counter>>>&&>(current))->adjust(value); } int as_const_tuple_get_caller(std::tuple<Value, std::optional<std::shared_ptr<const Counter>>> current, int value) { return std::get<1>(std::as_const(current))->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
        ("api::moved_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::forwarded_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::as_const_tuple_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_smart_pointer_arrow_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current)->adjust(value); } int const_shared_pair_get_caller(std::pair<std::expected<std::shared_ptr<const Counter>, Value>, Value> current, int value) { return std::get<0>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_nested_optional_expected_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("nested_optional_expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return (*current)->value().adjust(value); } int nested_opt_opt_exp_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return current.value().value().value().adjust(value); } int nested_opt_opt_exp_double_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return current->value()->value().adjust(value); } int nested_opt_opt_exp_deref_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return (**current).value().adjust(value); } int nested_opt_opt_exp_error_arrow_caller(std::optional<std::optional<std::expected<Value, Counter>>> current, int value) { return (*current)->error().adjust(value); } int nested_opt_opt_exp_auto_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { auto nested = (*current)->value(); return nested.adjust(value); } int exp_opt_exp_error_caller(std::expected<std::optional<std::expected<Value, Counter>>, Value> current, int value) { return current.value().value().error().adjust(value); } int exp_opt_exp_error_arrow_caller(std::expected<std::optional<std::expected<Value, Counter>>, Value> current, int value) { return (*current)->error().adjust(value); } int opt_exp_error_opt_sp_arrow_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { return current->error()->adjust(value); } int opt_exp_error_opt_weak_arrow_caller(std::optional<std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { return current->error()->lock()->adjust(value); } int opt_exp_error_opt_ref_get_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { return current->error()->get().adjust(value); } int opt_exp_opt_exp_error_caller(std::optional<std::expected<std::optional<std::expected<Value, Counter>>, Value>> current, int value) { return current->value()->error().adjust(value); } int exp_error_opt_exp_value_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { return current.error()->value().adjust(value); } int exp_error_opt_exp_arrow_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { return (*current.error())->adjust(value); } int auto_opt_exp_error_opt_sp_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { auto nested = current->error(); return nested->adjust(value); } int decltype_auto_exp_error_opt_exp_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { decltype(auto) nested = (*current.error())->value(); return nested.adjust(value); } int decltype_auto_opt_exp_error_opt_sp_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) nested = current->error(); return nested->adjust(value); } int decltype_auto_exp_error_opt_exp_arrow_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { decltype(auto) nested = (*current.error()); return nested->adjust(value); } int decltype_auto_opt_exp_error_opt_weak_lock_caller(std::optional<std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { decltype(auto) nested = current->error()->lock(); return nested->adjust(value); } int decltype_auto_opt_exp_value_sp_get_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { decltype(auto) pointer = current->value().get(); return pointer->adjust(value); } int decltype_auto_const_opt_exp_error_opt_sp_get_caller(const std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) pointer = current->error().value().get(); return pointer->adjust(value); } int decltype_auto_opt_exp_error_opt_sp_get_arrow_caller(std::optional<std::expected<Value, std::optional<std::shared_ptr<Counter>>>> current, int value) { decltype(auto) pointer = current->error()->get(); return pointer->adjust(value); } int decltype_auto_opt_opt_sp_arrow_caller(std::optional<std::optional<std::unique_ptr<Counter>>> current, int value) { decltype(auto) nested = *current; return nested->adjust(value); } int decltype_auto_opt_exp_error_opt_sp_deref_arrow_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) nested = *current->error(); return nested->adjust(value); } int auto_opt_exp_error_opt_ref_via_nested_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { auto nested = current->error(); return nested->get().adjust(value); } int decltype_auto_opt_exp_error_opt_ref_via_nested_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { decltype(auto) nested = current->error(); return nested->get().adjust(value); } int moved_nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return std::move(*current)->value().adjust(value); } int as_const_nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return std::as_const(*current)->value().adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::nested_opt_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_double_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_deref_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_error_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_auto_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::exp_opt_exp_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::exp_opt_exp_error_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_error_opt_sp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_error_opt_weak_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_error_opt_ref_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_opt_exp_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::exp_error_opt_exp_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::exp_error_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_opt_exp_error_opt_sp_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_exp_error_opt_exp_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_sp_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_exp_error_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_weak_lock_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_value_sp_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_const_opt_exp_error_opt_sp_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_sp_get_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_opt_sp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_sp_deref_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_opt_exp_error_opt_ref_via_nested_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_ref_via_nested_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_nested_opt_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::as_const_nested_opt_opt_exp_arrow_caller",
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
fn traces_cpp_expected_optional_reference_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected_optional.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int error_value_caller(std::expected<Value, std::optional<std::reference_wrapper<Counter>>> current, int value) { return current.error().value().get().adjust(value); } int error_dereference_caller(std::expected<Value, std::optional<std::reference_wrapper<Counter>>> current, int value) { return (*current.error()).get().adjust(value); } int value_value_caller(std::expected<std::optional<std::reference_wrapper<Counter>>, Value> current, int value) { return current.value().value().get().adjust(value); } int value_dereference_caller(std::expected<std::optional<std::reference_wrapper<Counter>>, Value> current, int value) { return (*current.value()).get().adjust(value); } int const_error_pointee_caller(std::expected<Value, std::optional<std::reference_wrapper<const Counter>>> current, int value) { return (*current.error()).get().adjust(value); } int get_copy_caller(std::expected<Value, std::optional<std::reference_wrapper<Counter>>> current, int value) { auto target = current.error().value().get(); return target.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::error_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::error_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::value_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_error_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_expected_optional_weak_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected_optional.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int error_value_caller(std::expected<Value, std::optional<std::weak_ptr<Counter>>> current, int value) { return current.error().value().lock()->adjust(value); } int error_dereference_caller(std::expected<Value, std::optional<std::weak_ptr<Counter>>> current, int value) { return (*current.error()).lock()->adjust(value); } int value_value_caller(std::expected<std::optional<std::weak_ptr<Counter>>, Value> current, int value) { return current.value().value().lock()->adjust(value); } int value_dereference_caller(std::expected<std::optional<std::weak_ptr<Counter>>, Value> current, int value) { return (*current.value()).lock()->adjust(value); } int const_error_pointee_caller(std::expected<Value, std::optional<std::weak_ptr<const Counter>>> current, int value) { return (*current.error()).lock()->adjust(value); } int lock_copy_caller(std::expected<Value, std::optional<std::weak_ptr<Counter>>> current, int value) { auto shared = current.error().value().lock(); return shared->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::error_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::error_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::value_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_error_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::lock_copy_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_optional_reference_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("optional.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { return current.value().get().adjust(value); } int dereference_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { return (*current).get().adjust(value); } int const_pointee_caller(std::optional<std::reference_wrapper<const Counter>> current, int value) { return (*current).get().adjust(value); } int get_alias_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { auto& target = (*current).get(); return target.adjust(value); } int get_copy_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { auto target = current.value().get(); return target.adjust(value); } int const_get_copy_caller(std::optional<std::reference_wrapper<const Counter>> current, int value) { auto target = (*current).get(); return target.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::get_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        ("api::const_get_copy_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_optional_weak_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("optional.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_caller(std::optional<std::weak_ptr<Counter>> current, int value) { return current.value().lock()->adjust(value); } int dereference_caller(std::optional<std::weak_ptr<Counter>> current, int value) { return (*current).lock()->adjust(value); } int const_pointee_caller(std::optional<std::weak_ptr<const Counter>> current, int value) { return (*current).lock()->adjust(value); } int lock_copy_caller(std::optional<std::weak_ptr<Counter>> current, int value) { auto shared = current.value().lock(); return shared->adjust(value); } int dereference_lock_copy_caller(std::optional<std::weak_ptr<Counter>> current, int value) { auto shared = (*current).lock(); return shared->adjust(value); } int const_lock_copy_caller(std::optional<std::weak_ptr<const Counter>> current, int value) { auto shared = (*current).lock(); return shared->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::lock_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::dereference_lock_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_lock_copy_caller",
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
fn traces_cpp_expected_reference_wrapper_value_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Error {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { return current.value().get().adjust(value); } int const_wrapper_caller(const std::expected<std::reference_wrapper<Counter>, Error> current, int value) { return current.value().get().adjust(value); } int const_pointee_caller(std::expected<std::reference_wrapper<const Counter>, Error> current, int value) { return current.value().get().adjust(value); } int alias_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto& current_value = current.value(); return current_value.get().adjust(value); } int get_alias_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto& target = current.value().get(); return target.adjust(value); } int const_get_alias_caller(const std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto&& target = current.value().get(); return target.adjust(value); } int get_copy_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto target = current.value().get(); return target.adjust(value); } int const_get_copy_caller(std::expected<std::reference_wrapper<const Counter>, Error> current, int value) { auto target = current.value().get(); return target.adjust(value); } int const_auto_get_copy_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { const auto target = current.value().get(); return target.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        ("api::const_wrapper_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_alias_caller", "api::Counter::adjust(int) &"),
        ("api::const_get_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        ("api::const_get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_get_copy_caller",
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
fn traces_cpp_expected_weak_pointer_value_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Error {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_caller(std::expected<std::weak_ptr<Counter>, Error> current, int value) { return current.value().lock()->adjust(value); } int const_pointee_caller(std::expected<std::weak_ptr<const Counter>, Error> current, int value) { return current.value().lock()->adjust(value); } int alias_caller(std::expected<std::weak_ptr<Counter>, Error> current, int value) { auto& current_value = current.value(); return current_value.lock()->adjust(value); } int lock_copy_caller(std::expected<std::weak_ptr<Counter>, Error> current, int value) { auto shared = current.value().lock(); return shared->adjust(value); } int const_lock_copy_caller(std::expected<std::weak_ptr<const Counter>, Error> current, int value) { auto shared = current.value().lock(); return shared->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::lock_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_lock_copy_caller",
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
fn traces_cpp_expected_reference_wrapper_error_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { return current.error().get().adjust(value); } int const_wrapper_caller(const std::expected<Value, std::reference_wrapper<Counter>> current, int value) { return current.error().get().adjust(value); } int const_pointee_caller(std::expected<Value, std::reference_wrapper<const Counter>> current, int value) { return current.error().get().adjust(value); } int alias_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto& error = current.error(); return error.get().adjust(value); } int get_alias_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto& target = current.error().get(); return target.adjust(value); } int const_get_alias_caller(const std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto&& target = current.error().get(); return target.adjust(value); } int get_copy_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto target = current.error().get(); return target.adjust(value); } int const_get_copy_caller(std::expected<Value, std::reference_wrapper<const Counter>> current, int value) { auto target = current.error().get(); return target.adjust(value); } int const_auto_get_copy_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { const auto target = current.error().get(); return target.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        ("api::const_wrapper_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_alias_caller", "api::Counter::adjust(int) &"),
        ("api::const_get_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        ("api::const_get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_get_copy_caller",
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
fn traces_cpp_expected_weak_pointer_error_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { return current.error().lock()->adjust(value); } int const_pointee_caller(std::expected<Value, std::weak_ptr<const Counter>> current, int value) { return current.error().lock()->adjust(value); } int alias_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { auto& error = current.error(); return error.lock()->adjust(value); } int lock_copy_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { auto shared = current.error().lock(); return shared->adjust(value); } int const_lock_copy_caller(std::expected<Value, std::weak_ptr<const Counter>> current, int value) { auto shared = current.error().lock(); return shared->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::lock_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_lock_copy_caller",
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
fn traces_cpp_expected_smart_pointer_get_error_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { return current.error().get()->adjust(value); } int const_pointee_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { return current.error().get()->adjust(value); } int alias_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto& error = current.error(); return error.get()->adjust(value); } int get_copy_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto pointer = current.error().get(); return pointer->adjust(value); } int const_get_copy_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { auto pointer = current.error().get(); return pointer->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_get_copy_caller",
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
fn traces_cpp_expected_smart_pointer_dereference_errors_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { return (*current.error()).adjust(value); } int const_pointee_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { return (*current.error()).adjust(value); } int alias_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto& error = current.error(); return (*error).adjust(value); } int dereference_copy_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto target = *current.error(); return target.adjust(value); } int const_dereference_copy_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { auto target = *current.error(); return target.adjust(value); } int dereference_alias_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto& target = *current.error(); return target.adjust(value); } int const_dereference_alias_caller(const std::expected<Value, std::shared_ptr<Counter>> current, int value) { auto&& target = *current.error(); return target.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::dereference_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_alias_caller",
            "api::Counter::adjust(int) &",
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
fn traces_cpp_auto_expected_error_wrapper_copies_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int optional_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto error = current.error(); return error->adjust(value); } int const_optional_caller(std::expected<Value, std::optional<Counter>> current, int value) { const auto error = current.error(); return error->adjust(value); } int nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { auto error = current.error(); return error.error().adjust(value); } int const_nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { const auto error = current.error(); return error.error().adjust(value); } int direct_nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { return current.error().error().adjust(value); } int direct_const_nested_expected_caller(const std::expected<Value, std::expected<Value, Counter>> current, int value) { return current.error().error().adjust(value); } int direct_const_nested_error_type_caller(std::expected<Value, const std::expected<Value, Counter>> current, int value) { return current.error().error().adjust(value); } int direct_moved_nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { return std::move(current).error().error().adjust(value); } int pointer_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { auto error = current.error(); return error->adjust(value); } int const_pointer_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { const auto error = current.error(); return error->adjust(value); } int wrapper_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto error = current.error(); return error.get().adjust(value); } int const_wrapper_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { const auto error = current.error(); return error.get().adjust(value); } int weak_caller(std::expected<Value, std::weak_ptr<const Counter>> current, int value) { auto error = current.error(); return error.lock()->adjust(value); } int const_weak_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { const auto error = current.error(); return error.lock()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::optional_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_optional_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::nested_expected_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_nested_expected_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::direct_nested_expected_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::direct_const_nested_expected_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::direct_const_nested_error_type_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::direct_moved_nested_expected_caller",
            "api::Counter::adjust(int) &&",
        ),
        ("api::pointer_caller", "api::Counter::adjust(int) const &"),
        ("api::const_pointer_caller", "api::Counter::adjust(int) &"),
        ("api::wrapper_caller", "api::Counter::adjust(int) &"),
        ("api::const_wrapper_caller", "api::Counter::adjust(int) &"),
        ("api::weak_caller", "api::Counter::adjust(int) const &"),
        ("api::const_weak_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_auto_reference_aliases_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint moved_alias_caller(int value) { Alias target{}; auto&& alias = std::move(target); return alias.adjust(value); }\nint reference_wrapper_alias_caller(int value) { Alias target{}; std::reference_wrapper<Alias> wrapper(target); auto& alias = wrapper.get(); return alias.adjust(value); }\nint optional_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::as_const(*current); return alias.adjust(value); }\nint smart_pointer_alias_caller(int value) { std::shared_ptr<const Alias> current; auto&& alias = *current; return alias.adjust(value); }\n}\n";
    for (caller, expected_callee) in [
        ("api::moved_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::reference_wrapper_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::smart_pointer_alias_caller",
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
fn traces_cpp_forwarded_base_alias_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api {\nclass Base { public: int adjust(int value) & { return value; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { Derived target{}; auto&& alias = std::forward<Base&&>(target); return alias.adjust(value); }\n}\n";
    let trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        source,
        "api::caller",
        TraceDirection::Both,
    )
    .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"],
    );
}

#[test]
fn traces_cpp_forwarded_optional_base_alias_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api {\nclass Base { public: int adjust(int value) & { return value; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { std::optional<Derived> current; auto&& alias = std::forward<Base&&>(*current); return alias.adjust(value); }\n}\n";
    let trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        source,
        "api::caller",
        TraceDirection::Both,
    )
    .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"],
    );
}

#[test]
fn traces_cpp_cast_optional_base_alias_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let source = "namespace api { class Base { public: int adjust(int value) & { return value; } }; class Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } }; int caller(int value) { std::optional<Derived> current; auto&& alias = static_cast<Base&&>(*current); return alias.adjust(value); } }\n";
    let trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        source,
        "api::caller",
        TraceDirection::Both,
    )
    .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"]
    );
}

#[test]
fn traces_cpp_addressof_reference_aliases_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; using Alias = Counter; int caller(int value) { Alias target{}; auto& alias = *std::addressof(target); return alias.adjust(value); } int const_caller(int value) { const Alias target{}; auto&& alias = *std::addressof(target); return alias.adjust(value); } int wrapped_const_caller(int value) { Alias target{}; auto& alias = *std::addressof(std::as_const(target)); return alias.adjust(value); } int native_caller(int value) { Alias target{}; auto& alias = *&target; return alias.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        (
            "api::wrapped_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::native_caller", "api::Counter::adjust(int) &"),
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
            "{caller}"
        );
    }
}

#[test]
fn traces_cpp_cast_addressof_reference_aliases_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Base { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; class Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } }; int caller(int value) { Derived target{}; auto& alias = *std::addressof(static_cast<Base&>(target)); return alias.adjust(value); } int const_caller(int value) { Derived target{}; auto& alias = *std::addressof(std::as_const(static_cast<const Base&>(target))); return alias.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::caller", "api::Base::adjust(int) &"),
        ("api::const_caller", "api::Base::adjust(int) const &"),
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
fn traces_cpp_volatile_const_member_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) volatile const & { return value + 1; } int const_caller(int value) volatile const { return adjust(value); } }; int caller(int value) { const Counter current{}; return current.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::caller", "api::Counter::adjust(int) volatile const &"),
        (
            "api::Counter::const_caller(int) volatile const",
            "api::Counter::adjust(int) volatile const &",
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
fn traces_cpp_decltype_auto_reference_aliases_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; using Alias = Counter; int copied_caller(int value) { Alias target{}; decltype(auto) alias = target; return alias.adjust(value); } int copied_const_caller(int value) { const Alias target{}; decltype(auto) alias = target; return alias.adjust(value); } int parenthesized_caller(int value) { Alias target{}; decltype(auto) alias = (target); return alias.adjust(value); } int const_caller(int value) { const Alias target{}; decltype(auto) alias = (target); return alias.adjust(value); } int moved_caller(int value) { Alias target{}; decltype(auto) alias = std::move(target); return alias.adjust(value); } int pointer_caller(int value) { Alias* pointer = nullptr; decltype(auto) alias = *pointer; return alias.adjust(value); } int optional_caller(int value) { std::optional<Alias> current; decltype(auto) alias = current.value(); return alias.adjust(value); } int wrapper_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); decltype(auto) alias = current.get(); return alias.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::copied_caller", "api::Counter::adjust(int) &"),
        (
            "api::copied_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::parenthesized_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        ("api::optional_caller", "api::Counter::adjust(int) &"),
        ("api::wrapper_caller", "api::Counter::adjust(int) &"),
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
fn preserves_cpp_decltype_auto_parenthesized_binding_access_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } }; using Alias = Counter; int pointer_caller(int value) { Alias* current = nullptr; decltype(auto) alias = (current); return alias->adjust(value); } int optional_caller(int value) { std::optional<Alias> current; decltype(auto) alias = (current); return alias->adjust(value); } int wrapper_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); decltype(auto) alias = (current); return alias.get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        ("api::optional_caller", "api::Counter::adjust(int) &"),
        ("api::wrapper_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_auto_reference_alias_at_position_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::as_const(*current); return alias.adjust(value); }\n}\n";
    let trace = trace_symbol_graph_at_position_from_index_with_source(
        &db_path,
        &source_path,
        source,
        &Position { row: 7, column: 5 },
        TraceDirection::Both,
    )
    .unwrap();

    assert_eq!(trace.symbol.semantic_path, "api::alias_caller");
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) const &"],
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
fn symbol_query_context_rejects_workspace_overlay_in_ignored_directory() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let ignored = workspace.join(".venv").join("ignored.py");

    fs::create_dir_all(ignored.parent().unwrap()).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();

    let error = SymbolQueryContext::workspace(&workspace)
        .unwrap()
        .with_source_overlay(&ignored, "def ignored() -> int:\n    return 2\n")
        .expect_err("workspace contexts should reject overlays in ignored directories");

    assert!(error.to_string().contains("ignored workspace directory"));
}

#[test]
fn symbol_query_context_rejects_workspace_overlay_with_unsupported_extension() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let unsupported = workspace.join("notes.txt");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();

    let error = SymbolQueryContext::workspace(&workspace)
        .unwrap()
        .with_source_overlay(&unsupported, "not source code")
        .expect_err("workspace contexts should reject unsupported source overlays");

    assert!(error.to_string().contains("not a supported source file"));
}

#[test]
fn symbol_query_context_rejects_index_overlay_outside_indexed_workspace() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let outside = dir.join("outside.py");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();
    fs::write(&outside, "def outside() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&workspace, &db_path).unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(&outside, "def outside() -> int:\n    return 2\n")
        .unwrap();
    let error = context
        .list_symbols(10, None, None)
        .expect_err("index contexts should reject overlays outside the indexed workspace");

    assert!(error.to_string().contains("outside indexed workspace"));
}

#[test]
fn symbol_query_context_rejects_index_overlay_in_ignored_directory() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let ignored = workspace.join("node_modules").join("ignored.py");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(ignored.parent().unwrap()).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&workspace, &db_path).unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(&ignored, "def ignored() -> int:\n    return 2\n")
        .unwrap();
    let error = context
        .list_symbols(10, None, None)
        .expect_err("index contexts should reject overlays in ignored directories");

    assert!(error.to_string().contains("ignored workspace directory"));
}

#[test]
fn symbol_query_context_rejects_index_overlay_with_unsupported_extension() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let unsupported = workspace.join("notes.txt");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&workspace, &db_path).unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(&unsupported, "not source code")
        .unwrap();
    let error = context
        .list_symbols(10, None, None)
        .expect_err("index contexts should reject unsupported source overlays");

    assert!(error.to_string().contains("not a supported source file"));
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
