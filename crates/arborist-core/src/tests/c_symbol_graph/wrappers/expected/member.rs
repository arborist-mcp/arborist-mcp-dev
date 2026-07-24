use super::*;

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
