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
