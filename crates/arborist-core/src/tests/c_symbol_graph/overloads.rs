use super::*;

#[test]
fn indexes_cpp_using_declaration_overload_sets_per_scope() {
    let dir = temporary_dir();
    let source = dir.join("imports.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nnamespace integral { int convert(int value) { return value; } }\nnamespace decimal { double convert(double value) { return value; } }\nusing integral::convert;\nusing decimal::convert;\n\nclass IntegerReset { public: void reset(int value) {} };\nclass DecimalReset { public: void reset(double value) {} };\nclass Resettable : IntegerReset, DecimalReset { public: using IntegerReset::reset; using DecimalReset::reset; };\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let imported_symbols = skeleton
        .available_symbols
        .iter()
        .filter(|symbol| symbol.semantic_path == "api::convert")
        .collect::<Vec<_>>();
    assert_eq!(imported_symbols.len(), 2, "{imported_symbols:#?}");
    assert_eq!(
        imported_symbols
            .iter()
            .map(|symbol| symbol.signature.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some("using integral::convert;"),
            Some("using decimal::convert;")
        ]
    );
    let imported_methods = skeleton
        .available_symbols
        .iter()
        .filter(|symbol| symbol.semantic_path == "api::Resettable::reset")
        .collect::<Vec<_>>();
    assert_eq!(imported_methods.len(), 2, "{imported_methods:#?}");
    assert_eq!(
        imported_methods
            .iter()
            .map(|symbol| symbol.signature.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some("using IntegerReset::reset;"),
            Some("using DecimalReset::reset;")
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::convert", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.node_kind, "using_declaration");
    assert_eq!(persisted_trace.symbol.scope_path.as_deref(), Some("api"));
    let persisted_method =
        trace_symbol_graph_from_index(&db_path, "api::Resettable::reset", TraceDirection::Both)
            .unwrap();
    assert_eq!(persisted_method.symbol.node_kind, "using_declaration");
    assert_eq!(
        persisted_method.symbol.scope_path.as_deref(),
        Some("api::Resettable")
    );
}

#[test]
fn resolves_cpp_const_member_calls_to_const_overloads() {
    let dir = temporary_dir();
    let source = dir.join("const_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) const { return value + 1; }\n    int adjust(int value) { return value; }\n    int caller(int value) const { return this->adjust(value); }\n};\n}\n",
    )
    .unwrap();

    let expected_callee = "api::Counter::adjust(int) const";
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
fn resolves_cpp_this_member_calls_to_lvalue_ref_overloads() {
    let dir = temporary_dir();
    let source = dir.join("ref_qualified_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) && { return value + 1; }\n    int caller(int value) { return this->adjust(value); }\n};\n}\n",
    )
    .unwrap();

    let expected_callee = "api::Counter::adjust(int) &";
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
fn resolves_cpp_const_this_member_calls_to_lvalue_ref_overloads() {
    let dir = temporary_dir();
    let source = dir.join("const_ref_qualified_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) const & { return value; }\n    int adjust(int value) const && { return value + 1; }\n    int caller(int value) const { return this->adjust(value); }\n};\n}\n",
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
fn resolves_cpp_moved_this_member_calls_to_rvalue_ref_overloads() {
    let dir = temporary_dir();
    let source = dir.join("moved_this_ref_qualified_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) && { return value + 1; }\n    int adjust(int value) & { return value; }\n    int caller(int value) && { return std::move(*this).adjust(value); }\n};\n}\n",
    )
    .unwrap();

    let expected_callee = "api::Counter::adjust(int) &&";
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
fn resolves_cpp_rvalue_this_calls_with_sparse_const_ref_qualified_overloads() {
    let dir = temporary_dir();
    let source = dir.join("sparse_const_rvalue_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const && { return value + 1; }\n    int caller(int value) && { return std::move(*this).adjust(value); }\n};\n}\n",
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
fn resolves_cpp_cast_this_member_calls_to_rvalue_ref_overloads() {
    let dir = temporary_dir();
    let source = dir.join("cast_this_ref_qualified_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) && { return value + 1; }\n    int adjust(int value) & { return value; }\n    int caller(int value) && { return static_cast< Counter && >(*this).adjust(value); }\n};\n}\n",
    )
    .unwrap();

    let expected_callee = "api::Counter::adjust(int) &&";
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

#[test]
fn distinguishes_cpp_function_overloads_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("convert.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nint convert(int value) { return value; }\ndouble convert(double value) { return value; }\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let mut overload_ids = skeleton
        .available_symbols
        .iter()
        .filter(|symbol| symbol.semantic_path == "api::convert")
        .map(|symbol| symbol.symbol_id.clone())
        .collect::<Vec<_>>();
    overload_ids.sort();
    assert_eq!(
        overload_ids,
        vec!["api::convert(double)", "api::convert(int)"]
    );

    let live_list = list_symbols(&dir, 20).unwrap();
    let mut live_overload_ids = live_list
        .symbols
        .iter()
        .filter(|symbol| symbol.semantic_path == "api::convert")
        .map(|symbol| symbol.symbol_id.clone())
        .collect::<Vec<_>>();
    live_overload_ids.sort();
    assert_eq!(live_overload_ids, overload_ids);

    let live_int = trace_symbol_graph(&dir, "api::convert(int)", TraceDirection::Both).unwrap();
    let live_double =
        trace_symbol_graph(&dir, "api::convert(double)", TraceDirection::Both).unwrap();
    assert_eq!(live_int.symbol.return_type.as_deref(), Some("int"));
    assert_eq!(live_double.symbol.return_type.as_deref(), Some("double"));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_list = list_symbols_from_index(&db_path, 20).unwrap();
    let mut persisted_overload_ids = persisted_list
        .symbols
        .iter()
        .filter(|symbol| symbol.semantic_path == "api::convert")
        .map(|symbol| symbol.symbol_id.clone())
        .collect::<Vec<_>>();
    persisted_overload_ids.sort();
    assert_eq!(persisted_overload_ids, overload_ids);

    let persisted_int =
        trace_symbol_graph_from_index(&db_path, "api::convert(int)", TraceDirection::Both).unwrap();
    let persisted_double =
        trace_symbol_graph_from_index(&db_path, "api::convert(double)", TraceDirection::Both)
            .unwrap();
    assert_eq!(persisted_int.symbol.return_type.as_deref(), Some("int"));
    assert_eq!(
        persisted_double.symbol.return_type.as_deref(),
        Some("double")
    );
    assert_eq!(
        read_symbol_from_index(&db_path, "api::convert(int)")
            .unwrap()
            .symbol
            .return_type
            .as_deref(),
        Some("int")
    );
}

#[test]
fn resolves_cpp_direct_calls_to_overloads_by_argument_count_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("overloads.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nint convert(int value) { return value; }\nint convert(int left, int right) { return left + right; }\nint convert(int first, int second, int third) { return first + second + third; }\nint one() { return convert(1); }\nint two() { return convert(1, 2); }\nint three() { return convert(1, 2, 3); }\n}\n",
    )
    .unwrap();

    for (caller, callee) in [
        ("api::one", "api::convert(int)"),
        ("api::two", "api::convert(int,int)"),
        ("api::three", "api::convert(int,int,int)"),
    ] {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![callee]
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, callee) in [
        ("api::one", "api::convert(int)"),
        ("api::two", "api::convert(int,int)"),
        ("api::three", "api::convert(int,int,int)"),
    ] {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![callee]
        );
    }
}

#[test]
fn resolves_cpp_qualified_overload_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("qualified_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace alpha {\nint convert(int value) { return value; }\nnamespace beta {\nint convert(int value) { return value + 1; }\nint convert(int left, int right) { return left + right; }\n}\nint caller() { return beta::convert(1); }\n}\n",
    )
    .unwrap();

    let expected_callee = "alpha::beta::convert(int)";
    let trace = trace_symbol_graph(&dir, "alpha::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "alpha::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
}
