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
