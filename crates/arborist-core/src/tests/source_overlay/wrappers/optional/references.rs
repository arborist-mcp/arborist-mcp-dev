use super::*;

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
