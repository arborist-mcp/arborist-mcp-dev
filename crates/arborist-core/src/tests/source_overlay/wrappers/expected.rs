use super::*;

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
