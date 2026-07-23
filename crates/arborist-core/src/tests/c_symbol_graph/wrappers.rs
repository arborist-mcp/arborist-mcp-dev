use super::*;

#[test]
fn resolves_cpp_weak_pointer_lock_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("weak_pointer_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint direct_caller(std::weak_ptr<Alias> current, int value) { return current.lock()->adjust(value); }\nint local_caller(std::weak_ptr<Alias> current, int value) { auto shared = current.lock(); return shared->adjust(value); }\nint const_wrapper_caller(const std::weak_ptr<Alias> current, int value) { return current.lock()->adjust(value); }\nint auto_const_wrapper_caller(int value) { const auto current = std::weak_ptr<Alias>{}; return current.lock()->adjust(value); }\nint const_caller(std::weak_ptr<const Alias> current, int value) { return current.lock()->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        ("api::local_caller", "api::Counter::adjust(int) &"),
        ("api::const_wrapper_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_const_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
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
fn resolves_cpp_expected_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nusing Alias = Counter;\nint arrow_caller(std::expected<Alias, int> current, int value) { return current->adjust(value); }\nint value_caller(std::expected<Alias, int> current, int value) { return current.value().adjust(value); }\nint dereference_caller(std::expected<Alias, int> current, int value) { return (*current).adjust(value); }\nint moved_value_caller(std::expected<Alias, int> current, int value) { return std::move(current).value().adjust(value); }\nint const_caller(const std::expected<Alias, int> current, int value) { return current.value().adjust(value); }\nint auto_value_caller(std::expected<Alias, int> current, int value) { auto current_value = current.value(); return current_value.adjust(value); }\nint const_auto_value_caller(std::expected<Alias, int> current, int value) { const auto current_value = current.value(); return current_value.adjust(value); }\nint copied_const_source_value_caller(const std::expected<Alias, int> current, int value) { auto current_value = current.value(); return current_value.adjust(value); }\nint moved_auto_value_caller(std::expected<Alias, int> current, int value) { auto current_value = std::move(current).value(); return current_value.adjust(value); }\nint nested_expected_value_caller(std::expected<std::expected<Alias, int>, int> current, int value) { return current.value().value().adjust(value); }\nint const_nested_expected_value_caller(const std::expected<std::expected<Alias, int>, int> current, int value) { return current.value().value().adjust(value); }\nint moved_nested_expected_value_caller(std::expected<std::expected<Alias, int>, int> current, int value) { return std::move(current).value().value().adjust(value); }\nint auto_nested_expected_value_caller(std::expected<std::expected<Alias, int>, int> current, int value) { auto current_value = current.value(); return current_value.value().adjust(value); }\nint auto_nested_expected_error_caller(std::expected<std::expected<int, Alias>, int> current, int value) { auto current_value = current.value(); return current_value.error().adjust(value); }\nint const_auto_nested_expected_error_caller(std::expected<std::expected<int, Alias>, int> current, int value) { const auto current_value = current.value(); return current_value.error().adjust(value); }\nint nested_optional_value_caller(std::expected<std::optional<Alias>, int> current, int value) { return current.value().value().adjust(value); }\nint const_nested_optional_value_caller(const std::expected<std::optional<Alias>, int> current, int value) { return current.value().value().adjust(value); }\nint moved_nested_optional_value_caller(std::expected<std::optional<Alias>, int> current, int value) { return std::move(current).value().value().adjust(value); }\nint auto_optional_value_caller(std::expected<std::optional<Alias>, int> current, int value) { auto current_value = current.value(); return current_value->adjust(value); }\nint const_auto_optional_value_caller(std::expected<std::optional<Alias>, int> current, int value) { const auto current_value = current.value(); return current_value->adjust(value); }\nint auto_pointer_value_caller(std::expected<std::shared_ptr<Alias>, int> current, int value) { auto current_value = current.value(); return current_value->adjust(value); }\nint get_copy_caller(std::expected<std::unique_ptr<Alias>, int> current, int value) { auto pointer = current.value().get(); return pointer->adjust(value); }\nint const_get_copy_caller(std::expected<std::shared_ptr<const Alias>, int> current, int value) { auto pointer = current.value().get(); return pointer->adjust(value); }\nint dereference_copy_caller(std::expected<std::unique_ptr<Alias>, int> current, int value) { auto target = *current.value(); return target.adjust(value); }\nint const_dereference_copy_caller(std::expected<std::shared_ptr<const Alias>, int> current, int value) { auto target = *current.value(); return target.adjust(value); }\nint dereference_alias_caller(std::expected<std::unique_ptr<Alias>, int> current, int value) { auto& target = *current.value(); return target.adjust(value); }\nint const_dereference_alias_caller(const std::expected<std::shared_ptr<Alias>, int> current, int value) { auto&& target = *current.value(); return target.adjust(value); }\nint auto_caller(int value) { auto current = std::expected<Alias, int>{}; return current->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::arrow_caller", "api::Counter::adjust(int) &"),
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &&"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
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
            "api::moved_auto_value_caller",
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
        ("api::auto_caller", "api::Counter::adjust(int) &"),
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
fn resolves_cpp_expected_error_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {};\nclass Failure {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nusing Value = Counter;\nusing Error = Failure;\nint error_caller(std::expected<Value, Error> current, int value) { return current.error().adjust(value); }\nint moved_error_caller(std::expected<Value, Error> current, int value) { return std::move(current).error().adjust(value); }\nint const_error_caller(const std::expected<Value, Error> current, int value) { return current.error().adjust(value); }\nint const_value_caller(std::expected<const Value, Error> current, int value) { return current.error().adjust(value); }\nint const_error_type_caller(std::expected<Value, const Error> current, int value) { return current.error().adjust(value); }\nint auto_error_caller(int value) { auto current = std::expected<Value, Error>{}; return current.error().adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_caller", "api::Failure::adjust(int) &"),
        ("api::moved_error_caller", "api::Failure::adjust(int) &&"),
        (
            "api::const_error_caller",
            "api::Failure::adjust(int) const &",
        ),
        ("api::const_value_caller", "api::Failure::adjust(int) &"),
        (
            "api::const_error_type_caller",
            "api::Failure::adjust(int) const &",
        ),
        ("api::auto_error_caller", "api::Failure::adjust(int) &"),
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
fn resolves_cpp_expected_error_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Failure {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nint error_alias_caller(std::expected<Value, Failure> current, int value) { auto& alias = current.error(); return alias.adjust(value); }\nint decltype_error_alias_caller(std::expected<Value, Failure> current, int value) { decltype(auto) alias = current.error(); return alias.adjust(value); }\nint const_error_alias_caller(const std::expected<Value, Failure> current, int value) { auto&& alias = current.error(); return alias.adjust(value); }\nint moved_error_alias_caller(std::expected<Value, Failure> current, int value) { auto&& alias = std::move(current).error(); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_alias_caller", "api::Failure::adjust(int) &"),
        (
            "api::decltype_error_alias_caller",
            "api::Failure::adjust(int) &",
        ),
        (
            "api::const_error_alias_caller",
            "api::Failure::adjust(int) const &",
        ),
        (
            "api::moved_error_alias_caller",
            "api::Failure::adjust(int) &",
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
fn resolves_cpp_expected_error_smart_pointer_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_smart_pointer_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { return current.error()->adjust(value); }\nint moved_error_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { return std::move(current).error()->adjust(value); }\nint const_error_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { return current.error()->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_caller", "api::Counter::adjust(int) &"),
        ("api::moved_error_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_caller",
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
fn resolves_cpp_expected_error_smart_pointer_alias_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_smart_pointer_alias_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint unique_alias_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto& error = current.error(); return error->adjust(value); }\nint shared_alias_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { decltype(auto) error = current.error(); return error->adjust(value); }\nint const_shared_alias_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { auto&& error = current.error(); return error->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::unique_alias_caller", "api::Counter::adjust(int) &"),
        ("api::shared_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_alias_caller",
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
fn resolves_cpp_expected_error_optional_arrow_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_optional_arrow_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n    int adjust(int value) const && { return value + 3; }\n};\nint error_caller(std::expected<Value, std::optional<Counter>> current, int value) { return current.error()->adjust(value); }\nint moved_error_caller(std::expected<Value, std::optional<Counter>> current, int value) { return std::move(current).error()->adjust(value); }\nint const_error_caller(const std::expected<Value, std::optional<Counter>> current, int value) { return current.error()->adjust(value); }\nint const_pointee_caller(std::expected<Value, std::optional<const Counter>> current, int value) { return current.error()->adjust(value); }\nint value_caller(std::expected<Value, std::optional<Counter>> current, int value) { return current.error().value().adjust(value); }\nint moved_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { return std::move(current).error().value().adjust(value); }\nint dereference_caller(std::expected<Value, std::optional<Counter>> current, int value) { return (*current.error()).adjust(value); }\nint const_value_caller(const std::expected<Value, std::optional<Counter>> current, int value) { return current.error().value().adjust(value); }\nint auto_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = current.error().value(); return error_value.adjust(value); }\nint const_auto_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { const auto error_value = current.error().value(); return error_value.adjust(value); }\nint copied_const_source_value_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = current.error().value(); return error_value.adjust(value); }\nint auto_pointer_value_caller(std::expected<Value, std::optional<std::shared_ptr<Counter>>> current, int value) { auto error_value = current.error().value(); return error_value->adjust(value); }\nint auto_dereference_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = *current.error(); return error_value.adjust(value); }\nint const_auto_dereference_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { const auto error_value = *current.error(); return error_value.adjust(value); }\nint copied_const_source_dereference_value_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = *current.error(); return error_value.adjust(value); }\nint value_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error_value = current.error().value(); return error_value.adjust(value); }\nint decltype_value_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { decltype(auto) error_value = current.error().value(); return error_value.adjust(value); }\nint const_value_alias_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto&& error_value = current.error().value(); return error_value.adjust(value); }\nint dereference_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error_value = *current.error(); return error_value.adjust(value); }\nint decltype_dereference_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { decltype(auto) error_value = *current.error(); return error_value.adjust(value); }\nint const_dereference_alias_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto&& error_value = *current.error(); return error_value.adjust(value); }\nint alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error = current.error(); return error->adjust(value); }\nint decltype_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { decltype(auto) error = current.error(); return error->adjust(value); }\nint const_alias_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto&& error = current.error(); return error->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_caller", "api::Counter::adjust(int) &"),
        ("api::moved_error_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &&"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_caller",
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
        ("api::decltype_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_alias_caller",
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

#[test]
fn resolves_cpp_wrapped_weak_pointer_lock_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_weak_pointer_lock.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int moved_caller(std::weak_ptr<Counter> current, int value) { return std::move(current).lock()->adjust(value); } int const_caller(std::weak_ptr<Counter> current, int value) { return std::as_const(current).lock()->adjust(value); } int forwarded_caller(std::weak_ptr<Counter> current, int value) { return std::forward<std::weak_ptr<Counter>&&>(current).lock()->adjust(value); } int const_pointee_caller(std::weak_ptr<const Counter> current, int value) { return std::move(current).lock()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
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
fn resolves_cpp_wrapped_reference_wrapper_get_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_reference_wrapper_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int moved_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::move(current).get().adjust(value); } int const_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::as_const(current).get().adjust(value); } int forwarded_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::forward<std::reference_wrapper<Counter>&&>(current).get().adjust(value); } int const_pointee_caller(const Counter& target, int value) { std::reference_wrapper<const Counter> current(target); return std::move(current).get().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
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
fn resolves_cpp_wrapped_smart_pointer_get_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int moved_caller(std::shared_ptr<Counter> current, int value) { return std::move(current).get()->adjust(value); } int const_caller(std::shared_ptr<Counter> current, int value) { return std::as_const(current).get()->adjust(value); } int forwarded_caller(std::shared_ptr<Counter> current, int value) { return std::forward<std::shared_ptr<Counter>&&>(current).get()->adjust(value); } int const_pointee_caller(std::shared_ptr<const Counter> current, int value) { return std::move(current).get()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
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
fn resolves_cpp_direct_standard_pointer_cast_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("direct_standard_pointer_cast.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int get_if_caller(std::variant<Counter, Value> current, int value) { return std::get_if<Counter>(&current)->adjust(value); } int const_get_if_caller(std::variant<Counter, Value> current, int value) { return std::get_if<Counter>(std::addressof(std::as_const(current)))->adjust(value); } int any_cast_caller(std::any current, int value) { return std::any_cast<Counter>(&current)->adjust(value); } int const_any_cast_caller(std::any current, int value) { return std::any_cast<Counter>(std::addressof(std::as_const(current)))->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
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
fn resolves_cpp_wrapped_indexed_get_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_indexed_get_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int moved_smart_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int const_smart_caller(std::tuple<Value, std::shared_ptr<const Counter>> current, int value) { return std::get<1>(std::as_const(current))->adjust(value); } int forwarded_raw_caller(std::tuple<Value, Counter*> current, int value) { return std::get<1>(std::forward<std::tuple<Value, Counter*>&&>(current))->adjust(value); } int const_raw_caller(std::tuple<Value, const Counter*> current, int value) { return std::get<1>(std::as_const(current))->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_smart_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_smart_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::forwarded_raw_caller", "api::Counter::adjust(int) &"),
        ("api::const_raw_caller", "api::Counter::adjust(int) const &"),
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
fn resolves_cpp_wrapped_indexed_get_expected_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_indexed_get_expected_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int value_caller(std::tuple<Value, std::expected<std::shared_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::shared_ptr<Counter>, Value>>&&>(current)).value()->adjust(value); } int error_caller(std::tuple<Value, std::expected<Value, std::shared_ptr<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::error_caller", "api::Counter::adjust(int) const &"),
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
fn resolves_cpp_wrapped_indexed_get_expected_raw_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_indexed_get_expected_raw_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_caller(std::tuple<Value, std::expected<Counter*, Value>> current, int value) { return std::get<1>(std::move(current)).value()->adjust(value); } int error_caller(std::tuple<Value, std::expected<Value, Counter*>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<Value, Counter*>>&&>(current)).error()->adjust(value); } int const_value_caller(std::tuple<Value, std::expected<const Counter*, Value>> current, int value) { return std::get<1>(std::as_const(current)).value()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::error_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_caller",
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
fn resolves_cpp_wrapped_indexed_get_expected_optional_pointer_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("wrapped_indexed_get_expected_optional_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int smart_value_caller(std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>> current, int value) { return std::get<1>(std::move(current)).value()->adjust(value); } int smart_error_caller(std::tuple<Value, std::expected<Value, std::optional<std::shared_ptr<const Counter>>>> current, int value) { return std::get<1>(std::as_const(current)).error()->adjust(value); } int raw_value_caller(std::tuple<Value, std::expected<std::optional<Counter*>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::optional<Counter*>, Value>>&&>(current)).value()->adjust(value); } int raw_error_caller(std::tuple<Value, std::expected<Value, std::optional<const Counter*>>> current, int value) { return std::get<1>(std::as_const(current)).error()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::smart_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::smart_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::raw_value_caller", "api::Counter::adjust(int) &"),
        ("api::raw_error_caller", "api::Counter::adjust(int) const &"),
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
fn resolves_cpp_wrapped_indexed_get_expected_optional_wrapper_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("wrapped_indexed_get_expected_optional_wrappers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int weak_value_caller(std::tuple<Value, std::expected<std::optional<std::weak_ptr<Counter>>, Value>> current, int value) { return std::get<1>(std::move(current)).value()->lock()->adjust(value); } int reference_error_caller(std::tuple<Value, std::expected<Value, std::optional<std::reference_wrapper<const Counter>>>> current, int value) { return std::get<1>(std::as_const(current)).error()->get().adjust(value); } int smart_value_caller(std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>>&&>(current)).value()->get()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::weak_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::reference_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::smart_value_caller", "api::Counter::adjust(int) &"),
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
fn resolves_cpp_wrapped_indexed_get_expected_wrapper_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_indexed_get_expected_wrappers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int weak_value_caller(std::tuple<Value, std::expected<std::weak_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::move(current)).value().lock()->adjust(value); } int weak_error_caller(std::tuple<Value, std::expected<Value, std::weak_ptr<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error().lock()->adjust(value); } int reference_value_caller(std::tuple<Value, std::expected<std::reference_wrapper<Counter>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::reference_wrapper<Counter>, Value>>&&>(current)).value().get().adjust(value); } int reference_error_caller(std::tuple<Value, std::expected<Value, std::reference_wrapper<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error().get().adjust(value); } int smart_value_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::move(current)).value().get()->adjust(value); } int smart_error_get_caller(std::tuple<Value, std::expected<Value, std::shared_ptr<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error().get()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
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
fn resolves_cpp_reference_wrapper_get_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("reference_wrapper_get_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint wrapper_alias_caller(int value) { Alias target{}; std::reference_wrapper<Alias> wrapper(target); auto& current = wrapper.get(); return current.adjust(value); }\nint const_wrapper_alias_caller(int value) { const Alias target{}; std::reference_wrapper<const Alias> wrapper(target); auto&& current = wrapper.get(); return current.adjust(value); }\nint ref_alias_caller(int value) { Alias target{}; auto&& current = std::ref(target).get(); return current.adjust(value); }\nint cref_alias_caller(int value) { Alias target{}; auto&& current = std::cref(target).get(); return current.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::wrapper_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_wrapper_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::ref_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::cref_alias_caller",
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
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
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
fn resolves_cpp_optional_auto_value_copies_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_auto_value_copies.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint auto_value_caller(int value) { std::optional<Alias> current; auto current_value = current.value(); return current_value.adjust(value); }\nint const_auto_value_caller(int value) { std::optional<Alias> current; const auto current_value = current.value(); return current_value.adjust(value); }\nint copied_const_source_value_caller(int value) { const std::optional<Alias> current{}; auto current_value = current.value(); return current_value.adjust(value); }\nint auto_dereference_caller(int value) { std::optional<Alias> current; auto current_value = *current; return current_value.adjust(value); }\nint const_auto_dereference_caller(int value) { std::optional<Alias> current; const auto current_value = *current; return current_value.adjust(value); }\nint copied_const_source_dereference_caller(int value) { const std::optional<Alias> current{}; auto current_value = *current; return current_value.adjust(value); }\nint auto_pointer_value_caller(int value) { std::optional<std::shared_ptr<Alias>> current; auto current_value = current.value(); return current_value->adjust(value); }\nint auto_pointer_dereference_caller(int value) { std::optional<std::shared_ptr<Alias>> current; auto current_value = *current; return current_value->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
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
            "api::auto_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_dereference_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_pointer_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_pointer_dereference_caller",
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
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
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
fn resolves_cpp_optional_value_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_value_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint value_alias_caller(int value) { std::optional<Alias> current; auto& alias = current.value(); return alias.adjust(value); }\nint const_value_alias_caller(int value) { const std::optional<Alias> current{}; auto&& alias = current.value(); return alias.adjust(value); }\nint moved_value_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::move(current).value(); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_value_alias_caller",
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
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
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
fn resolves_cpp_optional_dereference_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_dereference_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint dereference_alias_caller(int value) { std::optional<Alias> current; auto& alias = *current; return alias.adjust(value); }\nint const_dereference_alias_caller(int value) { const std::optional<Alias> current{}; auto&& alias = *current; return alias.adjust(value); }\nint moved_dereference_alias_caller(int value) { std::optional<Alias> current; auto&& alias = *std::move(current); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_dereference_alias_caller",
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
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
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
fn resolves_cpp_optional_wrapped_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_wrapped_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint moved_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::move(*current); return alias.adjust(value); }\nint as_const_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::as_const(*current); return alias.adjust(value); }\nint forwarded_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::forward<Alias&&>(*current); return alias.adjust(value); }\nint const_forwarded_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::forward<const Alias&&>(*current); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::as_const_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::forwarded_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_forwarded_alias_caller",
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
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
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
fn resolves_cpp_forwarded_optional_base_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("forwarded_optional_base_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Base { public: int adjust(int value) & { return value; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { std::optional<Derived> current; auto&& alias = std::forward<Base&&>(*current); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"],
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"],
    );
}

