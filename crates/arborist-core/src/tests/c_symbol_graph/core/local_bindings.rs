use super::*;

#[test]
fn resolves_cpp_local_variable_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("local_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nusing Alias = Counter;\nint lvalue_caller(int value) { Alias current{}; return current.adjust(value); }\nint const_lvalue_caller(int value) { const Alias current{}; return current.adjust(value); }\nint postfix_const_caller(int value) { Alias const current{}; return current.adjust(value); }\nint static_caller(int value) { static Alias current{}; return current.adjust(value); }\nint static_const_caller(int value) { static const Alias current{}; return current.adjust(value); }\nint moved_caller(int value) { Alias current{}; return std::move(current).adjust(value); }\nint shadowed_caller(int value) { Alias current{}; { const Alias current{}; return current.adjust(value); } }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::lvalue_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::static_caller", "api::Counter::adjust(int) &"),
        (
            "api::static_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::moved_caller", "api::Counter::adjust(int) &&"),
        ("api::shadowed_caller", "api::Counter::adjust(int) const &"),
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
fn resolves_cpp_range_for_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("range_for_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint caller() { for (Alias current : values) { return current.adjust(1); } return 0; }\nint const_caller() { for (const Alias current : values) { return current.adjust(1); } return 0; }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::caller", "api::Counter::adjust(int) &"),
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
fn resolves_cpp_condition_binding_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("condition_binding_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    operator bool() const { return true; }\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nAlias make_counter() { return Alias{}; }\nint if_caller(int value) { if (Alias current = make_counter()) { return current.adjust(value); } else { return current.adjust(value); } }\nint const_switch_caller(int value) { switch (const Alias current = make_counter()) { default: return current.adjust(value); } }\nint while_caller(int value) { while (Alias current = make_counter()) { return current.adjust(value); } return value; }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::if_caller",
            ["api::Counter::adjust(int) &", "api::make_counter()"],
        ),
        (
            "api::const_switch_caller",
            ["api::Counter::adjust(int) const &", "api::make_counter()"],
        ),
        (
            "api::while_caller",
            ["api::Counter::adjust(int) &", "api::make_counter()"],
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
            expected_callee,
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
            expected_callee,
            "{caller}",
        );
    }
}

#[test]
fn resolves_cpp_parameter_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("parameter_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nusing Alias = Counter;\nint lvalue_caller(Alias& current, int value) { return current.adjust(value); }\nint const_lvalue_caller(const Alias& current, int value) { return current.adjust(value); }\nint postfix_const_lvalue_caller(Alias const& current, int value) { return current.adjust(value); }\nint rvalue_reference_caller(Alias&& current, int value) { return current.adjust(value); }\nint moved_rvalue_reference_caller(Alias&& current, int value) { return std::move(current).adjust(value); }\nint moved_caller(Alias& current, int value) { return std::move(current).adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::lvalue_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_const_lvalue_caller",
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
        ("api::moved_caller", "api::Counter::adjust(int) &&"),
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
fn preserves_cpp_callable_identity_for_qualifiers_and_declarator_shapes() {
    let dir = temporary_dir();
    let header = dir.join("counter.hpp");
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "namespace api {\nint helper();\nclass Counter { public: int value() const; };\nvoid reset(void);\nint transform(int values[3][4], void (*callback)(int code));\n}\n",
    )
    .unwrap();
    fs::write(
        &source,
        "#include \"counter.hpp\"\nnamespace api {\nint helper() { return 1; }\nint Counter::value() const { return helper(); }\nvoid reset() {}\nint transform(int buffer[3][4], void (*handler)(int error)) { handler(buffer[0][0]); return buffer[0][0]; }\n}\n",
    )
    .unwrap();

    let exact_ids = [
        "api::Counter::value() const",
        "api::reset()",
        "api::transform(int[3][4],void(*)(int))",
    ];
    for exact_id in exact_ids {
        let live = trace_symbol_graph(&dir, exact_id, TraceDirection::Both).unwrap();
        assert_eq!(live.symbol.symbol_id, exact_id);
        assert_eq!(
            live.symbol.file_path,
            source.to_string_lossy().replace('\\', "/")
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for exact_id in exact_ids {
        let persisted =
            trace_symbol_graph_from_index(&db_path, exact_id, TraceDirection::Both).unwrap();
        assert_eq!(persisted.symbol.symbol_id, exact_id);
        assert_eq!(
            persisted.symbol.file_path,
            source.to_string_lossy().replace('\\', "/")
        );
    }
}
