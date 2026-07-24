use super::*;

#[test]
fn resolves_cpp_this_member_template_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("this_member_template_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    template <typename T>\n    T adjust(T value) { return value; }\n    int caller(int value) { return this->template adjust<int>(value); }\n};\n}\n",
    )
    .unwrap();

    let expected_callee = "api::Counter::adjust(T)";
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
fn resolves_cpp_this_member_template_specializations_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("this_member_template_specialization_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    template <typename T>\n    T adjust(T value) { return value; }\n    int caller(int value) { return this->template adjust< int >(value); }\n};\ntemplate <>\nint Counter::adjust<int>(int value) { return value + 1; }\n}\n",
    )
    .unwrap();

    let expected_callee = "api::Counter::adjust<int>(int)";
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
fn traces_cpp_explicit_template_instantiations() {
    let dir = temporary_dir();
    let source = dir.join("instantiations.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T> class Vector {};\ntemplate <typename T> T increment(T value) { return value + 1; }\n}\n\ntemplate class api::Vector<int>;\ntemplate int api::increment<int>(int);\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Vector<int>")
    );
    let function = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::increment<int>")
        .expect("explicit function instantiation should be indexed");
    assert_eq!(function.parameters, vec!["int".to_string()]);
    assert_eq!(function.return_type.as_deref(), Some("int"));

    let trace = trace_symbol_graph(&dir, "api::Vector<int>", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::increment<int>", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_cpp_explicit_function_template_specialization() {
    let dir = temporary_dir();
    let header = dir.join("templates.hpp");
    let source = dir.join("templates.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "template <typename T>\nT increment(T value);\n\ntemplate <>\nint increment<int>(int value);\n",
    )
    .unwrap();
    fs::write(
        &source,
        "#include \"templates.hpp\"\n\ntemplate <>\nint increment<int>(int value) { return value + 2; }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "increment<int>", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "increment<int>", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_cpp_template_functions() {
    let dir = temporary_dir();
    let source = dir.join("templates.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "template <typename T>\nT increment(T value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "increment", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.semantic_path, "increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "increment", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.semantic_path, "increment");
}

#[test]
fn resolves_explicit_cpp_template_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("template_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T>\nT convert(T value) { return value; }\nint bare_caller() { return convert<int>(1); }\n}\nint qualified_caller() { return api::convert<int>(1); }\n",
    )
    .unwrap();

    let expected_callee = "api::convert(T)";
    for caller in ["api::bare_caller", "qualified_caller"] {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
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
    for caller in ["api::bare_caller", "qualified_caller"] {
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee]
        );
    }
}
