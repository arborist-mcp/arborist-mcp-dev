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
