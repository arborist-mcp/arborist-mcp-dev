use super::*;

#[test]
fn expands_selected_c_function_definitions() {
    let source = r#"
typedef struct item {
    int value;
} item;

int helper(int value) {
    return value + 1;
}
"#;

    let skeleton =
        get_semantic_skeleton(Path::new("sample.c"), source, 1, &["helper".to_string()]).unwrap();

    assert!(skeleton.skeleton.contains("typedef struct item"));
    assert!(
        skeleton
            .skeleton
            .contains("int helper(int value) {\n    return value + 1;\n}")
    );
    assert_eq!(skeleton.available_symbols.len(), 2);
    assert_eq!(skeleton.available_symbols[1].semantic_path, "helper");
    assert_eq!(skeleton.available_symbols[1].scope_path, None);
    assert_eq!(
        skeleton.available_symbols[1].node_kind,
        "function_definition"
    );
    assert_eq!(
        skeleton.available_symbols[1].signature.as_deref(),
        Some("int helper(int value);")
    );
    assert_eq!(
        skeleton.available_symbols[1].parameters,
        vec!["int value".to_string()]
    );
    assert_eq!(
        skeleton.available_symbols[1].return_type.as_deref(),
        Some("int")
    );
    assert_eq!(skeleton.available_symbols[1].docstring, None);
}

#[test]
fn expands_c_function_definition_by_precise_symbol_id() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let precise_symbol_id = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "helper")
        .map(|symbol| symbol.symbol_id.clone())
        .unwrap();

    let expanded = get_semantic_skeleton(&source, &source_text, 1, &[precise_symbol_id]).unwrap();

    assert!(
        expanded
            .skeleton
            .contains("int helper(int value) {\n    return value + 1;\n}")
    );
}

#[test]
fn anchors_c_source_symbol_ids_to_uppercase_sibling_header() {
    let dir = temporary_dir();
    let header = dir.join("helper.H");
    let source = dir.join("helper.C");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "int helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let symbol = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "helper")
        .unwrap();

    assert_eq!(
        symbol.symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );
}

