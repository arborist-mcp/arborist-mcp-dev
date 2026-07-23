use super::*;

#[test]
fn traces_nested_cpp_namespace_functions_with_scope_aware_resolution() {
    let dir = temporary_dir();
    let header = dir.join("api.hpp");
    let source = dir.join("api.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &header,
        "namespace alpha::detail {\nint helper(int value);\nint orchestrate(int value);\n}\n\nnamespace beta {\nint helper(int value);\n}\n",
    )
    .unwrap();
    fs::write(
        &source,
        "#include \"api.hpp\"\n\nnamespace alpha::detail {\nint helper(int value) {\n    return value + 1;\n}\n\nint orchestrate(int value) {\n    return helper(value);\n}\n}\n\nnamespace beta {\nint helper(int value) {\n    return value + 2;\n}\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "alpha::detail::orchestrate")
    );
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "beta::helper")
    );

    let trace =
        trace_symbol_graph(&dir, "alpha::detail::orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "alpha::detail::helper");
    assert_eq!(
        trace.callees[0].scope_path.as_deref(),
        Some("alpha::detail")
    );

    let stats = rebuild_symbol_index(&dir, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 2);
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "alpha::detail::orchestrate", TraceDirection::Both)
            .unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(
        persisted_trace.callees[0].semantic_path,
        "alpha::detail::helper"
    );
}

#[test]
fn traces_cpp_inline_friend_functions_in_enclosing_namespace() {
    let dir = temporary_dir();
    let source = dir.join("token.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Token {\n    friend int inspect(const Token&) { return 1; }\n};\n\nint orchestrate(const Token& token) { return inspect(token); }\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::inspect")
    );
    assert!(
        !skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Token::inspect")
    );

    let trace = trace_symbol_graph(&dir, "api::orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "api::inspect");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(persisted_trace.callees[0].semantic_path, "api::inspect");
}

#[test]
fn traces_cpp_using_aliases() {
    let dir = temporary_dir();
    let source = dir.join("aliases.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nusing Size = unsigned long;\n\nclass Config {\npublic:\n    using Count = int;\n};\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Size")
    );
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Config::Count")
    );

    let trace = trace_symbol_graph(&dir, "api::Config::Count", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Config::Count", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_cpp_using_declarations() {
    let dir = temporary_dir();
    let source = dir.join("imports.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nnamespace base { int convert(int value) { return value + 1; } }\nusing base::convert;\n\nclass Base { protected: void reset() {} };\nclass Derived : Base { public: using Base::reset; };\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let imported_function = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::convert")
        .expect("namespace using declaration should be indexed");
    assert_eq!(imported_function.node_kind, "using_declaration");
    assert_eq!(imported_function.scope_path.as_deref(), Some("api"));
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Derived::reset")
    );

    let trace = trace_symbol_graph(&dir, "api::convert", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.node_kind, "using_declaration");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Derived::reset", TraceDirection::Both)
            .unwrap();
    assert_eq!(persisted_trace.symbol.node_kind, "using_declaration");
    assert_eq!(
        persisted_trace.symbol.scope_path.as_deref(),
        Some("api::Derived")
    );
}

#[test]
fn traces_cpp_namespace_aliases() {
    let dir = temporary_dir();
    let source = dir.join("aliases.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nnamespace vendor = third_party::vendor;\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let alias = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::vendor")
        .expect("namespace alias should be indexed");
    assert_eq!(alias.node_kind, "namespace_alias_definition");
    assert_eq!(alias.scope_path.as_deref(), Some("api"));

    let trace = trace_symbol_graph(&dir, "api::vendor", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::vendor", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.scope_path.as_deref(), Some("api"));
}

#[test]
fn resolves_cpp_namespace_alias_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("alias_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nnamespace implementation {\nint convert(int value) { return value; }\n}\nnamespace detail = implementation;\nnamespace vendor = detail;\nint caller() { return vendor::convert(1); }\n}\n",
    )
    .unwrap();

    let expected_callee = "api::implementation::convert(int)";
    let trace = trace_symbol_graph(&dir, "api::caller", TraceDirection::Both).unwrap();
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
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
}

