use super::*;

#[test]
fn traces_cpp_template_friend_functions_in_enclosing_namespace() {
    let dir = temporary_dir();
    let source = dir.join("token.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Token {\n    template <typename T>\n    friend T inspect(const Token&, T value) { return value; }\n};\n\nint orchestrate(const Token& token) { return inspect(token, 1); }\n}\n",
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
fn indexes_cpp_template_functions_and_class_methods() {
    let source = r#"
template <typename T>
T increment(T value) {
    return value + 1;
}

template <typename T>
class Box {
public:
    T identity(T value) {
        return value;
    }
};
"#;

    let skeleton = get_semantic_skeleton(Path::new("templates.cpp"), source, 1, &[]).unwrap();
    let increment = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "increment")
        .expect("template function should be indexed");
    assert!(
        increment
            .signature
            .as_deref()
            .is_some_and(|signature| signature.starts_with("template <typename T>"))
    );

    let identity = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "Box::identity")
        .expect("template class method should be indexed");
    assert!(
        identity
            .signature
            .as_deref()
            .is_some_and(|signature| signature.starts_with("template <typename T>"))
    );

    let expanded = get_semantic_skeleton(
        Path::new("templates.cpp"),
        source,
        1,
        &["increment".to_string()],
    )
    .unwrap();
    assert!(
        expanded
            .skeleton
            .contains("template <typename T>\nT increment(T value)")
    );
}

#[test]
fn indexes_cpp_explicit_function_template_specializations() {
    let source = r#"
template <typename T>
T increment(T value) {
    return value + 1;
}

template <>
int increment<int>(int value) {
    return value + 2;
}
"#;

    let skeleton = get_semantic_skeleton(Path::new("templates.cpp"), source, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "increment"),
        "{:#?}",
        skeleton.available_symbols
    );
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "increment<int>"),
        "{:#?}",
        skeleton.available_symbols
    );
}

#[test]
fn indexes_cpp_explicit_class_template_specializations() {
    let source = r#"
template <typename T>
class Box {
public:
    T value() { return T{}; }
};

template <>
class Box<int> {
public:
    int value() { return 1; }
};
"#;

    let skeleton = get_semantic_skeleton(Path::new("templates.cpp"), source, 1, &[]).unwrap();
    for path in ["Box", "Box::value", "Box<int>", "Box<int>::value"] {
        assert!(
            skeleton
                .available_symbols
                .iter()
                .any(|symbol| symbol.semantic_path == path),
            "missing {path} in {:#?}",
            skeleton.available_symbols
        );
    }
}

#[test]
fn traces_cpp_explicit_class_template_specializations() {
    let dir = temporary_dir();
    let source = dir.join("templates.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "template <typename T>\nclass Box {\npublic:\n    T value() { return T{}; }\n};\n\ntemplate <>\nclass Box<int> {\npublic:\n    int value() { return 1; }\n};\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "Box<int>::value", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.semantic_path, "Box<int>::value");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "Box<int>::value", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.semantic_path, "Box<int>::value");
}

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

#[test]
fn indexes_cpp_template_implementation_file_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("algorithms.tpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T>\nT increment(T value) { return value + 1; }\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.symbol_id == "api::increment(T)")
    );

    let live = list_symbols(&dir, 20).unwrap();
    assert!(
        live.symbols
            .iter()
            .any(|symbol| symbol.symbol_id == "api::increment(T)")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = read_symbol_from_index(&db_path, "api::increment(T)").unwrap();
    assert_eq!(
        persisted.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn keeps_cpp_overload_identity_stable_between_unnamed_template_parameters_and_definitions() {
    let dir = temporary_dir();
    let header = dir.join("convert.hpp");
    let source = dir.join("convert.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "#include <vector>\nnamespace api { int convert(const std::vector<int>&); }\n",
    )
    .unwrap();
    fs::write(
        &source,
        "#include \"convert.hpp\"\nnamespace api { int convert(const std::vector<int>& values) { return static_cast<int>(values.size()); } }\n",
    )
    .unwrap();

    let exact_id = "api::convert(const std::vector<int>&)";
    let trace = trace_symbol_graph(&dir, exact_id, TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.symbol_id, exact_id);
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, exact_id, TraceDirection::Both).unwrap();
    assert_eq!(persisted.symbol.symbol_id, exact_id);
    assert_eq!(
        persisted.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn does_not_trace_non_type_cpp_template_parameters_as_global_references() {
    let dir = temporary_dir();
    let source = dir.join("templates.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "int Offset() { return 1; }\n\ntemplate <int Offset>\nint adjust(int value) { return value + Offset; }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "adjust", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty(), "{:#?}", trace.callees);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "adjust", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace.callees.is_empty(),
        "{:#?}",
        persisted_trace.callees
    );
}

#[test]
fn traces_qualified_references_named_like_cpp_template_parameters() {
    let dir = temporary_dir();
    let source = dir.join("templates.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace config { int Offset() { return 1; } }\n\ntemplate <int Offset>\nint adjust() { return config::Offset(); }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "adjust", TraceDirection::Both).unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "config::Offset");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "adjust", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(persisted_trace.callees[0].semantic_path, "config::Offset");
}