#[test]
fn traces_c_symbol_graph_across_header_declaration_and_source_definition() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let helper = dir.join("helper.c");
    let caller = dir.join("caller.c");
    let db_path = dir.join("symbols.db");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &helper,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );

    let stats = rebuild_symbol_index(&dir, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 3);

    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn traces_cpp_symbol_graph_across_header_declaration_and_source_definition() {
    let dir = temporary_dir();
    let header = dir.join("helper.hpp");
    let helper = dir.join("helper.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &helper,
        "#include \"helper.hpp\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.hpp\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );

    let stats = rebuild_symbol_index(&dir, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 3);
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn traces_cpp_extern_c_functions_across_header_and_source() {
    let dir = temporary_dir();
    let header = dir.join("bridge.hpp");
    let source = dir.join("bridge.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(&header, "extern \"C\" {\nint helper(int value);\n}\n").unwrap();
    fs::write(
        &source,
        "#include \"bridge.hpp\"\n\nextern \"C\" int helper(int value) { return value + 1; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"bridge.hpp\"\n\nint orchestrate(int value) { return helper(value); }\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "helper");
    assert_eq!(
        trace.callees[0].file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(
        persisted_trace.callees[0].file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_conditionally_compiled_cpp_functions() {
    let dir = temporary_dir();
    let source = dir.join("feature.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "#if ENABLED\nint helper(int value) { return value + 1; }\n#else\nint fallback(int value) { return value - 1; }\n#endif\n\nint orchestrate(int value) { return helper(value); }\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "fallback")
    );

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(persisted_trace.callees[0].semantic_path, "helper");
}

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
fn indexes_inline_cpp_class_methods_with_qualified_paths() {
    let source = r#"
namespace api {
class Counter {
public:
    int increment(int value) { return value + 1; }
    static int make(int value) { return value; }
    int current() const;
};
}
"#;

    let skeleton = get_semantic_skeleton(Path::new("counter.cpp"), source, 1, &[]).unwrap();

    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Counter::increment")
    );
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Counter::make")
    );
    let current = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::Counter::current")
        .expect("class method declaration should be indexed");
    assert_eq!(current.scope_path.as_deref(), Some("api::Counter"));
    assert_eq!(current.node_kind, "field_declaration");
}

#[test]
fn traces_inline_cpp_class_method_dependencies() {
    let dir = temporary_dir();
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int increment(int value) { return value + 1; }\n    int next(int value) { return increment(value); }\n};\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "api::Counter::next", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.semantic_path, "api::Counter::next");
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "api::Counter::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Counter::next", TraceDirection::Both)
            .unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(
        persisted_trace.callees[0].semantic_path,
        "api::Counter::increment"
    );
}

#[test]
fn traces_cpp_class_methods_defined_outside_the_class() {
    let dir = temporary_dir();
    let header = dir.join("counter.hpp");
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "namespace api {\nclass Counter {\npublic:\n    int increment(int value);\n    int next(int value);\n};\n}\n",
    )
    .unwrap();
    fs::write(
        &source,
        "#include \"counter.hpp\"\n\nnamespace api {\nint Counter::increment(int value) { return value + 1; }\n\nint Counter::next(int value) { return increment(value); }\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Counter::increment")
    );

    let trace = trace_symbol_graph(&dir, "api::Counter::next", TraceDirection::Both).unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "api::Counter::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Counter::next", TraceDirection::Both)
            .unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(
        persisted_trace.callees[0].semantic_path,
        "api::Counter::increment"
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
fn indexes_cpp_constructors_and_destructors() {
    let header_source = r#"
namespace api {
class Counter {
public:
    Counter(int value);
    ~Counter();
};
}
"#;
    let source = r#"
#include "counter.hpp"

api::Counter::Counter(int value) {}
api::Counter::~Counter() {}
"#;

    let header = get_semantic_skeleton(Path::new("counter.hpp"), header_source, 1, &[]).unwrap();
    assert!(
        header
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Counter::Counter")
    );
    assert!(
        header
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Counter::~Counter")
    );

    let implementation = get_semantic_skeleton(Path::new("counter.cpp"), source, 1, &[]).unwrap();
    assert!(
        implementation
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Counter::Counter")
    );
    assert!(
        implementation
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Counter::~Counter")
    );
}

#[test]
fn resolves_cpp_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    Counter(int value) {}\n    Counter(int left, int right) {}\n};\nCounter local_caller(int value) { return Counter(value); }\nCounter braced_caller(int value) { return Counter{value}; }\nCounter pair_braced_caller(int left, int right) { return Counter{left, right}; }\n}\napi::Counter qualified_caller(int value) { return api::Counter(value); }\napi::Counter qualified_braced_caller(int value) { return api::Counter{value}; }\n",
    )
    .unwrap();

    for (caller, expected_callee) in [
        ("api::local_caller", "api::Counter::Counter(int)"),
        ("api::braced_caller", "api::Counter::Counter(int)"),
        ("api::pair_braced_caller", "api::Counter::Counter(int,int)"),
        ("qualified_caller", "api::Counter::Counter(int)"),
        ("qualified_braced_caller", "api::Counter::Counter(int)"),
    ] {
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
    for (caller, expected_callee) in [
        ("api::local_caller", "api::Counter::Counter(int)"),
        ("api::braced_caller", "api::Counter::Counter(int)"),
        ("api::pair_braced_caller", "api::Counter::Counter(int,int)"),
        ("qualified_caller", "api::Counter::Counter(int)"),
        ("qualified_braced_caller", "api::Counter::Counter(int)"),
    ] {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
fn resolves_cpp_new_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: Counter(int value) {} Counter(int left, int right) {} }; }\nint caller(int value) { auto counter = new api::Counter(value); return value; }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
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
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn resolves_cpp_default_new_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: Counter() {} Counter(int value) {} }; }\nint caller() { auto counter = new api::Counter; return 0; }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter()"]
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
        vec!["api::Counter::Counter()"]
    );
}