#[test]
fn resolves_cpp_qualified_namespace_aliases_from_local_headers() {
    let dir = temporary_dir();
    let header = dir.join("imports.hpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "namespace implementation { int convert(int value) { return value; } }\nnamespace detail = implementation;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"imports.hpp\"\nint caller() { return detail::convert(1); }\n",
    )
    .unwrap();

    let expected_callee = "implementation::convert(int)";
    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
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
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
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
fn resolves_cpp_using_declaration_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("using_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nnamespace implementation {\nint convert(int value) { return value; }\n}\nnamespace detail = implementation;\nusing detail::convert;\ndouble convert(double left, double right) { return left + right; }\nint caller() { return api::convert(1); }\ndouble decimal_caller() { return api::convert(1.0, 2.0); }\n}\n",
    )
    .unwrap();

    let expected_callee = "api::implementation::convert(int)";
    let trace = trace_symbol_graph(&dir, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
    let expected_local_callee = "api::convert(double,double)";
    let decimal_trace =
        trace_symbol_graph(&dir, "api::decimal_caller", TraceDirection::Both).unwrap();
    assert_eq!(
        decimal_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_local_callee]
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
        vec![expected_callee]
    );
    let persisted_decimal_trace =
        trace_symbol_graph_from_index(&db_path, "api::decimal_caller", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_decimal_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_local_callee]
    );
}

