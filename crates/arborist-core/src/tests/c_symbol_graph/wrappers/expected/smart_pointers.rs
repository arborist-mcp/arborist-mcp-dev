use super::*;

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
