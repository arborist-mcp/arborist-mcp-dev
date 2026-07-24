use super::*;

#[test]
fn resolves_cpp_expected_value_reference_wrapper_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_value_reference_wrapper_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Error {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint value_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { return current.value().get().adjust(value); }\nint moved_value_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { return std::move(current).value().get().adjust(value); }\nint const_wrapper_caller(const std::expected<std::reference_wrapper<Counter>, Error> current, int value) { return current.value().get().adjust(value); }\nint const_value_caller(std::expected<std::reference_wrapper<const Counter>, Error> current, int value) { return current.value().get().adjust(value); }\nint alias_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto& current_value = current.value(); return current_value.get().adjust(value); }\nint const_alias_caller(const std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto&& current_value = current.value(); return current_value.get().adjust(value); }\nint get_alias_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto& target = current.value().get(); return target.adjust(value); }\nint decltype_get_alias_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { decltype(auto) target = current.value().get(); return target.adjust(value); }\nint const_get_alias_caller(const std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto&& target = current.value().get(); return target.adjust(value); }\nint get_copy_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto target = current.value().get(); return target.adjust(value); }\nint const_get_copy_caller(std::expected<std::reference_wrapper<const Counter>, Error> current, int value) { auto target = current.value().get(); return target.adjust(value); }\nint const_auto_get_copy_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { const auto target = current.value().get(); return target.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &"),
        ("api::const_wrapper_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::const_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_get_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::const_get_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        ("api::const_get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_get_copy_caller",
            "api::Counter::adjust(int) const &",
        ),
    ];
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
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

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
fn resolves_cpp_expected_value_weak_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_value_weak_pointer_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Error {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint value_caller(std::expected<std::weak_ptr<Counter>, Error> current, int value) { return current.value().lock()->adjust(value); }\nint moved_value_caller(std::expected<std::weak_ptr<Counter>, Error> current, int value) { return std::move(current).value().lock()->adjust(value); }\nint const_value_caller(std::expected<std::weak_ptr<const Counter>, Error> current, int value) { return current.value().lock()->adjust(value); }\nint alias_caller(std::expected<std::weak_ptr<Counter>, Error> current, int value) { auto& current_value = current.value(); return current_value.lock()->adjust(value); }\nint lock_copy_caller(std::expected<std::weak_ptr<Counter>, Error> current, int value) { auto shared = current.value().lock(); return shared->adjust(value); }\nint const_lock_copy_caller(std::expected<std::weak_ptr<const Counter>, Error> current, int value) { auto shared = current.value().lock(); return shared->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::lock_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_lock_copy_caller",
            "api::Counter::adjust(int) const &",
        ),
    ];
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
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

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
fn resolves_cpp_expected_error_reference_wrapper_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_reference_wrapper_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { return current.error().get().adjust(value); }\nint moved_error_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { return std::move(current).error().get().adjust(value); }\nint const_wrapper_caller(const std::expected<Value, std::reference_wrapper<Counter>> current, int value) { return current.error().get().adjust(value); }\nint const_error_caller(std::expected<Value, std::reference_wrapper<const Counter>> current, int value) { return current.error().get().adjust(value); }\nint alias_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto& error = current.error(); return error.get().adjust(value); }\nint const_alias_caller(const std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto&& error = current.error(); return error.get().adjust(value); }\nint get_alias_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto& target = current.error().get(); return target.adjust(value); }\nint decltype_get_alias_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { decltype(auto) target = current.error().get(); return target.adjust(value); }\nint const_get_alias_caller(const std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto&& target = current.error().get(); return target.adjust(value); }\nint get_copy_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto target = current.error().get(); return target.adjust(value); }\nint const_get_copy_caller(std::expected<Value, std::reference_wrapper<const Counter>> current, int value) { auto target = current.error().get(); return target.adjust(value); }\nint const_auto_get_copy_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { const auto target = current.error().get(); return target.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_caller", "api::Counter::adjust(int) &"),
        ("api::moved_error_caller", "api::Counter::adjust(int) &"),
        ("api::const_wrapper_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::const_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_get_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::const_get_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        ("api::const_get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_get_copy_caller",
            "api::Counter::adjust(int) const &",
        ),
    ];
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
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

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
fn resolves_cpp_expected_error_weak_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_weak_pointer_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { return current.error().lock()->adjust(value); }\nint moved_error_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { return std::move(current).error().lock()->adjust(value); }\nint const_error_caller(std::expected<Value, std::weak_ptr<const Counter>> current, int value) { return current.error().lock()->adjust(value); }\nint alias_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { auto& error = current.error(); return error.lock()->adjust(value); }\nint lock_copy_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { auto shared = current.error().lock(); return shared->adjust(value); }\nint const_lock_copy_caller(std::expected<Value, std::weak_ptr<const Counter>> current, int value) { auto shared = current.error().lock(); return shared->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_caller", "api::Counter::adjust(int) &"),
        ("api::moved_error_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::lock_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_lock_copy_caller",
            "api::Counter::adjust(int) const &",
        ),
    ];
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
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

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
fn resolves_cpp_expected_error_smart_pointer_get_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_smart_pointer_get_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint unique_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { return current.error().get()->adjust(value); }\nint shared_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { return std::move(current).error().get()->adjust(value); }\nint const_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { return current.error().get()->adjust(value); }\nint alias_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto& error = current.error(); return error.get()->adjust(value); }\nint get_copy_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto pointer = current.error().get(); return pointer->adjust(value); }\nint const_get_copy_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { auto pointer = current.error().get(); return pointer->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::unique_caller", "api::Counter::adjust(int) &"),
        ("api::shared_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_get_copy_caller",
            "api::Counter::adjust(int) const &",
        ),
    ];
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
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

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
fn resolves_cpp_expected_error_smart_pointer_dereferences_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_smart_pointer_dereferences.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint unique_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { return (*current.error()).adjust(value); }\nint shared_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { return (*std::move(current).error()).adjust(value); }\nint const_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { return (*current.error()).adjust(value); }\nint alias_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto& error = current.error(); return (*error).adjust(value); }\nint dereference_copy_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto target = *current.error(); return target.adjust(value); }\nint const_dereference_copy_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { auto target = *current.error(); return target.adjust(value); }\nint dereference_alias_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto& target = *current.error(); return target.adjust(value); }\nint const_dereference_alias_caller(const std::expected<Value, std::shared_ptr<Counter>> current, int value) { auto&& target = *current.error(); return target.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::unique_caller", "api::Counter::adjust(int) &"),
        ("api::shared_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
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
    ];
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
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

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
fn resolves_cpp_auto_expected_error_wrapper_copies_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("auto_expected_error_wrapper_copies.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nint optional_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto error = current.error(); return error->adjust(value); }\nint const_optional_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto error = current.error(); return error->adjust(value); }\nint const_copied_optional_caller(std::expected<Value, std::optional<Counter>> current, int value) { const auto error = current.error(); return error->adjust(value); }\nint nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { auto error = current.error(); return error.error().adjust(value); }\nint const_nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { const auto error = current.error(); return error.error().adjust(value); }\nint direct_nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { return current.error().error().adjust(value); }\nint direct_const_nested_expected_caller(const std::expected<Value, std::expected<Value, Counter>> current, int value) { return current.error().error().adjust(value); }\nint direct_const_nested_error_type_caller(std::expected<Value, const std::expected<Value, Counter>> current, int value) { return current.error().error().adjust(value); }\nint direct_moved_nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { return std::move(current).error().error().adjust(value); }\nint pointer_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { auto error = current.error(); return error->adjust(value); }\nint const_copied_pointer_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { const auto error = current.error(); return error->adjust(value); }\nint const_pointer_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { auto error = current.error(); return error->adjust(value); }\nint wrapper_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto error = current.error(); return error.get().adjust(value); }\nint const_copied_wrapper_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { const auto error = current.error(); return error.get().adjust(value); }\nint weak_caller(std::expected<Value, std::weak_ptr<const Counter>> current, int value) { auto error = current.error(); return error.lock()->adjust(value); }\nint const_copied_weak_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { const auto error = current.error(); return error.lock()->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::optional_caller", "api::Counter::adjust(int) &"),
        ("api::const_optional_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_copied_optional_caller",
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
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_copied_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::wrapper_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_copied_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::weak_caller", "api::Counter::adjust(int) const &"),
        (
            "api::const_copied_weak_caller",
            "api::Counter::adjust(int) &",
        ),
    ];
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
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

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
