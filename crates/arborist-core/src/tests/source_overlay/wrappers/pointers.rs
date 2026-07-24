use super::*;

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