#[test]
fn resolves_unqualified_cpp_using_declarations_from_local_headers() {
    let dir = temporary_dir();
    let header = dir.join("imports.hpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "namespace vendor { int convert(int value) { return value; } }\nnamespace app { using vendor::convert; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"imports.hpp\"\nnamespace app { int caller() { return convert(1); } }\n",
    )
    .unwrap();

    let expected_callee = "vendor::convert(int)";
    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
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
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
}

#[test]
fn resolves_unqualified_cpp_using_namespaces_from_local_headers() {
    let dir = temporary_dir();
    let header = dir.join("imports.hpp");
    let caller = dir.join("caller.cpp");
    fs::write(
        &header,
        "namespace vendor { int convert(int value) { return value; } }\nnamespace app { using namespace vendor; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"imports.hpp\"\nnamespace app { int caller() { return convert(1); } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["vendor::convert(int)"]
    );
}

#[test]
fn does_not_resolve_cpp_using_declarations_from_headers_included_after_callers() {
    let dir = temporary_dir();
    let header = dir.join("imports.hpp");
    let caller = dir.join("caller.cpp");
    fs::write(
        &header,
        "namespace vendor { int convert(int value) { return value; } }\nnamespace app { using vendor::convert; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace app { int caller() { return convert(1); } }\n#include \"imports.hpp\"\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_unindexed_cpp_using_declaration_call_targets() {
    let dir = temporary_dir();
    let source = dir.join("using_external.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nusing external::convert;\nint caller() { return api::convert(1); }\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "api::caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn retains_unindexed_cpp_using_declaration_noncall_references() {
    let dir = temporary_dir();
    let source = dir.join("using_external_reference.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nusing external::convert;\nvoid callback() { auto target = &api::convert; }\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "api::callback", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.semantic_path.as_str())
            .collect::<Vec<_>>(),
        vec!["api::convert"]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::callback", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.semantic_path.as_str())
            .collect::<Vec<_>>(),
        vec!["api::convert"]
    );
}

#[test]
fn resolves_cpp_using_namespace_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let definitions = dir.join("definitions.cpp");
    let caller = dir.join("caller.cpp");
    let global_caller = dir.join("global_caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "double convert(double left, double right) { return left + right; }\nnamespace api { namespace vendor { int convert(int value) { return value + 1; } } }\n",
    )
    .unwrap();
    fs::write(
        &global_caller,
        "using namespace api::vendor;\nint global_caller() { return convert(1); }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace api {\nnamespace alias = vendor;\nint before_import() { return convert(1); }\nusing namespace alias;\ndouble convert(double left, double right) { return left + right; }\nint caller() { return convert(1); }\ndouble decimal_caller() { return convert(1.0, 2.0); }\n}\n",
    )
    .unwrap();

    let caller_source = fs::read_to_string(&caller).unwrap();
    let skeleton = get_semantic_skeleton(&caller, &caller_source, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.signature.as_deref() == Some("using namespace alias;")),
        "{:#?}",
        skeleton.available_symbols
    );

    let expected_callee = "api::vendor::convert(int)";
    let before_import =
        trace_symbol_graph(&dir, "api::before_import", TraceDirection::Both).unwrap();
    assert!(before_import.callees.is_empty());
    let trace = trace_symbol_graph(&dir, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
    let decimal_trace =
        trace_symbol_graph(&dir, "api::decimal_caller", TraceDirection::Both).unwrap();
    assert_eq!(
        decimal_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::convert(double,double)"]
    );
    let global_trace = trace_symbol_graph(&dir, "global_caller", TraceDirection::Both).unwrap();
    assert_eq!(
        global_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_before_import =
        trace_symbol_graph_from_index(&db_path, "api::before_import", TraceDirection::Both)
            .unwrap();
    assert!(persisted_before_import.callees.is_empty());
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
    let persisted_decimal_trace =
        trace_symbol_graph_from_index(&db_path, "api::decimal_caller", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_decimal_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::convert(double,double)"]
    );
    let persisted_global_trace =
        trace_symbol_graph_from_index(&db_path, "global_caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_global_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
}

#[test]
fn resolves_unqualified_cpp_using_declaration_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let definitions = dir.join("definitions.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "double convert(double left, double right) { return left + right; }\nnamespace api { namespace base { int convert(int value) { return value + 1; } } }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace api {\nnamespace import_alias = base;\nint before_import() { return convert(1); }\nusing import_alias::convert;\ndouble convert(double left, double right) { return left + right; }\nint caller() { return convert(1); }\ndouble decimal_caller() { return convert(1.0, 2.0); }\n}\n",
    )
    .unwrap();

    let expected_callee = "api::base::convert(int)";
    let before_import =
        trace_symbol_graph(&dir, "api::before_import", TraceDirection::Both).unwrap();
    assert!(before_import.callees.is_empty());
    let trace = trace_symbol_graph(&dir, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
    let decimal_trace =
        trace_symbol_graph(&dir, "api::decimal_caller", TraceDirection::Both).unwrap();
    assert_eq!(
        decimal_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::convert(double,double)"]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_before_import =
        trace_symbol_graph_from_index(&db_path, "api::before_import", TraceDirection::Both)
            .unwrap();
    assert!(persisted_before_import.callees.is_empty());
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
    let persisted_decimal_trace =
        trace_symbol_graph_from_index(&db_path, "api::decimal_caller", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_decimal_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::convert(double,double)"]
    );
}

#[test]
fn isolates_cpp_anonymous_namespace_symbols_per_file() {
    let dir = temporary_dir();
    let a = dir.join("a.cpp");
    let b = dir.join("b.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &a,
        "namespace {\nint helper(int value) { return value + 1; }\nint use_a(int value) { return helper(value); }\n}\n",
    )
    .unwrap();
    fs::write(
        &b,
        "namespace {\nint helper(int value) { return value + 2; }\nint use_b(int value) { return helper(value); }\n}\n",
    )
    .unwrap();

    let use_a = format!("{}::use_a", a.to_string_lossy().replace('\\', "/"));
    let use_b = format!("{}::use_b", b.to_string_lossy().replace('\\', "/"));
    let trace_a = trace_symbol_graph(&dir, &use_a, TraceDirection::Both).unwrap();
    let trace_b = trace_symbol_graph(&dir, &use_b, TraceDirection::Both).unwrap();

    assert_eq!(trace_a.callees.len(), 1);
    assert_eq!(trace_b.callees.len(), 1);
    assert_eq!(
        trace_a.callees[0].file_path,
        a.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        trace_b.callees[0].file_path,
        b.to_string_lossy().replace('\\', "/")
    );
    assert_ne!(
        trace_a.callees[0].semantic_path,
        trace_b.callees[0].semantic_path
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace_b =
        trace_symbol_graph_from_index(&db_path, &use_b, TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace_b.callees.len(), 1);
    assert_eq!(
        persisted_trace_b.callees[0].file_path,
        b.to_string_lossy().replace('\\', "/")
    );
}
