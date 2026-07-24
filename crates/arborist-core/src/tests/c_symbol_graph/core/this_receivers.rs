use super::*;

#[test]
fn resolves_cpp_this_member_calls_by_arity_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("this_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) { return value; }\n    double adjust(double left, double right) { return left + right; }\n    int caller(int value) { return this->adjust(value); }\n    int dereferenced_caller(int value) { return (*this).adjust(value); }\n};\n}\n",
    )
    .unwrap();

    let expected_callee = "api::Counter::adjust(int)";
    for symbol_path in ["api::Counter::caller", "api::Counter::dereferenced_caller"] {
        let trace = trace_symbol_graph(&dir, symbol_path, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
            "{symbol_path}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for symbol_path in ["api::Counter::caller", "api::Counter::dereferenced_caller"] {
        let trace =
            trace_symbol_graph_from_index(&db_path, symbol_path, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
            "{symbol_path}",
        );
    }
}

#[test]
fn resolves_cpp_parenthesized_and_nested_this_receivers_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("nested_this_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) && { return value + 1; }\n    int adjust(int value) const & { return value + 2; }\n    int adjust(int value) const && { return value + 3; }\n    int parenthesized_caller(int value) { return (((*this))).adjust(value); }\n    int moved_caller(int value) { return (std::move(static_cast<Counter &>(*this))).adjust(value); }\n    int const_moved_caller(int value) { return std::move(std::as_const(((*this)))).adjust(value); }\n    int forwarded_caller(int value) { return ((std::forward<Counter const &>(((*this))))).adjust(value); }\n};\n}\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::Counter::parenthesized_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::Counter::moved_caller", "api::Counter::adjust(int) &&"),
        (
            "api::Counter::const_moved_caller",
            "api::Counter::adjust(int) const &&",
        ),
        (
            "api::Counter::forwarded_caller",
            "api::Counter::adjust(int) const &",
        ),
    ];
    for (symbol_path, expected_callee) in expected_callees {
        let trace = trace_symbol_graph(&dir, symbol_path, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
            "{symbol_path}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (symbol_path, expected_callee) in expected_callees {
        let trace =
            trace_symbol_graph_from_index(&db_path, symbol_path, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
            "{symbol_path}",
        );
    }
}

#[test]
fn resolves_cpp_forward_this_member_calls_with_value_categories() {
    let dir = temporary_dir();
    let source = dir.join("forward_this_ref_qualified_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) const & { return value + 3; }\n    int adjust(int value) & { return value + 2; }\n    int adjust(int value) const && { return value + 1; }\n    int adjust(int value) && { return value; }\n    int rvalue_caller(int value) { return std::forward<Counter>(*this).adjust(value); }\n    int const_lvalue_caller(int value) { return std::forward<Counter const &>(*this).adjust(value); }\n};\n}\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::Counter::rvalue_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::Counter::const_lvalue_caller",
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
        );
    }
}
