use super::*;

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
