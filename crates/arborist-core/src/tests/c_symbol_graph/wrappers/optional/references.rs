use super::*;

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
