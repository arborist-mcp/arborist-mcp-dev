use super::*;

#[test]
fn resolves_cpp_using_declaration_overloads_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("using_overloads.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nnamespace integral { int convert(int value) { return value; } }\nnamespace decimal { double convert(double left, double right) { return left + right; } }\nusing integral::convert;\nusing decimal::convert;\nint caller() { return api::convert(1); }\ndouble decimal_caller() { return api::convert(1.0, 2.0); }\n}\n",
    )
    .unwrap();

    let expected_integer_callee = "api::integral::convert(int)";
    let expected_decimal_callee = "api::decimal::convert(double,double)";
    for (symbol_path, expected_callee) in [
        ("api::caller", expected_integer_callee),
        ("api::decimal_caller", expected_decimal_callee),
    ] {
        let trace = trace_symbol_graph(&dir, symbol_path, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee]
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (symbol_path, expected_callee) in [
        ("api::caller", expected_integer_callee),
        ("api::decimal_caller", expected_decimal_callee),
    ] {
        let trace =
            trace_symbol_graph_from_index(&db_path, symbol_path, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee]
        );
    }
}

#[test]
fn resolves_cpp_const_cast_this_member_calls_to_const_rvalue_ref_overloads() {
    let dir = temporary_dir();
    let source = dir.join("const_cast_this_ref_qualified_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) const && { return value + 1; }\n    int adjust(int value) && { return value; }\n    int caller(int value) && { return static_cast<const Counter&&>(*this).adjust(value); }\n};\n}\n",
    )
    .unwrap();

    let expected_callee = "api::Counter::adjust(int) const &&";
    let trace = trace_symbol_graph(&dir, "api::Counter::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee],
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Counter::caller", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee],
    );
}

#[test]
fn resolves_cpp_const_cast_this_member_calls_to_const_lvalue_ref_overloads() {
    let dir = temporary_dir();
    let source = dir.join("const_cast_this_lvalue_ref_qualified_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) & { return value; }\n    int caller(int value) { return static_cast<Counter const &>(*this).adjust(value); }\n};\n}\n",
    )
    .unwrap();

    let expected_callee = "api::Counter::adjust(int) const &";
    let trace = trace_symbol_graph(&dir, "api::Counter::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee],
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Counter::caller", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee],
    );
}

#[test]
fn resolves_cpp_as_const_this_member_calls_to_const_lvalue_ref_overloads() {
    let dir = temporary_dir();
    let source = dir.join("as_const_this_lvalue_ref_qualified_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) & { return value; }\n    int caller(int value) { return std::as_const(*this).adjust(value); }\n};\n}\n",
    )
    .unwrap();

    let expected_callee = "api::Counter::adjust(int) const &";
    let trace = trace_symbol_graph(&dir, "api::Counter::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee],
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Counter::caller", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee],
    );
}

#[test]
fn resolves_cpp_temporary_member_calls_to_rvalue_ref_overloads() {
    let dir = temporary_dir();
    let source = dir.join("temporary_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) && { return value + 1; }\n    int adjust(int value) const & { return value + 2; }\n    int adjust(int value) const && { return value + 3; }\n};\nusing Alias = Counter;\nusing Second = Alias;\nint caller(int value) { return api::Counter{}.adjust(value); }\nint alias_caller(int value) { return Alias{}.adjust(value); }\nint chained_alias_caller(int value) { return Second{}.adjust(value); }\nint moved_caller(int value) { return std::move(api::Counter{}).adjust(value); }\nint cast_rvalue_caller(int value) { return static_cast<Counter&&>(Counter{}).adjust(value); }\nint cast_const_lvalue_caller(int value) { return static_cast<Counter const &>(Counter{}).adjust(value); }\nint cast_const_rvalue_caller(int value) { return static_cast<const Counter&&>(Counter{}).adjust(value); }\nint forward_rvalue_caller(int value) { return std::forward<Counter>(Counter{}).adjust(value); }\nint forward_const_lvalue_caller(int value) { return std::forward<Counter const &>(Counter{}).adjust(value); }\nint forward_const_rvalue_caller(int value) { return std::forward<const Counter&&>(Counter{}).adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::caller", "api::Counter::adjust(int) &&"),
        ("api::alias_caller", "api::Counter::adjust(int) &&"),
        ("api::chained_alias_caller", "api::Counter::adjust(int) &&"),
        ("api::moved_caller", "api::Counter::adjust(int) &&"),
        ("api::cast_rvalue_caller", "api::Counter::adjust(int) &&"),
        (
            "api::cast_const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::cast_const_rvalue_caller",
            "api::Counter::adjust(int) const &&",
        ),
        ("api::forward_rvalue_caller", "api::Counter::adjust(int) &&"),
        (
            "api::forward_const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forward_const_rvalue_caller",
            "api::Counter::adjust(int) const &&",
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