#[test]
fn resolves_cpp_cast_optional_base_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("cast_optional_base_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Base { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { std::optional<Derived> current; auto&& alias = static_cast<Base&&>(*current); return alias.adjust(value); }\nint const_caller(int value) { std::optional<Derived> current; auto&& alias = static_cast<const Base&&>(*current); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::caller", "api::Base::adjust(int) &"),
        ("api::const_caller", "api::Base::adjust(int) const &"),
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
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
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
fn resolves_cpp_smart_pointer_dereference_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("smart_pointer_dereference_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint unique_alias_caller(int value) { std::unique_ptr<Alias> current; auto& alias = *current; return alias.adjust(value); }\nint const_shared_alias_caller(int value) { std::shared_ptr<const Alias> current; auto&& alias = *current; return alias.adjust(value); }\nint unique_copy_caller(int value) { std::unique_ptr<Alias> current; auto alias = *current; return alias.adjust(value); }\nint const_shared_copy_caller(int value) { std::shared_ptr<const Alias> current; auto alias = *current; return alias.adjust(value); }\nint unique_get_copy_caller(int value) { std::unique_ptr<Alias> current; auto pointer = current.get(); return pointer->adjust(value); }\nint const_shared_get_copy_caller(int value) { std::shared_ptr<const Alias> current; auto pointer = current.get(); return pointer->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::unique_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::unique_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::unique_get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_get_copy_caller",
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
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
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
fn resolves_cpp_pointer_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("pointer_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint parameter_caller(Alias* current, int value) { return current->adjust(value); }\nint const_parameter_caller(const Alias* current, int value) { return current->adjust(value); }\nint postfix_const_parameter_caller(Alias const* current, int value) { return current->adjust(value); }\nint const_pointer_parameter_caller(Alias* const current, int value) { return current->adjust(value); }\nint pointer_reference_caller(Alias* const& current, int value) { return current->adjust(value); }\nint const_pointer_local_caller(int value) { Alias* const current = nullptr; return current->adjust(value); }\nint local_caller(int value) { Alias* current = nullptr; return current->adjust(value); }\nint dereference_caller(Alias* current, int value) { return (*current).adjust(value); }\nint addressof_local_caller(int value) { Alias current{}; return std::addressof(current)->adjust(value); }\nint addressof_const_local_caller(int value) { const Alias current{}; return std::addressof(current)->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::parameter_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_parameter_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_const_parameter_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointer_parameter_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::pointer_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_pointer_local_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::local_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::addressof_local_caller", "api::Counter::adjust(int) &"),
        (
            "api::addressof_const_local_caller",
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
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
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
fn resolves_cpp_wrapped_pointer_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_pointer_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint moved_parameter_caller(Alias* current, int value) { return std::move(current)->adjust(value); }\nint as_const_parameter_caller(Alias* current, int value) { return std::as_const(current)->adjust(value); }\nint forwarded_const_parameter_caller(const Alias* current, int value) { return std::forward<const Alias*&>(current)->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_parameter_caller", "api::Counter::adjust(int) &"),
        (
            "api::as_const_parameter_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::forwarded_const_parameter_caller",
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
fn resolves_cpp_expected_optional_smart_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_optional_smart_pointer_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_value_get_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return current.error().value().get()->adjust(value); }\nint error_dereference_get_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return (*current.error()).get()->adjust(value); }\nint value_value_get_caller(std::expected<std::optional<std::shared_ptr<Counter>>, Value> current, int value) { return current.value().value().get()->adjust(value); }\nint value_dereference_get_caller(std::expected<std::optional<std::shared_ptr<Counter>>, Value> current, int value) { return (*current.value()).get()->adjust(value); }\nint error_value_arrow_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return current.error().value()->adjust(value); }\nint error_dereference_arrow_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return (*current.error())->adjust(value); }\nint const_error_pointee_caller(std::expected<Value, std::optional<std::shared_ptr<const Counter>>> current, int value) { return (*current.error()).get()->adjust(value); }\nint get_copy_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { auto pointer = current.error().value().get(); return pointer->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
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
fn resolves_cpp_optional_expected_nested_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_expected_nested_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint value_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return current.value().value().adjust(value); }\nint dereference_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return (*current).value().adjust(value); }\nint value_error_caller(std::optional<std::expected<Value, Counter>> current, int value) { return current.value().error().adjust(value); }\nint arrow_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return current->value().adjust(value); }\nint smart_pointer_value_get_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return current.value().value().get()->adjust(value); }\nint smart_pointer_arrow_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return (*current).value()->adjust(value); }\nint nested_optional_value_arrow_caller(std::optional<std::expected<std::optional<Counter>, Value>> current, int value) { return (*current).value()->adjust(value); }\nint nested_optional_value_value_caller(std::optional<std::expected<std::optional<Counter>, Value>> current, int value) { return current.value().value().value().adjust(value); }\nint const_value_value_caller(const std::optional<std::expected<Counter, Value>> current, int value) { return current.value().value().adjust(value); }\nint const_arrow_error_caller(const std::optional<std::expected<Value, Counter>> current, int value) { return current->error().adjust(value); }\nint arrow_error_smart_pointer_get_caller(std::optional<std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return current->error().get()->adjust(value); }\nint arrow_error_smart_pointer_arrow_caller(std::optional<std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return current->error()->adjust(value); }\nint arrow_error_reference_wrapper_caller(std::optional<std::expected<Value, std::reference_wrapper<Counter>>> current, int value) { return current->error().get().adjust(value); }\nint arrow_error_weak_pointer_caller(std::optional<std::expected<Value, std::weak_ptr<Counter>>> current, int value) { return current->error().lock()->adjust(value); }\nint auto_arrow_error_caller(std::optional<std::expected<Value, Counter>> current, int value) { auto nested = current->error(); return nested.adjust(value); }\nint auto_const_arrow_error_caller(const std::optional<std::expected<Value, Counter>> current, int value) { auto nested = current->error(); return nested.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
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
fn resolves_cpp_nested_optional_expected_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("nested_optional_expected_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nint nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return (*current)->value().adjust(value); }\nint nested_opt_opt_exp_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return current.value().value().value().adjust(value); }\nint nested_opt_opt_exp_double_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return current->value()->value().adjust(value); }\nint nested_opt_opt_exp_deref_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return (**current).value().adjust(value); }\nint nested_opt_opt_exp_error_arrow_caller(std::optional<std::optional<std::expected<Value, Counter>>> current, int value) { return (*current)->error().adjust(value); }\nint nested_opt_opt_exp_error_value_caller(std::optional<std::optional<std::expected<Value, Counter>>> current, int value) { return current.value().value().error().adjust(value); }\nint nested_opt_opt_exp_auto_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { auto nested = (*current)->value(); return nested.adjust(value); }\nint moved_nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return std::move(*current)->value().adjust(value); }\nint as_const_nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return std::as_const(*current)->value().adjust(value); }\nint exp_opt_exp_error_caller(std::expected<std::optional<std::expected<Value, Counter>>, Value> current, int value) { return current.value().value().error().adjust(value); }\nint exp_opt_exp_error_arrow_caller(std::expected<std::optional<std::expected<Value, Counter>>, Value> current, int value) { return (*current)->error().adjust(value); }\nint opt_exp_error_opt_sp_arrow_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { return current->error()->adjust(value); }\nint opt_exp_error_opt_sp_get_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { return current->error().value().get()->adjust(value); }\nint opt_exp_error_opt_weak_arrow_caller(std::optional<std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { return current->error()->lock()->adjust(value); }\nint opt_exp_error_opt_ref_get_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { return current->error()->get().adjust(value); }\nint opt_exp_opt_exp_error_caller(std::optional<std::expected<std::optional<std::expected<Value, Counter>>, Value>> current, int value) { return current->value()->error().adjust(value); }\nint exp_error_opt_exp_value_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { return current.error()->value().adjust(value); }\nint exp_error_opt_exp_arrow_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { return (*current.error())->adjust(value); }\nint auto_opt_exp_error_opt_sp_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { auto nested = current->error(); return nested->adjust(value); }\nint decltype_auto_exp_error_opt_exp_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { decltype(auto) nested = (*current.error())->value(); return nested.adjust(value); }\nint decltype_auto_opt_exp_error_opt_sp_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) nested = current->error(); return nested->adjust(value); }\nint decltype_auto_exp_error_opt_exp_arrow_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { decltype(auto) nested = (*current.error()); return nested->adjust(value); }\nint decltype_auto_opt_exp_error_opt_weak_lock_caller(std::optional<std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { decltype(auto) nested = current->error()->lock(); return nested->adjust(value); }\nint decltype_auto_opt_exp_value_sp_get_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { decltype(auto) pointer = current->value().get(); return pointer->adjust(value); }\nint decltype_auto_const_opt_exp_error_opt_sp_get_caller(const std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) pointer = current->error().value().get(); return pointer->adjust(value); }\nint decltype_auto_opt_exp_error_opt_sp_get_arrow_caller(std::optional<std::expected<Value, std::optional<std::shared_ptr<Counter>>>> current, int value) { decltype(auto) pointer = current->error()->get(); return pointer->adjust(value); }\nint decltype_auto_opt_opt_sp_arrow_caller(std::optional<std::optional<std::unique_ptr<Counter>>> current, int value) { decltype(auto) nested = *current; return nested->adjust(value); }\nint decltype_auto_opt_exp_error_opt_sp_deref_arrow_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) nested = *current->error(); return nested->adjust(value); }\nint auto_opt_exp_error_opt_ref_via_nested_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { auto nested = current->error(); return nested->get().adjust(value); }\nint decltype_auto_opt_exp_error_opt_ref_via_nested_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { decltype(auto) nested = current->error(); return nested->get().adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
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
            "api::nested_opt_opt_exp_error_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_auto_value_caller",
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
            "api::opt_exp_error_opt_sp_get_caller",
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
fn resolves_cpp_expected_optional_reference_wrapper_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_optional_reference_wrapper_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_value_caller(std::expected<Value, std::optional<std::reference_wrapper<Counter>>> current, int value) { return current.error().value().get().adjust(value); }\nint error_dereference_caller(std::expected<Value, std::optional<std::reference_wrapper<Counter>>> current, int value) { return (*current.error()).get().adjust(value); }\nint value_value_caller(std::expected<std::optional<std::reference_wrapper<Counter>>, Value> current, int value) { return current.value().value().get().adjust(value); }\nint value_dereference_caller(std::expected<std::optional<std::reference_wrapper<Counter>>, Value> current, int value) { return (*current.value()).get().adjust(value); }\nint const_error_pointee_caller(std::expected<Value, std::optional<std::reference_wrapper<const Counter>>> current, int value) { return (*current.error()).get().adjust(value); }\nint get_copy_caller(std::expected<Value, std::optional<std::reference_wrapper<Counter>>> current, int value) { auto target = current.error().value().get(); return target.adjust(value); }\nint dereference_get_copy_caller(std::expected<std::optional<std::reference_wrapper<Counter>>, Value> current, int value) { auto target = (*current.value()).get(); return target.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
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
        (
            "api::dereference_get_copy_caller",
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
fn resolves_cpp_expected_optional_weak_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_optional_weak_pointer_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_value_caller(std::expected<Value, std::optional<std::weak_ptr<Counter>>> current, int value) { return current.error().value().lock()->adjust(value); }\nint error_dereference_caller(std::expected<Value, std::optional<std::weak_ptr<Counter>>> current, int value) { return (*current.error()).lock()->adjust(value); }\nint value_value_caller(std::expected<std::optional<std::weak_ptr<Counter>>, Value> current, int value) { return current.value().value().lock()->adjust(value); }\nint value_dereference_caller(std::expected<std::optional<std::weak_ptr<Counter>>, Value> current, int value) { return (*current.value()).lock()->adjust(value); }\nint const_error_pointee_caller(std::expected<Value, std::optional<std::weak_ptr<const Counter>>> current, int value) { return (*current.error()).lock()->adjust(value); }\nint lock_copy_caller(std::expected<Value, std::optional<std::weak_ptr<Counter>>> current, int value) { auto shared = current.error().value().lock(); return shared->adjust(value); }\nint dereference_lock_copy_caller(std::expected<std::optional<std::weak_ptr<Counter>>, Value> current, int value) { auto shared = (*current.value()).lock(); return shared->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
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
        (
            "api::dereference_lock_copy_caller",
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
fn resolves_cpp_optional_reference_wrapper_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_reference_wrapper_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint value_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { return current.value().get().adjust(value); }\nint dereference_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { return (*current).get().adjust(value); }\nint moved_value_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { return std::move(current).value().get().adjust(value); }\nint const_pointee_caller(std::optional<std::reference_wrapper<const Counter>> current, int value) { return (*current).get().adjust(value); }\nint get_alias_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { auto& target = (*current).get(); return target.adjust(value); }\nint get_copy_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { auto target = current.value().get(); return target.adjust(value); }\nint const_get_copy_caller(std::optional<std::reference_wrapper<const Counter>> current, int value) { auto target = (*current).get(); return target.adjust(value); }\nint const_auto_get_copy_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { const auto target = current.value().get(); return target.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::get_alias_caller", "api::Counter::adjust(int) &"),
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
fn resolves_cpp_optional_weak_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_weak_pointer_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint value_caller(std::optional<std::weak_ptr<Counter>> current, int value) { return current.value().lock()->adjust(value); }\nint dereference_caller(std::optional<std::weak_ptr<Counter>> current, int value) { return (*current).lock()->adjust(value); }\nint moved_value_caller(std::optional<std::weak_ptr<Counter>> current, int value) { return std::move(current).value().lock()->adjust(value); }\nint const_pointee_caller(std::optional<std::weak_ptr<const Counter>> current, int value) { return (*current).lock()->adjust(value); }\nint lock_copy_caller(std::optional<std::weak_ptr<Counter>> current, int value) { auto shared = current.value().lock(); return shared->adjust(value); }\nint dereference_lock_copy_caller(std::optional<std::weak_ptr<Counter>> current, int value) { auto shared = (*current).lock(); return shared->adjust(value); }\nint const_lock_copy_caller(std::optional<std::weak_ptr<const Counter>> current, int value) { auto shared = (*current).lock(); return shared->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &"),
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
fn resolves_cpp_optional_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nusing Alias = Counter;\nint arrow_caller(int value) { std::optional<Alias> current; return current->adjust(value); }\nint auto_arrow_caller(int value) { auto current = std::optional<Alias>{}; return current->adjust(value); }\nint auto_const_arrow_caller(int value) { const auto current = std::optional<Alias>{}; return current->adjust(value); }\nint nested_unique_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return (*current)->adjust(value); }\nint nested_unique_value_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return current.value()->adjust(value); }\nint moved_arrow_caller(int value) { std::optional<Alias> current; return std::move(current)->adjust(value); }\nint as_const_arrow_caller(int value) { std::optional<Alias> current; return std::as_const(current)->adjust(value); }\nint forwarded_arrow_caller(int value) { std::optional<Alias> current; return std::forward<std::optional<Alias>&&>(current)->adjust(value); }\nint value_caller(int value) { std::optional<Alias> current; return current.value().adjust(value); }\nint dereference_caller(int value) { std::optional<Alias> current; return (*current).adjust(value); }\nint moved_value_caller(int value) { std::optional<Alias> current; return std::move(current).value().adjust(value); }\nint moved_dereference_caller(int value) { std::optional<Alias> current; return (*std::move(current)).adjust(value); }\nint as_const_value_caller(int value) { std::optional<Alias> current; return std::as_const(current).value().adjust(value); }\nint as_const_dereference_caller(int value) { std::optional<Alias> current; return (*std::as_const(current)).adjust(value); }\nint forwarded_value_caller(int value) { std::optional<Alias> current; return std::forward<std::optional<Alias>&&>(current).value().adjust(value); }\nint forwarded_dereference_caller(int value) { std::optional<Alias> current; return (*std::forward<std::optional<Alias>&&>(current)).adjust(value); }\nint const_arrow_caller(int value) { const std::optional<Alias> current{}; return current->adjust(value); }\nint const_value_caller(int value) { const std::optional<Alias> current{}; return current.value().adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::arrow_caller", "api::Counter::adjust(int) &"),
        ("api::auto_arrow_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_const_arrow_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::nested_unique_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_unique_value_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::moved_arrow_caller", "api::Counter::adjust(int) &"),
        (
            "api::as_const_arrow_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::forwarded_arrow_caller", "api::Counter::adjust(int) &"),
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &&"),
        (
            "api::moved_dereference_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::as_const_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::as_const_dereference_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarded_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::forwarded_dereference_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::const_arrow_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_value_caller",
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