#[test]
fn resolves_cpp_braced_initializer_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: Counter(int value) {} Counter(int left, int right) {} }; }\nint caller(int value) { api::Counter counter{value}; return value; }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
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
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn resolves_cpp_type_alias_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("alias.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: Counter(int value) {} Counter(int left, int right) {} }; }\nnamespace app { using First = api::Counter; using Alias = First; int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
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
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn resolves_cpp_typedef_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("alias.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: Counter(int value) {} Counter(int left, int right) {} }; }\nnamespace app { typedef api::Counter Alias; int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
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
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn resolves_cpp_cv_qualified_type_alias_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("alias.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: Counter(int value) {} Counter(int left, int right) {} }; }\nnamespace app { using Alias = const api::Counter; int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
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
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn resolves_cpp_header_type_alias_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let header = dir.join("aliases.hpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "namespace api { class Counter { public: Counter(int value) {} Counter(int left, int right) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
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
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn resolves_cpp_type_aliases_from_definitely_active_conditional_headers() {
    let dir = temporary_dir();
    let header = dir.join("aliases.hpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#if 1\n#include \"aliases.hpp\"\n#endif\nnamespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
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
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn does_not_trace_cpp_type_aliases_from_macro_conditional_headers() {
    let dir = temporary_dir();
    let header = dir.join("aliases.hpp");
    let caller = dir.join("caller.cpp");
    fs::write(
        &header,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#if ENABLE_ALIAS\n#include \"aliases.hpp\"\n#endif\nnamespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn resolves_cpp_type_aliases_through_cyclic_local_header_includes() {
    let dir = temporary_dir();
    let first_header = dir.join("first.hpp");
    let alias_header = dir.join("aliases.hpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(&first_header, "#include \"aliases.hpp\"\n").unwrap();
    fs::write(
        &alias_header,
        "#include \"first.hpp\"\nnamespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"first.hpp\"\nnamespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
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
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn does_not_trace_cpp_header_type_aliases_included_after_the_caller() {
    let dir = temporary_dir();
    let header = dir.join("aliases.hpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace app { int caller(int value) { Alias counter{value}; return value; } }\n#include \"aliases.hpp\"\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn does_not_trace_cpp_pointer_type_aliases_as_constructor_dependencies() {
    let dir = temporary_dir();
    let source = dir.join("alias.cpp");
    fs::write(
        &source,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Pointer = api::Counter*; int caller(api::Counter* value) { Pointer pointer{value}; return 0; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn resolves_cpp_template_type_alias_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("alias.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { template <typename T> class Box { public: Box(T value) {} }; }\nnamespace app { template <typename T> using Alias = api::Box<T>; int caller(int value) { Alias<int> box{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
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
        vec!["api::Box::Box(T)"]
    );
}

#[test]
fn does_not_trace_cpp_type_aliases_declared_after_the_caller() {
    let dir = temporary_dir();
    let source = dir.join("alias.cpp");
    fs::write(
        &source,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { int caller(int value) { Alias counter{value}; return value; } using Alias = api::Counter; }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn does_not_trace_cpp_type_aliases_from_unrelated_files() {
    let dir = temporary_dir();
    let definitions = dir.join("counter.cpp");
    let aliases = dir.join("aliases.cpp");
    let caller = dir.join("caller.cpp");
    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int value) {} }; }\n",
    )
    .unwrap();
    fs::write(&aliases, "namespace app { using Alias = api::Counter; }\n").unwrap();
    fs::write(
        &caller,
        "namespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn does_not_trace_unresolved_cpp_type_aliases_as_constructor_dependencies() {
    let dir = temporary_dir();
    let source = dir.join("alias.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace app { using Missing = external::Counter; int caller(int value) { Missing counter{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn resolves_cpp_template_braced_initializer_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("box.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int value) {} };\n}\nint caller(int value) { api::Box<int> box{value}; return value; }\n",
    )
    .unwrap();

    let expected_callee = "api::Box<int>::Box(int)";
    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
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
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
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
fn resolves_cpp_template_new_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("box.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int value) {} };\n}\nint caller(int value) { auto box = new api::Box<int>(value); return value; }\n",
    )
    .unwrap();

    let expected_callee = "api::Box<int>::Box(int)";
    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
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
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
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
fn resolves_cpp_template_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("box.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { template <typename T> class Box { public: Box(T value) {} }; }\nint caller(int value) { auto box = api::Box<int>{value}; return value; }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
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
        vec!["api::Box::Box(T)"]
    );
}

#[test]
fn prefers_cpp_template_specialization_constructors_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("box.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int left, int right) {} };\n}\nint caller(int value) { auto box = api::Box<int>{value, value}; return value; }\n",
    )
    .unwrap();

    let expected_callee = "api::Box<int>::Box(int,int)";
    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
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
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
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
fn resolves_cpp_imported_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace lib { class Counter { public: Counter(int value) {} }; }\nnamespace api { using namespace lib; Counter namespace_caller(int value) { return Counter(value); } }\nnamespace vendor = lib;\nnamespace app { using vendor::Counter; Counter declaration_caller(int value) { return Counter(value); } }\n",
    )
    .unwrap();

    for caller in ["api::namespace_caller", "app::declaration_caller"] {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec!["lib::Counter::Counter(int)"]
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["api::namespace_caller", "app::declaration_caller"] {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec!["lib::Counter::Counter(int)"]
        );
    }
}

#[test]
fn indexes_defaulted_and_deleted_cpp_methods() {
    let source = r#"
namespace api {
class Defaulted {
public:
    Defaulted() = default;
};

class Deleted {
public:
    Deleted() = delete;
};
}
"#;

    let skeleton = get_semantic_skeleton(Path::new("lifecycle.hpp"), source, 1, &[]).unwrap();
    let defaulted = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::Defaulted::Defaulted")
        .expect("defaulted constructor should be indexed");
    assert_eq!(
        defaulted.signature.as_deref(),
        Some("Defaulted() = default;")
    );

    let deleted = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::Deleted::Deleted")
        .expect("deleted constructor should be indexed");
    assert_eq!(deleted.signature.as_deref(), Some("Deleted() = delete;"));
}

#[test]
fn traces_defaulted_cpp_methods() {
    let dir = temporary_dir();
    let source = dir.join("lifecycle.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Defaulted {\npublic:\n    Defaulted() = default;\n};\n\nclass Deleted {\npublic:\n    Deleted() = delete;\n};\n}\n",
    )
    .unwrap();

    let trace =
        trace_symbol_graph(&dir, "api::Defaulted::Defaulted", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.semantic_path, "api::Defaulted::Defaulted");
    let deleted_trace =
        trace_symbol_graph(&dir, "api::Deleted::Deleted", TraceDirection::Both).unwrap();
    assert_eq!(deleted_trace.symbol.semantic_path, "api::Deleted::Deleted");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Defaulted::Defaulted", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_trace.symbol.semantic_path,
        "api::Defaulted::Defaulted"
    );
    let persisted_deleted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Deleted::Deleted", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_deleted_trace.symbol.semantic_path,
        "api::Deleted::Deleted"
    );
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
fn does_not_resolve_cpp_qualified_imports_declared_after_callers() {
    let dir = temporary_dir();
    let source = dir.join("qualified_import_order.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nnamespace implementation { int convert(int value) { return value; } }\nint before_alias() { return detail::convert(1); }\nnamespace detail = implementation;\nint after_alias() { return detail::convert(1); }\nnamespace imported { int scale(int value) { return value; } }\nint before_using() { return api::scale(1); }\nusing imported::scale;\nint after_using() { return api::scale(1); }\n}\n",
    )
    .unwrap();

    for symbol_path in ["api::before_alias", "api::before_using"] {
        let trace = trace_symbol_graph(&dir, symbol_path, TraceDirection::Both).unwrap();
        assert!(trace.callees.is_empty(), "{symbol_path}: {trace:#?}");
    }
    for (symbol_path, expected_callee) in [
        ("api::after_alias", "api::implementation::convert(int)"),
        ("api::after_using", "api::imported::scale(int)"),
    ] {
        let trace = trace_symbol_graph(&dir, symbol_path, TraceDirection::Both).unwrap();
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
    for symbol_path in ["api::before_alias", "api::before_using"] {
        let trace =
            trace_symbol_graph_from_index(&db_path, symbol_path, TraceDirection::Both).unwrap();
        assert!(trace.callees.is_empty(), "{symbol_path}: {trace:#?}");
    }
    for (symbol_path, expected_callee) in [
        ("api::after_alias", "api::implementation::convert(int)"),
        ("api::after_using", "api::imported::scale(int)"),
    ] {
        let trace =
            trace_symbol_graph_from_index(&db_path, symbol_path, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
        );
    }
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
fn traces_cpp_concept_definitions() {
    let dir = temporary_dir();
    let source = dir.join("concepts.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T>\nconcept Incrementable = requires(T value) { value + 1; };\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Incrementable")
    );

    let trace = trace_symbol_graph(&dir, "api::Incrementable", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Incrementable", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_cpp_class_definitions() {
    let dir = temporary_dir();
    let source = dir.join("config.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T>\nclass Config {\npublic:\n    class State {};\n    T value(T input) { return input; }\n};\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let config = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::Config")
        .expect("class definition should be indexed");
    assert_eq!(config.scope_path.as_deref(), Some("api"));
    let state = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::Config::State")
        .expect("nested class definition should be indexed");
    assert_eq!(state.scope_path.as_deref(), Some("api::Config"));

    let trace = trace_symbol_graph(&dir, "api::Config", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Config", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.scope_path.as_deref(), Some("api"));
}

#[test]
fn traces_named_c_struct_and_union_definitions() {
    let dir = temporary_dir();
    let source = dir.join("protocol.c");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "struct Packet { int id; };\nunion Payload { int count; float ratio; };\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let packet = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "Packet")
        .expect("named C struct definition should be indexed");
    assert_eq!(packet.node_kind, "struct_specifier");
    assert_eq!(packet.scope_path, None);
    let payload = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "Payload")
        .expect("named C union definition should be indexed");
    assert_eq!(payload.node_kind, "union_specifier");
    assert_eq!(payload.scope_path, None);

    let trace = trace_symbol_graph(&dir, "Packet", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "Payload", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_c_enum_members() {
    let dir = temporary_dir();
    let source = dir.join("status.c");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "enum Status { STATUS_READY = 1, STATUS_FAILED = 2 };\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "STATUS_READY")
    );

    let trace = trace_symbol_graph(&dir, "STATUS_FAILED", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.node_kind, "enumerator");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "STATUS_READY", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.node_kind, "enumerator");
}

#[test]
fn traces_cpp_struct_methods_and_nested_union_definitions() {
    let dir = temporary_dir();
    let source = dir.join("counter.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nstruct Counter {\n    union Storage { int count; double ratio; };\n    int increment(int value) { return value + 1; }\n};\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let counter = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::Counter")
        .expect("C++ struct definition should be indexed");
    assert_eq!(counter.node_kind, "struct_specifier");
    assert_eq!(counter.scope_path.as_deref(), Some("api"));
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Counter::Storage")
    );
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Counter::increment")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Counter::Storage", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_trace.symbol.scope_path.as_deref(),
        Some("api::Counter")
    );
}

#[test]
fn traces_cpp_enum_definitions() {
    let dir = temporary_dir();
    let source = dir.join("status.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nenum class Status : unsigned char { idle, busy };\n\nclass Task {\npublic:\n    enum class State { queued, running };\n};\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Status")
    );
    let state = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::Task::State")
        .expect("nested enum definition should be indexed");
    assert_eq!(state.scope_path.as_deref(), Some("api::Task"));

    let trace = trace_symbol_graph(&dir, "api::Status", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Status", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.scope_path.as_deref(), Some("api"));
}

#[test]
fn traces_cpp_enum_members() {
    let dir = temporary_dir();
    let source = dir.join("status.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nenum class Status : unsigned char { idle = 0, busy };\nenum Legacy { pending, complete };\n\nclass Task {\npublic:\n    enum class State { queued, running };\n    enum Mode { paused, active };\n};\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    for expected_path in [
        "api::Status::idle",
        "api::Status::busy",
        "api::pending",
        "api::Task::State::queued",
        "api::Task::paused",
    ] {
        assert!(
            skeleton
                .available_symbols
                .iter()
                .any(|symbol| symbol.semantic_path == expected_path),
            "missing {expected_path} in {:#?}",
            skeleton.available_symbols
        );
    }

    let trace = trace_symbol_graph(&dir, "api::Status::busy", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.node_kind, "enumerator");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Task::State::queued", TraceDirection::Both)
            .unwrap();
    assert_eq!(persisted_trace.symbol.node_kind, "enumerator");
    assert_eq!(
        persisted_trace.symbol.scope_path.as_deref(),
        Some("api::Task::State")
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
fn indexes_cpp_operator_methods() {
    let source = r#"
namespace math {
class Number {
public:
    Number operator+(const Number& other) const {
        return *this;
    }
};
}
"#;

    let skeleton = get_semantic_skeleton(Path::new("number.cpp"), source, 1, &[]).unwrap();
    let operator = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "math::Number::operator+")
        .expect("operator method should be indexed");
    assert_eq!(operator.scope_path.as_deref(), Some("math::Number"));
    assert_eq!(operator.parameters, vec!["const Number& other".to_string()]);
}

#[test]
fn indexes_cpp_conversion_operator_methods() {
    let source = r#"
namespace config {
class Flag {
public:
    explicit operator bool() const {
        return true;
    }
};
}
"#;

    let skeleton = get_semantic_skeleton(Path::new("flag.cpp"), source, 1, &[]).unwrap();
    let conversion = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "config::Flag::operator bool")
        .expect("conversion operator should be indexed");
    assert_eq!(conversion.scope_path.as_deref(), Some("config::Flag"));
    assert!(conversion.parameters.is_empty());
    assert_eq!(conversion.return_type, None);
}

#[test]
fn traces_cpp_conversion_operator_methods() {
    let dir = temporary_dir();
    let source = dir.join("flag.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace config {\nclass Flag {\npublic:\n    explicit operator bool() const { return true; }\n};\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "config::Flag::operator bool"),
        "{:#?}",
        skeleton.available_symbols
    );

    let trace =
        trace_symbol_graph(&dir, "config::Flag::operator bool", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.semantic_path, "config::Flag::operator bool");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace = trace_symbol_graph_from_index(
        &db_path,
        "config::Flag::operator bool",
        TraceDirection::Both,
    )
    .unwrap();
    assert_eq!(
        persisted_trace.symbol.semantic_path,
        "config::Flag::operator bool"
    );
}

#[test]
fn traces_cpp_conversion_operator_defined_outside_class() {
    let dir = temporary_dir();
    let header = dir.join("flag.hpp");
    let source = dir.join("flag.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "namespace config {\nclass Flag {\npublic:\n    explicit operator bool() const;\n};\n}\n",
    )
    .unwrap();
    fs::write(
        &source,
        "#include \"flag.hpp\"\n\nconfig::Flag::operator bool() const { return true; }\n",
    )
    .unwrap();

    let trace =
        trace_symbol_graph(&dir, "config::Flag::operator bool", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace = trace_symbol_graph_from_index(
        &db_path,
        "config::Flag::operator bool",
        TraceDirection::Both,
    )
    .unwrap();
    assert_eq!(
        persisted_trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_cpp_operator_methods() {
    let dir = temporary_dir();
    let source = dir.join("number.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace math {\nclass Number {\npublic:\n    Number operator+(const Number& other) const { return *this; }\n};\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "math::Number::operator+", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.semantic_path, "math::Number::operator+");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "math::Number::operator+", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_trace.symbol.semantic_path,
        "math::Number::operator+"
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
fn resolves_cpp_defaulted_and_variadic_direct_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("call_shapes.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nint defaulted(int value, int radix = 10) { return value + radix; }\nint select(int first, int second, int third) { return first + second + third; }\nint select(int first, ...) { return first; }\nint use_default() { return defaulted(1); }\nint use_variadic() { return select(1, 2, 3, 4); }\n}\n",
    )
    .unwrap();

    for (caller, expected_path, expects_variadic) in [
        ("api::use_default", "api::defaulted", false),
        ("api::use_variadic", "api::select", true),
    ] {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
        assert_eq!(trace.callees.len(), 1, "{caller}: {:#?}", trace.callees);
        assert_eq!(trace.callees[0].semantic_path, expected_path);
        assert_eq!(
            trace.callees[0]
                .parameters
                .last()
                .is_some_and(|parameter| parameter.trim() == "..."),
            expects_variadic
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_path, expects_variadic) in [
        ("api::use_default", "api::defaulted", false),
        ("api::use_variadic", "api::select", true),
    ] {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(trace.callees.len(), 1, "{caller}: {:#?}", trace.callees);
        assert_eq!(trace.callees[0].semantic_path, expected_path);
        assert_eq!(
            trace.callees[0]
                .parameters
                .last()
                .is_some_and(|parameter| parameter.trim() == "..."),
            expects_variadic
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
fn preserves_cpp_callable_identity_for_qualifiers_and_declarator_shapes() {
    let dir = temporary_dir();
    let header = dir.join("counter.hpp");
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "namespace api {\nint helper();\nclass Counter { public: int value() const; };\nvoid reset(void);\nint transform(int values[3][4], void (*callback)(int code));\n}\n",
    )
    .unwrap();
    fs::write(
        &source,
        "#include \"counter.hpp\"\nnamespace api {\nint helper() { return 1; }\nint Counter::value() const { return helper(); }\nvoid reset() {}\nint transform(int buffer[3][4], void (*handler)(int error)) { handler(buffer[0][0]); return buffer[0][0]; }\n}\n",
    )
    .unwrap();

    let exact_ids = [
        "api::Counter::value() const",
        "api::reset()",
        "api::transform(int[3][4],void(*)(int))",
    ];
    for exact_id in exact_ids {
        let live = trace_symbol_graph(&dir, exact_id, TraceDirection::Both).unwrap();
        assert_eq!(live.symbol.symbol_id, exact_id);
        assert_eq!(
            live.symbol.file_path,
            source.to_string_lossy().replace('\\', "/")
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for exact_id in exact_ids {
        let persisted =
            trace_symbol_graph_from_index(&db_path, exact_id, TraceDirection::Both).unwrap();
        assert_eq!(persisted.symbol.symbol_id, exact_id);
        assert_eq!(
            persisted.symbol.file_path,
            source.to_string_lossy().replace('\\', "/")
        );
    }
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

#[test]
fn traces_cpp_constructors_and_destructors() {
    let dir = temporary_dir();
    let header = dir.join("counter.hpp");
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "namespace api {\nclass Counter {\npublic:\n    Counter(int value);\n    ~Counter();\n};\n}\n",
    )
    .unwrap();
    fs::write(
        &source,
        "#include \"counter.hpp\"\n\napi::Counter::Counter(int value) {}\napi::Counter::~Counter() {}\n",
    )
    .unwrap();

    let constructor =
        trace_symbol_graph(&dir, "api::Counter::Counter", TraceDirection::Both).unwrap();
    assert_eq!(
        constructor.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
    let destructor =
        trace_symbol_graph(&dir, "api::Counter::~Counter", TraceDirection::Both).unwrap();
    assert_eq!(
        destructor.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_destructor =
        trace_symbol_graph_from_index(&db_path, "api::Counter::~Counter", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_destructor.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_c_symbol_graph_across_uppercase_header_and_source_definition() {
    let dir = temporary_dir();
    let header = dir.join("helper.H");
    let helper = dir.join("helper.C");
    let caller = dir.join("caller.C");
    let db_path = dir.join("symbols.db");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &helper,
        "#include \"helper.H\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.H\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "helper");
    assert_eq!(trace.callees[0].origin_type, "companion_source");
    assert_eq!(
        trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        trace.callees[0].symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );

    let stats = rebuild_symbol_index(&dir, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 3);

    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(persisted_trace.callees[0].semantic_path, "helper");
    assert_eq!(persisted_trace.callees[0].origin_type, "companion_source");
    assert_eq!(
        persisted_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        persisted_trace.callees[0].symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );
}

#[test]
fn traces_c_symbol_graph_across_hpp_header_declaration() {
    let dir = temporary_dir();
    let header = dir.join("helper.HPP");
    let helper = dir.join("helper.c");
    let caller = dir.join("caller.c");
    let db_path = dir.join("symbols.db");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &helper,
        "#include \"helper.HPP\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.HPP\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(trace.callees[0].semantic_path, "helper");
    assert_eq!(trace.callees[0].origin_type, "companion_source");
    assert_eq!(
        trace.callees[0].symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );

    let stats = rebuild_symbol_index(&dir, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 3);

    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(persisted_trace.callees[0].semantic_path, "helper");
    assert_eq!(persisted_trace.callees[0].origin_type, "companion_source");
    assert_eq!(
        persisted_trace.callees[0].symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );
}

#[test]
fn isolates_static_c_symbols_per_file() {
    let dir = temporary_dir();
    let a = dir.join("a.c");
    let b = dir.join("b.c");
    let db_path = dir.join("symbols.db");

    fs::write(
        &a,
        "static int helper(int value) {\n    return value + 1;\n}\n\nint use_a(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();
    fs::write(
        &b,
        "static int helper(int value) {\n    return value + 2;\n}\n\nint use_b(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    let trace_a = trace_symbol_graph(&dir, "use_a", TraceDirection::Both).unwrap();
    let trace_b = trace_symbol_graph(&dir, "use_b", TraceDirection::Both).unwrap();

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
        trace_symbol_graph_from_index(&db_path, "use_b", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace_b.callees.len(), 1);
    assert_eq!(
        persisted_trace_b.callees[0].file_path,
        b.to_string_lossy().replace('\\', "/")
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

#[test]
fn prefers_callee_from_included_header_family_when_names_collide() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let caller = dir.join("caller.c");
    let db_path = dir.join("symbols.db");

    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"zeta.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(trace.callees.len(), 1);
    assert_eq!(
        trace.callees[0].file_path,
        zeta_source.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(trace.callees[0].origin_type, "companion_source");
    assert_eq!(
        trace.evidence_keys.callees,
        vec![trace.callees[0].evidence_key.clone()]
    );
    assert_eq!(trace.symbol.origin_type, "trace_root");
    assert_eq!(trace.symbol.evidence_key, trace.evidence_keys.symbol);
    assert!(trace.symbol.evidence_key.contains("trace_root"));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(
        persisted_trace.callees[0].file_path,
        zeta_source.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(persisted_trace.callees[0].origin_type, "companion_source");
    assert_eq!(
        persisted_trace.evidence_keys.callees,
        vec![persisted_trace.callees[0].evidence_key.clone()]
    );
    assert_eq!(persisted_trace.symbol.origin_type, "trace_root");
    assert_eq!(
        persisted_trace.symbol.evidence_key,
        persisted_trace.evidence_keys.symbol
    );
    let zeta_source_text = fs::read_to_string(&zeta_source).unwrap();
    let zeta_start = zeta_source_text.find("int helper(int value) {").unwrap();
    let zeta_end = zeta_source_text.find('}').map(|index| index + 1).unwrap();
    assert_eq!(persisted_trace.callees[0].node_kind, "function_definition");
    assert_eq!(
        persisted_trace.callees[0].byte_range,
        (zeta_start, zeta_end)
    );
    assert_eq!(
        persisted_trace.callees[0].signature.as_deref(),
        Some("int helper(int value);")
    );
    assert!(
        persisted_trace.callees[0]
            .evidence_key
            .contains(&persisted_trace.callees[0].symbol_id)
    );
    assert!(
        persisted_trace.callees[0]
            .evidence_key
            .contains("function_definition|companion_source")
    );
    assert!(
        persisted_trace.callees[0]
            .evidence_key
            .contains(&format!("{zeta_start}..{zeta_end}"))
    );
}
