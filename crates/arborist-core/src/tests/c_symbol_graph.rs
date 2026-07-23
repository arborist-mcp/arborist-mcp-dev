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
fn resolves_cpp_local_variable_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("local_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nusing Alias = Counter;\nint lvalue_caller(int value) { Alias current{}; return current.adjust(value); }\nint const_lvalue_caller(int value) { const Alias current{}; return current.adjust(value); }\nint postfix_const_caller(int value) { Alias const current{}; return current.adjust(value); }\nint static_caller(int value) { static Alias current{}; return current.adjust(value); }\nint static_const_caller(int value) { static const Alias current{}; return current.adjust(value); }\nint moved_caller(int value) { Alias current{}; return std::move(current).adjust(value); }\nint shadowed_caller(int value) { Alias current{}; { const Alias current{}; return current.adjust(value); } }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::lvalue_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::static_caller", "api::Counter::adjust(int) &"),
        (
            "api::static_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::moved_caller", "api::Counter::adjust(int) &&"),
        ("api::shadowed_caller", "api::Counter::adjust(int) const &"),
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
fn resolves_cpp_weak_pointer_lock_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("weak_pointer_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint direct_caller(std::weak_ptr<Alias> current, int value) { return current.lock()->adjust(value); }\nint local_caller(std::weak_ptr<Alias> current, int value) { auto shared = current.lock(); return shared->adjust(value); }\nint const_wrapper_caller(const std::weak_ptr<Alias> current, int value) { return current.lock()->adjust(value); }\nint auto_const_wrapper_caller(int value) { const auto current = std::weak_ptr<Alias>{}; return current.lock()->adjust(value); }\nint const_caller(std::weak_ptr<const Alias> current, int value) { return current.lock()->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        ("api::local_caller", "api::Counter::adjust(int) &"),
        ("api::const_wrapper_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_const_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nusing Alias = Counter;\nint arrow_caller(std::expected<Alias, int> current, int value) { return current->adjust(value); }\nint value_caller(std::expected<Alias, int> current, int value) { return current.value().adjust(value); }\nint dereference_caller(std::expected<Alias, int> current, int value) { return (*current).adjust(value); }\nint moved_value_caller(std::expected<Alias, int> current, int value) { return std::move(current).value().adjust(value); }\nint const_caller(const std::expected<Alias, int> current, int value) { return current.value().adjust(value); }\nint auto_value_caller(std::expected<Alias, int> current, int value) { auto current_value = current.value(); return current_value.adjust(value); }\nint const_auto_value_caller(std::expected<Alias, int> current, int value) { const auto current_value = current.value(); return current_value.adjust(value); }\nint copied_const_source_value_caller(const std::expected<Alias, int> current, int value) { auto current_value = current.value(); return current_value.adjust(value); }\nint moved_auto_value_caller(std::expected<Alias, int> current, int value) { auto current_value = std::move(current).value(); return current_value.adjust(value); }\nint nested_expected_value_caller(std::expected<std::expected<Alias, int>, int> current, int value) { return current.value().value().adjust(value); }\nint const_nested_expected_value_caller(const std::expected<std::expected<Alias, int>, int> current, int value) { return current.value().value().adjust(value); }\nint moved_nested_expected_value_caller(std::expected<std::expected<Alias, int>, int> current, int value) { return std::move(current).value().value().adjust(value); }\nint auto_nested_expected_value_caller(std::expected<std::expected<Alias, int>, int> current, int value) { auto current_value = current.value(); return current_value.value().adjust(value); }\nint auto_nested_expected_error_caller(std::expected<std::expected<int, Alias>, int> current, int value) { auto current_value = current.value(); return current_value.error().adjust(value); }\nint const_auto_nested_expected_error_caller(std::expected<std::expected<int, Alias>, int> current, int value) { const auto current_value = current.value(); return current_value.error().adjust(value); }\nint nested_optional_value_caller(std::expected<std::optional<Alias>, int> current, int value) { return current.value().value().adjust(value); }\nint const_nested_optional_value_caller(const std::expected<std::optional<Alias>, int> current, int value) { return current.value().value().adjust(value); }\nint moved_nested_optional_value_caller(std::expected<std::optional<Alias>, int> current, int value) { return std::move(current).value().value().adjust(value); }\nint auto_optional_value_caller(std::expected<std::optional<Alias>, int> current, int value) { auto current_value = current.value(); return current_value->adjust(value); }\nint const_auto_optional_value_caller(std::expected<std::optional<Alias>, int> current, int value) { const auto current_value = current.value(); return current_value->adjust(value); }\nint auto_pointer_value_caller(std::expected<std::shared_ptr<Alias>, int> current, int value) { auto current_value = current.value(); return current_value->adjust(value); }\nint get_copy_caller(std::expected<std::unique_ptr<Alias>, int> current, int value) { auto pointer = current.value().get(); return pointer->adjust(value); }\nint const_get_copy_caller(std::expected<std::shared_ptr<const Alias>, int> current, int value) { auto pointer = current.value().get(); return pointer->adjust(value); }\nint dereference_copy_caller(std::expected<std::unique_ptr<Alias>, int> current, int value) { auto target = *current.value(); return target.adjust(value); }\nint const_dereference_copy_caller(std::expected<std::shared_ptr<const Alias>, int> current, int value) { auto target = *current.value(); return target.adjust(value); }\nint dereference_alias_caller(std::expected<std::unique_ptr<Alias>, int> current, int value) { auto& target = *current.value(); return target.adjust(value); }\nint const_dereference_alias_caller(const std::expected<std::shared_ptr<Alias>, int> current, int value) { auto&& target = *current.value(); return target.adjust(value); }\nint auto_caller(int value) { auto current = std::expected<Alias, int>{}; return current->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::arrow_caller", "api::Counter::adjust(int) &"),
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &&"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        ("api::auto_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_auto_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_expected_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_nested_expected_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_nested_expected_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::auto_nested_expected_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_nested_expected_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_nested_expected_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::nested_optional_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_nested_optional_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_nested_optional_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::auto_optional_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_optional_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_pointer_value_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_get_copy_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::dereference_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::auto_caller", "api::Counter::adjust(int) &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_error_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {};\nclass Failure {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nusing Value = Counter;\nusing Error = Failure;\nint error_caller(std::expected<Value, Error> current, int value) { return current.error().adjust(value); }\nint moved_error_caller(std::expected<Value, Error> current, int value) { return std::move(current).error().adjust(value); }\nint const_error_caller(const std::expected<Value, Error> current, int value) { return current.error().adjust(value); }\nint const_value_caller(std::expected<const Value, Error> current, int value) { return current.error().adjust(value); }\nint const_error_type_caller(std::expected<Value, const Error> current, int value) { return current.error().adjust(value); }\nint auto_error_caller(int value) { auto current = std::expected<Value, Error>{}; return current.error().adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_caller", "api::Failure::adjust(int) &"),
        ("api::moved_error_caller", "api::Failure::adjust(int) &&"),
        (
            "api::const_error_caller",
            "api::Failure::adjust(int) const &",
        ),
        ("api::const_value_caller", "api::Failure::adjust(int) &"),
        (
            "api::const_error_type_caller",
            "api::Failure::adjust(int) const &",
        ),
        ("api::auto_error_caller", "api::Failure::adjust(int) &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_error_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Failure {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nint error_alias_caller(std::expected<Value, Failure> current, int value) { auto& alias = current.error(); return alias.adjust(value); }\nint decltype_error_alias_caller(std::expected<Value, Failure> current, int value) { decltype(auto) alias = current.error(); return alias.adjust(value); }\nint const_error_alias_caller(const std::expected<Value, Failure> current, int value) { auto&& alias = current.error(); return alias.adjust(value); }\nint moved_error_alias_caller(std::expected<Value, Failure> current, int value) { auto&& alias = std::move(current).error(); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_alias_caller", "api::Failure::adjust(int) &"),
        (
            "api::decltype_error_alias_caller",
            "api::Failure::adjust(int) &",
        ),
        (
            "api::const_error_alias_caller",
            "api::Failure::adjust(int) const &",
        ),
        (
            "api::moved_error_alias_caller",
            "api::Failure::adjust(int) &",
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_error_smart_pointer_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_smart_pointer_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { return current.error()->adjust(value); }\nint moved_error_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { return std::move(current).error()->adjust(value); }\nint const_error_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { return current.error()->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_caller", "api::Counter::adjust(int) &"),
        ("api::moved_error_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_error_smart_pointer_alias_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_smart_pointer_alias_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint unique_alias_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto& error = current.error(); return error->adjust(value); }\nint shared_alias_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { decltype(auto) error = current.error(); return error->adjust(value); }\nint const_shared_alias_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { auto&& error = current.error(); return error->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::unique_alias_caller", "api::Counter::adjust(int) &"),
        ("api::shared_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_alias_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_error_optional_arrow_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_optional_arrow_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n    int adjust(int value) const && { return value + 3; }\n};\nint error_caller(std::expected<Value, std::optional<Counter>> current, int value) { return current.error()->adjust(value); }\nint moved_error_caller(std::expected<Value, std::optional<Counter>> current, int value) { return std::move(current).error()->adjust(value); }\nint const_error_caller(const std::expected<Value, std::optional<Counter>> current, int value) { return current.error()->adjust(value); }\nint const_pointee_caller(std::expected<Value, std::optional<const Counter>> current, int value) { return current.error()->adjust(value); }\nint value_caller(std::expected<Value, std::optional<Counter>> current, int value) { return current.error().value().adjust(value); }\nint moved_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { return std::move(current).error().value().adjust(value); }\nint dereference_caller(std::expected<Value, std::optional<Counter>> current, int value) { return (*current.error()).adjust(value); }\nint const_value_caller(const std::expected<Value, std::optional<Counter>> current, int value) { return current.error().value().adjust(value); }\nint auto_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = current.error().value(); return error_value.adjust(value); }\nint const_auto_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { const auto error_value = current.error().value(); return error_value.adjust(value); }\nint copied_const_source_value_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = current.error().value(); return error_value.adjust(value); }\nint auto_pointer_value_caller(std::expected<Value, std::optional<std::shared_ptr<Counter>>> current, int value) { auto error_value = current.error().value(); return error_value->adjust(value); }\nint auto_dereference_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = *current.error(); return error_value.adjust(value); }\nint const_auto_dereference_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { const auto error_value = *current.error(); return error_value.adjust(value); }\nint copied_const_source_dereference_value_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = *current.error(); return error_value.adjust(value); }\nint value_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error_value = current.error().value(); return error_value.adjust(value); }\nint decltype_value_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { decltype(auto) error_value = current.error().value(); return error_value.adjust(value); }\nint const_value_alias_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto&& error_value = current.error().value(); return error_value.adjust(value); }\nint dereference_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error_value = *current.error(); return error_value.adjust(value); }\nint decltype_dereference_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { decltype(auto) error_value = *current.error(); return error_value.adjust(value); }\nint const_dereference_alias_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto&& error_value = *current.error(); return error_value.adjust(value); }\nint alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error = current.error(); return error->adjust(value); }\nint decltype_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { decltype(auto) error = current.error(); return error->adjust(value); }\nint const_alias_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto&& error = current.error(); return error->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_caller", "api::Counter::adjust(int) &"),
        ("api::moved_error_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &&"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::auto_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_pointer_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_dereference_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_dereference_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_dereference_value_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_value_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_value_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::decltype_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_alias_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_value_reference_wrapper_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_value_reference_wrapper_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Error {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint value_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { return current.value().get().adjust(value); }\nint moved_value_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { return std::move(current).value().get().adjust(value); }\nint const_wrapper_caller(const std::expected<std::reference_wrapper<Counter>, Error> current, int value) { return current.value().get().adjust(value); }\nint const_value_caller(std::expected<std::reference_wrapper<const Counter>, Error> current, int value) { return current.value().get().adjust(value); }\nint alias_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto& current_value = current.value(); return current_value.get().adjust(value); }\nint const_alias_caller(const std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto&& current_value = current.value(); return current_value.get().adjust(value); }\nint get_alias_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto& target = current.value().get(); return target.adjust(value); }\nint decltype_get_alias_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { decltype(auto) target = current.value().get(); return target.adjust(value); }\nint const_get_alias_caller(const std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto&& target = current.value().get(); return target.adjust(value); }\nint get_copy_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { auto target = current.value().get(); return target.adjust(value); }\nint const_get_copy_caller(std::expected<std::reference_wrapper<const Counter>, Error> current, int value) { auto target = current.value().get(); return target.adjust(value); }\nint const_auto_get_copy_caller(std::expected<std::reference_wrapper<Counter>, Error> current, int value) { const auto target = current.value().get(); return target.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &"),
        ("api::const_wrapper_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::const_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_get_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::const_get_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        ("api::const_get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_get_copy_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_value_weak_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_value_weak_pointer_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Error {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint value_caller(std::expected<std::weak_ptr<Counter>, Error> current, int value) { return current.value().lock()->adjust(value); }\nint moved_value_caller(std::expected<std::weak_ptr<Counter>, Error> current, int value) { return std::move(current).value().lock()->adjust(value); }\nint const_value_caller(std::expected<std::weak_ptr<const Counter>, Error> current, int value) { return current.value().lock()->adjust(value); }\nint alias_caller(std::expected<std::weak_ptr<Counter>, Error> current, int value) { auto& current_value = current.value(); return current_value.lock()->adjust(value); }\nint lock_copy_caller(std::expected<std::weak_ptr<Counter>, Error> current, int value) { auto shared = current.value().lock(); return shared->adjust(value); }\nint const_lock_copy_caller(std::expected<std::weak_ptr<const Counter>, Error> current, int value) { auto shared = current.value().lock(); return shared->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::lock_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_lock_copy_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_error_reference_wrapper_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_reference_wrapper_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { return current.error().get().adjust(value); }\nint moved_error_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { return std::move(current).error().get().adjust(value); }\nint const_wrapper_caller(const std::expected<Value, std::reference_wrapper<Counter>> current, int value) { return current.error().get().adjust(value); }\nint const_error_caller(std::expected<Value, std::reference_wrapper<const Counter>> current, int value) { return current.error().get().adjust(value); }\nint alias_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto& error = current.error(); return error.get().adjust(value); }\nint const_alias_caller(const std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto&& error = current.error(); return error.get().adjust(value); }\nint get_alias_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto& target = current.error().get(); return target.adjust(value); }\nint decltype_get_alias_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { decltype(auto) target = current.error().get(); return target.adjust(value); }\nint const_get_alias_caller(const std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto&& target = current.error().get(); return target.adjust(value); }\nint get_copy_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto target = current.error().get(); return target.adjust(value); }\nint const_get_copy_caller(std::expected<Value, std::reference_wrapper<const Counter>> current, int value) { auto target = current.error().get(); return target.adjust(value); }\nint const_auto_get_copy_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { const auto target = current.error().get(); return target.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_caller", "api::Counter::adjust(int) &"),
        ("api::moved_error_caller", "api::Counter::adjust(int) &"),
        ("api::const_wrapper_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::const_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_get_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::const_get_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        ("api::const_get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_get_copy_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_error_weak_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_weak_pointer_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { return current.error().lock()->adjust(value); }\nint moved_error_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { return std::move(current).error().lock()->adjust(value); }\nint const_error_caller(std::expected<Value, std::weak_ptr<const Counter>> current, int value) { return current.error().lock()->adjust(value); }\nint alias_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { auto& error = current.error(); return error.lock()->adjust(value); }\nint lock_copy_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { auto shared = current.error().lock(); return shared->adjust(value); }\nint const_lock_copy_caller(std::expected<Value, std::weak_ptr<const Counter>> current, int value) { auto shared = current.error().lock(); return shared->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_caller", "api::Counter::adjust(int) &"),
        ("api::moved_error_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::lock_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_lock_copy_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_error_smart_pointer_get_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_smart_pointer_get_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint unique_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { return current.error().get()->adjust(value); }\nint shared_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { return std::move(current).error().get()->adjust(value); }\nint const_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { return current.error().get()->adjust(value); }\nint alias_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto& error = current.error(); return error.get()->adjust(value); }\nint get_copy_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto pointer = current.error().get(); return pointer->adjust(value); }\nint const_get_copy_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { auto pointer = current.error().get(); return pointer->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::unique_caller", "api::Counter::adjust(int) &"),
        ("api::shared_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_get_copy_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_error_smart_pointer_dereferences_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_error_smart_pointer_dereferences.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint unique_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { return (*current.error()).adjust(value); }\nint shared_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { return (*std::move(current).error()).adjust(value); }\nint const_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { return (*current.error()).adjust(value); }\nint alias_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto& error = current.error(); return (*error).adjust(value); }\nint dereference_copy_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto target = *current.error(); return target.adjust(value); }\nint const_dereference_copy_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { auto target = *current.error(); return target.adjust(value); }\nint dereference_alias_caller(std::expected<Value, std::unique_ptr<Counter>> current, int value) { auto& target = *current.error(); return target.adjust(value); }\nint const_dereference_alias_caller(const std::expected<Value, std::shared_ptr<Counter>> current, int value) { auto&& target = *current.error(); return target.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::unique_caller", "api::Counter::adjust(int) &"),
        ("api::shared_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::dereference_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_alias_caller",
            "api::Counter::adjust(int) &",
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_auto_expected_error_wrapper_copies_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("auto_expected_error_wrapper_copies.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nint optional_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto error = current.error(); return error->adjust(value); }\nint const_optional_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto error = current.error(); return error->adjust(value); }\nint const_copied_optional_caller(std::expected<Value, std::optional<Counter>> current, int value) { const auto error = current.error(); return error->adjust(value); }\nint nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { auto error = current.error(); return error.error().adjust(value); }\nint const_nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { const auto error = current.error(); return error.error().adjust(value); }\nint direct_nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { return current.error().error().adjust(value); }\nint direct_const_nested_expected_caller(const std::expected<Value, std::expected<Value, Counter>> current, int value) { return current.error().error().adjust(value); }\nint direct_const_nested_error_type_caller(std::expected<Value, const std::expected<Value, Counter>> current, int value) { return current.error().error().adjust(value); }\nint direct_moved_nested_expected_caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { return std::move(current).error().error().adjust(value); }\nint pointer_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { auto error = current.error(); return error->adjust(value); }\nint const_copied_pointer_caller(std::expected<Value, std::shared_ptr<Counter>> current, int value) { const auto error = current.error(); return error->adjust(value); }\nint const_pointer_caller(std::expected<Value, std::shared_ptr<const Counter>> current, int value) { auto error = current.error(); return error->adjust(value); }\nint wrapper_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { auto error = current.error(); return error.get().adjust(value); }\nint const_copied_wrapper_caller(std::expected<Value, std::reference_wrapper<Counter>> current, int value) { const auto error = current.error(); return error.get().adjust(value); }\nint weak_caller(std::expected<Value, std::weak_ptr<const Counter>> current, int value) { auto error = current.error(); return error.lock()->adjust(value); }\nint const_copied_weak_caller(std::expected<Value, std::weak_ptr<Counter>> current, int value) { const auto error = current.error(); return error.lock()->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::optional_caller", "api::Counter::adjust(int) &"),
        ("api::const_optional_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_copied_optional_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::nested_expected_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_nested_expected_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::direct_nested_expected_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::direct_const_nested_expected_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::direct_const_nested_error_type_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::direct_moved_nested_expected_caller",
            "api::Counter::adjust(int) &&",
        ),
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_copied_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::wrapper_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_copied_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::weak_caller", "api::Counter::adjust(int) const &"),
        (
            "api::const_copied_weak_caller",
            "api::Counter::adjust(int) &",
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_get_if_pointer_bindings_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("get_if_pointer_bindings.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint auto_get_if_caller(std::variant<Counter, Value> current, int value) { auto nested = std::get_if<Counter>(&current); return nested->adjust(value); }\nint auto_star_get_if_caller(std::variant<Counter, Value> current, int value) { auto* nested = std::get_if<Counter>(&current); return nested->adjust(value); }\nint decltype_auto_get_if_caller(std::variant<Counter, Value> current, int value) { decltype(auto) nested = std::get_if<Counter>(&current); return nested->adjust(value); }\nint auto_const_get_if_caller(const std::variant<Counter, Value> current, int value) { auto nested = std::get_if<const Counter>(&current); return nested->adjust(value); }\nint auto_dynamic_pointer_cast_caller(std::shared_ptr<Value> current, int value) { auto nested = std::dynamic_pointer_cast<Counter>(current); return nested->adjust(value); }\nint decltype_auto_dynamic_pointer_cast_caller(std::shared_ptr<Value> current, int value) { decltype(auto) nested = std::dynamic_pointer_cast<Counter>(current); return nested->adjust(value); }\nint auto_static_pointer_cast_caller(std::shared_ptr<Value> current, int value) { auto nested = std::static_pointer_cast<Counter>(current); return nested->adjust(value); }\nint auto_const_pointer_cast_caller(std::shared_ptr<const Counter> current, int value) { auto nested = std::const_pointer_cast<Counter>(current); return nested->adjust(value); }\nint auto_any_cast_pointer_caller(std::any current, int value) { auto nested = std::any_cast<Counter>(&current); return nested->adjust(value); }\nint auto_any_cast_value_caller(std::any current, int value) { auto nested = std::any_cast<Counter>(current); return nested.adjust(value); }\nint decltype_auto_any_cast_value_caller(std::any current, int value) { decltype(auto) nested = std::any_cast<Counter>(current); return nested.adjust(value); }\nint auto_variant_get_caller(std::variant<Counter, Value> current, int value) { auto nested = std::get<Counter>(current); return nested.adjust(value); }\nint decltype_auto_variant_get_caller(std::variant<Counter, Value> current, int value) { decltype(auto) nested = std::get<Counter>(current); return nested.adjust(value); }\nint auto_get_if_then_member_caller(std::variant<std::unique_ptr<Counter>, Value> current, int value) { auto nested = std::get_if<std::unique_ptr<Counter>>(&current); return (*nested)->adjust(value); }\nint decltype_auto_get_if_unique_caller(std::variant<std::unique_ptr<Counter>, Value> current, int value) { decltype(auto) nested = std::get_if<std::unique_ptr<Counter>>(&current); return (*nested)->adjust(value); }\nint auto_get_if_value_caller(std::variant<Counter, Value> current, int value) { auto nested = std::get_if<Counter>(&current); return nested->adjust(value); }\nint direct_to_address_raw_caller(Counter* current, int value) { return std::to_address(current)->adjust(value); }\nint auto_to_address_raw_caller(Counter* current, int value) { auto nested = std::to_address(current); return nested->adjust(value); }\nint decltype_auto_to_address_smart_caller(std::unique_ptr<Counter> current, int value) { decltype(auto) nested = std::to_address(current); return nested->adjust(value); }\nint auto_to_address_const_smart_caller(std::unique_ptr<const Counter> current, int value) { auto nested = std::to_address(current); return nested->adjust(value); }\nint vector_front_caller(std::vector<Counter> current, int value) { return current.front().adjust(value); }\nint vector_back_caller(std::vector<Counter> current, int value) { return current.back().adjust(value); }\nint array_at_caller(std::array<Counter, 2> current, int value) { return current.at(0).adjust(value); }\nint span_const_front_caller(std::span<const Counter> current, int value) { return current.front().adjust(value); }\nint const_vector_back_caller(const std::vector<Counter> current, int value) { return current.back().adjust(value); }\nint auto_tuple_get_caller(std::tuple<Value, Counter> current, int value) { auto nested = std::get<1>(current); return nested.adjust(value); }\nint decltype_auto_tuple_get_caller(std::tuple<Value, Counter> current, int value) { decltype(auto) nested = std::get<1>(current); return nested.adjust(value); }\nint auto_const_pair_get_caller(const std::pair<Counter, Value> current, int value) { auto nested = std::get<0>(current); return nested.adjust(value); }\nint decltype_auto_const_pair_get_caller(const std::pair<Counter, Value> current, int value) { decltype(auto) nested = std::get<0>(current); return nested.adjust(value); }\nint auto_tuple_get_unique_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { auto nested = std::get<1>(current); return nested->adjust(value); }\nint decltype_auto_tuple_get_unique_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { decltype(auto) nested = std::get<1>(current); return nested->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::auto_get_if_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_star_get_if_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_get_if_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_get_if_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_dynamic_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_dynamic_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_static_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_any_cast_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_any_cast_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_any_cast_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_get_if_then_member_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_get_if_unique_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_get_if_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::direct_to_address_raw_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_to_address_raw_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_to_address_smart_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_to_address_const_smart_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::vector_front_caller", "api::Counter::adjust(int) &"),
        ("api::vector_back_caller", "api::Counter::adjust(int) &"),
        ("api::array_at_caller", "api::Counter::adjust(int) &"),
        (
            "api::span_const_front_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_vector_back_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::auto_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_auto_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_pair_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_const_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_tuple_get_unique_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_tuple_get_unique_caller",
            "api::Counter::adjust(int) &",
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_direct_indexed_tuple_get_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("direct_indexed_tuple_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint direct_tuple_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(current).adjust(value); }\nint direct_const_pair_get_caller(const std::pair<Counter, Value> current, int value) { return std::get<0>(current).adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::direct_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::direct_const_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_get_receiver_categories_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexed_get_receiver_categories.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } int adjust(int value) const && { return value + 3; } }; int moved_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(std::move(current)).adjust(value); } int const_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(std::as_const(current)).adjust(value); } int forwarded_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(std::forward<std::tuple<Value, Counter>&&>(current)).adjust(value); } int decltype_auto_get_caller(std::tuple<Value, Counter> current, int value) { decltype(auto) nested = std::get<1>(std::move(current)); return nested.adjust(value); } int decltype_auto_moved_get_caller(std::tuple<Value, Counter> current, int value) { decltype(auto) nested = std::get<1>(std::move(current)); return std::move(nested).adjust(value); } int moved_optional_value_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(std::move(current)).value().adjust(value); } int moved_optional_arrow_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int moved_expected_value_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(std::move(current)).value().adjust(value); } int moved_expected_arrow_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int moved_expected_error_caller(std::tuple<Value, std::expected<Value, Counter>> current, int value) { return std::get<1>(std::move(current)).error().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_get_caller", "api::Counter::adjust(int) &&"),
        ("api::const_get_caller", "api::Counter::adjust(int) const &"),
        ("api::forwarded_get_caller", "api::Counter::adjust(int) &&"),
        (
            "api::decltype_auto_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_moved_get_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_expected_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_expected_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_expected_error_caller",
            "api::Counter::adjust(int) &&",
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_direct_indexed_variant_get_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("direct_indexed_variant_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_variant_get_caller(std::variant<Value, Counter> current, int value) { return std::get<1>(current).adjust(value); } int const_variant_get_caller(const std::variant<Counter, Value> current, int value) { return std::get<0>(current).adjust(value); } int direct_typed_variant_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(current).adjust(value); } int const_typed_variant_get_caller(const std::variant<Counter, Value> current, int value) { return std::get<Counter>(current).adjust(value); } int typed_tuple_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<Counter>(current).adjust(value); } int typed_unique_variant_get_caller(std::variant<Value, std::unique_ptr<Counter>> current, int value) { return std::get<std::unique_ptr<Counter>>(current)->adjust(value); } int typed_const_shared_variant_get_caller(std::variant<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<std::shared_ptr<const Counter>>(current)->adjust(value); } int typed_raw_pointer_variant_get_caller(std::variant<Value, Counter*> current, int value) { return std::get<Counter*>(current)->adjust(value); } int typed_const_reference_variant_get_caller(std::variant<std::reference_wrapper<const Counter>, Value> current, int value) { return std::get<std::reference_wrapper<const Counter>>(current).get().adjust(value); } int typed_weak_pointer_variant_get_caller(std::variant<Value, std::weak_ptr<Counter>> current, int value) { return std::get<std::weak_ptr<Counter>>(current).lock()->adjust(value); } int typed_optional_variant_get_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(current)->adjust(value); } int typed_const_expected_variant_get_caller(const std::variant<std::expected<Counter, Value>, Value> current, int value) { return std::get<std::expected<Counter, Value>>(current)->adjust(value); } int invalid_missing_typed_variant_get_caller(std::variant<Value, Counter> current, int value) { return std::get<std::unique_ptr<Counter>>(current)->adjust(value); } int invalid_duplicate_typed_tuple_get_caller(std::tuple<Counter, Counter> current, int value) { return std::get<Counter>(current).adjust(value); } int auto_variant_get_caller(std::variant<Value, Counter> current, int value) { auto nested = std::get<1>(current); return nested.adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::direct_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::direct_typed_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_typed_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::typed_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::typed_unique_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_const_shared_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_raw_pointer_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_const_reference_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_weak_pointer_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_optional_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_const_expected_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_variant_get_caller",
            "api::Counter::adjust(int) &",
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

    for caller in [
        "api::invalid_missing_typed_variant_get_caller",
        "api::invalid_duplicate_typed_tuple_get_caller",
    ] {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
        assert!(trace.callees.is_empty(), "{caller}");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
    for caller in [
        "api::invalid_missing_typed_variant_get_caller",
        "api::invalid_duplicate_typed_tuple_get_caller",
    ] {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert!(trace.callees.is_empty(), "{caller}");
    }
}

#[test]
fn resolves_cpp_typed_get_standard_value_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("typed_get_standard_value.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } int adjust(int value) const && { return value + 3; } }; int optional_value_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(current).value().adjust(value); } int expected_value_caller(std::variant<Value, std::expected<Counter, Value>> current, int value) { return std::get<std::expected<Counter, Value>>(current).value().adjust(value); } int const_expected_error_caller(const std::variant<Value, std::expected<Value, Counter>> current, int value) { return std::get<std::expected<Value, Counter>>(current).error().adjust(value); } int moved_typed_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(std::move(current)).adjust(value); } int const_typed_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(std::as_const(current)).adjust(value); } int forwarded_typed_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(std::forward<std::variant<Value, Counter>&&>(current)).adjust(value); } int moved_optional_value_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(std::move(current)).value().adjust(value); } int moved_expected_error_caller(std::variant<Value, std::expected<Value, Counter>> current, int value) { return std::get<std::expected<Value, Counter>>(std::move(current)).error().adjust(value); } int moved_optional_arrow_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(std::move(current))->adjust(value); } int moved_expected_arrow_caller(std::variant<Value, std::expected<Counter, Value>> current, int value) { return std::get<std::expected<Counter, Value>>(std::move(current))->adjust(value); } int optional_unique_caller(std::variant<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<std::optional<std::unique_ptr<Counter>>>(current)->adjust(value); } int expected_const_shared_caller(std::variant<std::expected<std::shared_ptr<const Counter>, Value>, Value> current, int value) { return std::get<std::expected<std::shared_ptr<const Counter>, Value>>(current)->adjust(value); } int shared_get_caller(std::variant<Value, std::shared_ptr<Counter>> current, int value) { return std::get<std::shared_ptr<Counter>>(current).get()->adjust(value); } int const_shared_get_caller(std::variant<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<std::shared_ptr<const Counter>>(current).get()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::optional_value_caller", "api::Counter::adjust(int) &"),
        ("api::expected_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_expected_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_typed_get_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::const_typed_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarded_typed_get_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_expected_error_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_expected_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::optional_unique_caller", "api::Counter::adjust(int) &"),
        (
            "api::expected_const_shared_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::shared_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn preserves_cpp_decltype_auto_typed_get_receiver_categories_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("decltype_auto_typed_get_receiver_categories.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int const_get_caller(const std::variant<Value, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(current); return nested.adjust(value); } int rvalue_reference_get_caller(std::variant<Value, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(std::move(current)); return nested.adjust(value); } int moved_get_caller(std::variant<Value, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(std::move(current)); return std::move(nested).adjust(value); } int optional_get_caller(const std::variant<Value, std::optional<Counter>> current, int value) { decltype(auto) nested = std::get<std::optional<Counter>>(current); return nested.value().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::const_get_caller", "api::Counter::adjust(int) const &"),
        (
            "api::rvalue_reference_get_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::moved_get_caller", "api::Counter::adjust(int) &&"),
        (
            "api::optional_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn does_not_resolve_invalid_cpp_typed_get_bindings() {
    let dir = temporary_dir();
    let source = dir.join("invalid_typed_get_bindings.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) { return value; } }; int missing_auto_caller(std::variant<Value, Counter> current, int value) { auto nested = std::get<std::unique_ptr<Counter>>(current); return nested->adjust(value); } int duplicate_decltype_auto_caller(std::tuple<Counter, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(current); return nested.adjust(value); } }\n",
    )
    .unwrap();

    for caller in [
        "api::missing_auto_caller",
        "api::duplicate_decltype_auto_caller",
    ] {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
        assert!(trace.callees.is_empty(), "{caller}");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "api::missing_auto_caller",
        "api::duplicate_decltype_auto_caller",
    ] {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert!(trace.callees.is_empty(), "{caller}");
    }
}

#[test]
fn resolves_cpp_direct_indexed_tuple_get_smart_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("direct_indexed_tuple_get_smart_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } int const_shared_pair_get_caller(std::pair<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<0>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_smart_pointer_get_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(current).get()->adjust(value); } int const_shared_pair_get_caller(std::pair<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<0>(current).get()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_reference_wrapper_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_reference_wrapper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::reference_wrapper<Counter>> current, int value) { return std::get<1>(current).get().adjust(value); } int const_pair_get_caller(std::pair<std::reference_wrapper<const Counter>, Value> current, int value) { return std::get<0>(current).get().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_raw_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_raw_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, Counter*> current, int value) { return std::get<1>(current)->adjust(value); } int const_pair_get_caller(std::pair<const Counter*, Value> current, int value) { return std::get<0>(current)->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_optional_value_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_optional_value.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_value_pair_get_caller(std::pair<std::optional<const Counter>, Value> current, int value) { return std::get<0>(current).value().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_value_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_value_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_value.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<const Counter, Value>, Value> current, int value) { return std::get<0>(current).value().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_value_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_value_smart_pointer_arrow_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_value_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int optional_unique_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int const_shared_pair_get_caller(std::pair<std::expected<std::shared_ptr<const Counter>, Value>, Value> current, int value) { return std::get<0>(current).value()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_smart_pointer_get_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value().get()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::shared_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current).error().get()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value().get()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_raw_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_raw_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<Counter*, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<const Counter*, Value>, Value> current, int value) { return std::get<0>(current).value()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, Counter*>> current, int value) { return std::get<1>(current).error()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, const Counter*>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_optional_raw_pointer_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_optional_raw_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<Counter*>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<const Counter*>, Value>, Value> current, int value) { return std::get<0>(current).value()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<Counter*>>> current, int value) { return std::get<1>(current).error()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<const Counter*>>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_error_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_error.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::expected<Value, Counter>> current, int value) { return std::get<1>(current).error().adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Value, Counter>> current, int value) { return std::get<1>(current).error().adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, const Counter>, Value> current, int value) { return std::get<0>(current).error().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_error_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_weak_pointer_lock_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_weak_pointer_lock.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::weak_ptr<Counter>> current, int value) { return std::get<1>(current).lock()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::weak_ptr<Counter>> current, int value) { return std::get<1>(current).lock()->adjust(value); } int const_pointee_pair_get_caller(std::pair<std::weak_ptr<const Counter>, Value> current, int value) { return std::get<0>(current).lock()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_optional_arrow_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_optional_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } int const_pointee_pair_get_caller(std::pair<std::optional<const Counter>, Value> current, int value) { return std::get<0>(current)->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointee_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_error_smart_pointer_arrow_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_error_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current).error()->adjust(value); } int optional_shared_pair_get_caller(std::pair<std::expected<Value, std::optional<std::shared_ptr<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } int const_shared_pair_get_caller(std::pair<std::expected<Value, std::shared_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current).error()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_weak_pointer_lock_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_weak_pointer_lock.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::weak_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value().lock()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::weak_ptr<const Counter>, Value>, Value> current, int value) { return std::get<0>(current).value().lock()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::weak_ptr<Counter>>> current, int value) { return std::get<1>(current).error().lock()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::weak_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current).error().lock()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_optional_weak_pointer_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_optional_weak_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::weak_ptr<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->lock()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<std::weak_ptr<const Counter>>, Value>, Value> current, int value) { return std::get<0>(current).value()->lock()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { return std::get<1>(current).error()->lock()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<std::weak_ptr<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->lock()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_optional_reference_wrapper_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_optional_reference_wrapper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::reference_wrapper<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->get().adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<std::reference_wrapper<const Counter>>, Value>, Value> current, int value) { return std::get<0>(current).value()->get().adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { return std::get<1>(current).error()->get().adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<std::reference_wrapper<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->get().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_optional_smart_pointer_get_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_optional_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::shared_ptr<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->get()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<std::unique_ptr<const Counter>>, Value>, Value> current, int value) { return std::get<0>(current).value()->get()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<std::shared_ptr<Counter>>>> current, int value) { return std::get<1>(current).error()->get()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<std::unique_ptr<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->get()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_sequence_element_access_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_sequence_element_access.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::vector<Counter>, Value>> current, int value) { return std::get<1>(current).value()[0].adjust(value); } int const_value_pair_get_caller(const std::pair<std::expected<std::vector<Counter>, Value>, Value> current, int value) { return std::get<0>(current).value().front().adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::deque<Counter>>> current, int value) { return std::get<1>(current).error().at(0).adjust(value); } int const_error_pair_get_caller(const std::pair<std::expected<Value, std::list<Counter>>, Value> current, int value) { return std::get<0>(current).error().back().adjust(value); } int value_data_tuple_get_caller(std::tuple<Value, std::expected<std::span<Counter>, Value>> current, int value) { return std::get<1>(current).value().data()->adjust(value); } int const_error_data_pair_get_caller(const std::pair<std::expected<Value, std::array<Counter, 2>>, Value> current, int value) { return std::get<0>(current).error().data()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::value_data_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_error_data_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_sequence_data_pointer_bindings_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_sequence_data_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int auto_value_caller(std::tuple<Value, std::expected<std::vector<Counter>, Value>> current, int value) { auto pointer = std::get<1>(current).value().data(); return pointer->adjust(value); } int decltype_auto_const_error_caller(const std::pair<std::expected<Value, std::span<Counter>>, Value> current, int value) { decltype(auto) pointer = std::get<0>(current).error().data(); return pointer->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::auto_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_auto_const_error_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_reference_wrapper_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_reference_wrapper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::reference_wrapper<Counter>, Value>> current, int value) { return std::get<1>(current).value().get().adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::reference_wrapper<const Counter>, Value>, Value> current, int value) { return std::get<0>(current).value().get().adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::reference_wrapper<Counter>>> current, int value) { return std::get<1>(current).error().get().adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::reference_wrapper<const Counter>>, Value> current, int value) { return std::get<0>(current).error().get().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_arrow_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current)->adjust(value); } int const_pointee_pair_get_caller(std::pair<std::expected<const Counter, Value>, Value> current, int value) { return std::get<0>(current)->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointee_pair_get_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_optional_smart_pointer_arrow_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_optional_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current)->adjust(value); } int const_shared_pair_get_caller(std::pair<std::optional<std::shared_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current)->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexed_tuple_get_expected_smart_pointer_arrow_calls_across_live_and_persisted_queries()
 {
    let dir = temporary_dir();
    let source = dir.join("indexed_tuple_get_expected_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current)->adjust(value); } int const_shared_pair_get_caller(std::pair<std::expected<std::shared_ptr<const Counter>, Value>, Value> current, int value) { return std::get<0>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current)->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_indexable_sequence_element_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("indexable_sequence_elements.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int vector_index_caller(std::vector<Counter> current, int value) { return current[0].adjust(value); } int vector_nested_index_caller(std::vector<Counter> current, std::array<int, 1> indexes, int value) { return current[indexes[0]].adjust(value); } int span_index_caller(std::span<const Counter> current, int value) { return current[0].adjust(value); } int array_index_caller(std::array<Counter, 2> current, int value) { return current[1].adjust(value); } int const_deque_index_caller(const std::deque<Counter> current, int value) { return current[0].adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::vector_index_caller", "api::Counter::adjust(int) &"),
        (
            "api::vector_nested_index_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::span_index_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::array_index_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_deque_index_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_wrapped_sequence_receiver_categories_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_sequence_receiver_categories.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int moved_front_caller(std::vector<Counter> current, int value) { return std::move(current).front().adjust(value); } int const_back_caller(std::vector<Counter> current, int value) { return std::as_const(current).back().adjust(value); } int forwarded_subscript_caller(std::array<Counter, 2> current, int value) { return std::forward<std::array<Counter, 2>&&>(current)[0].adjust(value); } int moved_data_caller(std::span<Counter> current, int value) { return std::move(current).data()->adjust(value); } int const_data_caller(std::vector<Counter> current, int value) { return std::as_const(current).data()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_front_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_back_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarded_subscript_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::moved_data_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_data_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_wrapped_weak_pointer_lock_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_weak_pointer_lock.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int moved_caller(std::weak_ptr<Counter> current, int value) { return std::move(current).lock()->adjust(value); } int const_caller(std::weak_ptr<Counter> current, int value) { return std::as_const(current).lock()->adjust(value); } int forwarded_caller(std::weak_ptr<Counter> current, int value) { return std::forward<std::weak_ptr<Counter>&&>(current).lock()->adjust(value); } int const_pointee_caller(std::weak_ptr<const Counter> current, int value) { return std::move(current).lock()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_wrapped_reference_wrapper_get_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_reference_wrapper_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int moved_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::move(current).get().adjust(value); } int const_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::as_const(current).get().adjust(value); } int forwarded_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::forward<std::reference_wrapper<Counter>&&>(current).get().adjust(value); } int const_pointee_caller(const Counter& target, int value) { std::reference_wrapper<const Counter> current(target); return std::move(current).get().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_wrapped_smart_pointer_get_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int moved_caller(std::shared_ptr<Counter> current, int value) { return std::move(current).get()->adjust(value); } int const_caller(std::shared_ptr<Counter> current, int value) { return std::as_const(current).get()->adjust(value); } int forwarded_caller(std::shared_ptr<Counter> current, int value) { return std::forward<std::shared_ptr<Counter>&&>(current).get()->adjust(value); } int const_pointee_caller(std::shared_ptr<const Counter> current, int value) { return std::move(current).get()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_direct_standard_pointer_cast_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("direct_standard_pointer_cast.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int get_if_caller(std::variant<Counter, Value> current, int value) { return std::get_if<Counter>(&current)->adjust(value); } int const_get_if_caller(std::variant<Counter, Value> current, int value) { return std::get_if<Counter>(std::addressof(std::as_const(current)))->adjust(value); } int any_cast_caller(std::any current, int value) { return std::any_cast<Counter>(&current)->adjust(value); } int const_any_cast_caller(std::any current, int value) { return std::any_cast<Counter>(std::addressof(std::as_const(current)))->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::get_if_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_get_if_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::any_cast_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_any_cast_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_wrapped_indexed_get_wrapper_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_indexed_get_wrappers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int moved_weak_caller(std::tuple<Value, std::weak_ptr<Counter>> current, int value) { return std::get<1>(std::move(current)).lock()->adjust(value); } int const_weak_caller(std::tuple<Value, std::weak_ptr<const Counter>> current, int value) { return std::get<1>(std::as_const(current)).lock()->adjust(value); } int forwarded_reference_caller(std::tuple<Value, std::reference_wrapper<Counter>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::reference_wrapper<Counter>>&&>(current)).get().adjust(value); } int const_reference_caller(std::tuple<Value, std::reference_wrapper<const Counter>> current, int value) { return std::get<1>(std::as_const(current)).get().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_weak_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_weak_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarded_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_reference_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_contiguous_sequence_data_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("contiguous_sequence_data.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int inline_data_caller(std::vector<Counter> current, int value) { return current.data()->adjust(value); } int auto_data_caller(std::array<Counter, 2> current, int value) { auto pointer = current.data(); return pointer->adjust(value); } int decltype_auto_data_caller(std::vector<Counter> current, int value) { decltype(auto) pointer = current.data(); return pointer->adjust(value); } int const_span_data_caller(std::span<const Counter> current, int value) { auto pointer = current.data(); return pointer->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::inline_data_caller", "api::Counter::adjust(int) &"),
        ("api::auto_data_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_auto_data_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_span_data_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_auto_constructor_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("auto_constructor_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nstruct Deleter {};\nusing Alias = Counter;\nAlias make_counter() { return Alias{}; }\nint lvalue_caller(int value) { auto current = Alias{}; return current.adjust(value); }\nint auto_reference_alias_caller(int value) { Alias target{}; auto& current = target; return current.adjust(value); }\nint auto_const_reference_alias_caller(int value) { Alias target{}; const auto& current = target; return current.adjust(value); }\nint auto_forwarding_reference_alias_caller(int value) { const Alias target{}; auto&& current = target; return current.adjust(value); }\nint direct_list_caller(int value) { auto current{Alias{}}; return current.adjust(value); }\nint copy_list_caller(int value) { auto current = {Alias{}}; return current.adjust(value); }\nint deduced_pointer_caller(int value) { auto current = new Alias{}; return current->adjust(value); }\nint parenthesized_deduced_pointer_caller(int value) { auto current = new Alias(); return current->adjust(value); }\nint default_deduced_pointer_caller(int value) { auto current = new Alias; return current->adjust(value); }\nint pointee_const_deduced_pointer_caller(int value) { auto current = new const Alias{}; return current->adjust(value); }\nint postfix_pointee_const_deduced_pointer_caller(int value) { auto current = new Alias const{}; return current->adjust(value); }\nint make_unique_caller(int value) { auto current = std::make_unique<Alias>(); return current->adjust(value); }\nint make_shared_caller(int value) { auto current = std::make_shared<Alias>(); return current->adjust(value); }\nint auto_unique_pointer_caller(int value) { auto current = std::unique_ptr<Alias>{}; return current->adjust(value); }\nint auto_const_unique_pointer_caller(int value) { const auto current = std::unique_ptr<Alias>{}; return current->adjust(value); }\nint unique_pointer_caller(int value) { std::unique_ptr<Alias> current; return current->adjust(value); }\nint unique_pointer_get_caller(int value) { std::unique_ptr<Alias> current; return current.get()->adjust(value); }\nint moved_unique_pointer_dereference_caller(int value) { std::unique_ptr<Alias> current; return (*std::move(current)).adjust(value); }\nint as_const_unique_pointer_dereference_caller(int value) { std::unique_ptr<Alias> current; return (*std::as_const(current)).adjust(value); }\nint forwarded_unique_pointer_dereference_caller(int value) { std::unique_ptr<Alias> current; return (*std::forward<std::unique_ptr<Alias>&&>(current)).adjust(value); }\nint reference_wrapper_get_caller(int value) { std::reference_wrapper<Alias> current = *static_cast<Alias*>(nullptr); return current.get().adjust(value); }\nint const_reference_wrapper_get_caller(int value) { std::reference_wrapper<const Alias> current = *static_cast<Alias*>(nullptr); return current.get().adjust(value); }\nint auto_reference_wrapper_caller(int value) { Alias target{}; auto current = std::reference_wrapper<Alias>(target); return current.get().adjust(value); }\nint auto_parenthesized_reference_wrapper_caller(int value) { Alias target{}; auto current = (std::reference_wrapper<Alias>(target)); return current.get().adjust(value); }\nint auto_const_reference_wrapper_caller(int value) { const Alias target{}; auto current = std::reference_wrapper<const Alias>(target); return current.get().adjust(value); }\nint ref_factory_caller(int value) { Alias target{}; return std::ref(target).get().adjust(value); }\nint parenthesized_ref_factory_caller(int value) { Alias target{}; return (std::ref(target)).get().adjust(value); }\nint cref_factory_caller(int value) { Alias target{}; return std::cref(target).get().adjust(value); }\nint ref_as_const_factory_caller(int value) { Alias target{}; return std::ref(std::as_const(target)).get().adjust(value); }\nint auto_ref_factory_caller(int value) { Alias target{}; auto current = std::ref(target); return current.get().adjust(value); }\nint auto_cref_factory_caller(int value) { Alias target{}; auto current = std::cref(target); return current.get().adjust(value); }\nint auto_ref_as_const_factory_caller(int value) { Alias target{}; auto current = std::ref(std::as_const(target)); return current.get().adjust(value); }\nint auto_addressof_caller(int value) { Alias target{}; auto current = std::addressof(target); return current->adjust(value); }\nint auto_const_addressof_caller(int value) { const Alias target{}; auto current = std::addressof(target); return current->adjust(value); }\nint auto_native_addressof_caller(int value) { Alias target{}; auto current = &target; return current->adjust(value); }\nint auto_const_native_addressof_caller(int value) { const Alias target{}; auto current = &target; return current->adjust(value); }\nint nested_wrapped_unique_pointer_get_caller(int value) { std::unique_ptr<Alias> current; return (std::move(std::as_const(current))).get()->adjust(value); }\nint forwarded_unique_pointer_get_caller(int value) { std::unique_ptr<Alias> current; return std::forward<std::unique_ptr<Alias>&>(current).get()->adjust(value); }\nint moved_unique_pointer_get_caller(int value) { std::unique_ptr<Alias> current; return std::move(current).get()->adjust(value); }\nint as_const_unique_pointer_get_caller(int value) { std::unique_ptr<Alias> current; return std::as_const(current).get()->adjust(value); }\nint custom_unique_pointer_caller(int value) { std::unique_ptr<Alias, Deleter> current; return current->adjust(value); }\nint shared_pointer_caller(int value) { std::shared_ptr<Alias> current; return current->adjust(value); }\nint const_unique_pointer_caller(int value) { std::unique_ptr<const Alias> current; return current->adjust(value); }\nint const_deduced_pointer_caller(int value) { const auto current = new Alias{}; return current->adjust(value); }\nint auto_pointer_caller(int value) { auto* current = new Alias{}; return current->adjust(value); }\nint const_auto_pointer_caller(int value) { const auto* current = new Alias{}; return current->adjust(value); }\nint const_lvalue_caller(int value) { const auto current = Alias{}; return current.adjust(value); }\nint const_reference_caller(int value) { const auto& current = Alias{}; return current.adjust(value); }\nint rvalue_reference_caller(int value) { auto&& current = Alias{}; return current.adjust(value); }\nint factory_caller(int value) { auto current = make_counter(); return current.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::lvalue_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_reference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_reference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_forwarding_reference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::direct_list_caller", "api::Counter::adjust(int) &"),
        ("api::deduced_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::parenthesized_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::default_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::pointee_const_deduced_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_pointee_const_deduced_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::make_unique_caller", "api::Counter::adjust(int) &"),
        ("api::make_shared_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_unique_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_unique_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::unique_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::unique_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_unique_pointer_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::as_const_unique_pointer_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::forwarded_unique_pointer_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::reference_wrapper_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_reference_wrapper_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_reference_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_parenthesized_reference_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_reference_wrapper_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::ref_factory_caller", "api::Counter::adjust(int) &"),
        (
            "api::parenthesized_ref_factory_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::cref_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::ref_as_const_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_ref_factory_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_cref_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_ref_as_const_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::auto_addressof_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_const_addressof_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_native_addressof_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_native_addressof_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::nested_wrapped_unique_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::forwarded_unique_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_unique_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::as_const_unique_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::custom_unique_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::shared_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_unique_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::auto_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_reference_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::rvalue_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::factory_caller", "api::make_counter()"),
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
    assert!(
        trace_symbol_graph(&dir, "api::copy_list_caller", TraceDirection::Both)
            .unwrap()
            .callees
            .is_empty()
    );
    assert!(
        trace_symbol_graph_from_index(&db_path, "api::copy_list_caller", TraceDirection::Both)
            .unwrap()
            .callees
            .is_empty()
    );
}

#[test]
fn resolves_cpp_auto_reference_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("auto_reference_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint mutable_alias_caller(int value) { Alias target{}; auto& current = target; return current.adjust(value); }\nint const_alias_caller(int value) { Alias target{}; const auto& current = target; return current.adjust(value); }\nint postfix_const_alias_caller(int value) { Alias target{}; auto const& current = target; return current.adjust(value); }\nint forwarding_alias_caller(int value) { const Alias target{}; auto&& current = target; return current.adjust(value); }\nint moved_alias_caller(int value) { Alias target{}; auto&& current = std::move(target); return current.adjust(value); }\nint as_const_alias_caller(int value) { Alias target{}; auto&& current = std::as_const(target); return current.adjust(value); }\nint forwarded_alias_caller(int value) { Alias target{}; auto&& current = std::forward<Alias&&>(target); return current.adjust(value); }\nint const_forwarded_alias_caller(int value) { Alias target{}; auto&& current = std::forward<const Alias&&>(target); return current.adjust(value); }\nint cast_alias_caller(int value) { Alias target{}; auto&& current = static_cast<Alias&&>(target); return current.adjust(value); }\nint const_cast_alias_caller(int value) { Alias target{}; auto&& current = static_cast<const Alias&&>(target); return current.adjust(value); }\nint pointer_alias_caller(Alias* pointer, int value) { auto& current = *pointer; return current.adjust(value); }\nint const_pointer_alias_caller(const Alias* pointer, int value) { auto&& current = *pointer; return current.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::mutable_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_const_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarding_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::moved_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::as_const_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::forwarded_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_forwarded_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::cast_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_cast_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::pointer_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointer_alias_caller",
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
fn resolves_cpp_reference_wrapper_get_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("reference_wrapper_get_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint wrapper_alias_caller(int value) { Alias target{}; std::reference_wrapper<Alias> wrapper(target); auto& current = wrapper.get(); return current.adjust(value); }\nint const_wrapper_alias_caller(int value) { const Alias target{}; std::reference_wrapper<const Alias> wrapper(target); auto&& current = wrapper.get(); return current.adjust(value); }\nint ref_alias_caller(int value) { Alias target{}; auto&& current = std::ref(target).get(); return current.adjust(value); }\nint cref_alias_caller(int value) { Alias target{}; auto&& current = std::cref(target).get(); return current.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::wrapper_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_wrapper_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::ref_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::cref_alias_caller",
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
fn resolves_cpp_optional_auto_value_copies_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_auto_value_copies.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint auto_value_caller(int value) { std::optional<Alias> current; auto current_value = current.value(); return current_value.adjust(value); }\nint const_auto_value_caller(int value) { std::optional<Alias> current; const auto current_value = current.value(); return current_value.adjust(value); }\nint copied_const_source_value_caller(int value) { const std::optional<Alias> current{}; auto current_value = current.value(); return current_value.adjust(value); }\nint auto_dereference_caller(int value) { std::optional<Alias> current; auto current_value = *current; return current_value.adjust(value); }\nint const_auto_dereference_caller(int value) { std::optional<Alias> current; const auto current_value = *current; return current_value.adjust(value); }\nint copied_const_source_dereference_caller(int value) { const std::optional<Alias> current{}; auto current_value = *current; return current_value.adjust(value); }\nint auto_pointer_value_caller(int value) { std::optional<std::shared_ptr<Alias>> current; auto current_value = current.value(); return current_value->adjust(value); }\nint auto_pointer_dereference_caller(int value) { std::optional<std::shared_ptr<Alias>> current; auto current_value = *current; return current_value->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::auto_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_dereference_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_pointer_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_pointer_dereference_caller",
            "api::Counter::adjust(int) &",
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
fn resolves_cpp_optional_value_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_value_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint value_alias_caller(int value) { std::optional<Alias> current; auto& alias = current.value(); return alias.adjust(value); }\nint const_value_alias_caller(int value) { const std::optional<Alias> current{}; auto&& alias = current.value(); return alias.adjust(value); }\nint moved_value_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::move(current).value(); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_value_alias_caller",
            "api::Counter::adjust(int) &",
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
fn resolves_cpp_optional_dereference_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_dereference_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint dereference_alias_caller(int value) { std::optional<Alias> current; auto& alias = *current; return alias.adjust(value); }\nint const_dereference_alias_caller(int value) { const std::optional<Alias> current{}; auto&& alias = *current; return alias.adjust(value); }\nint moved_dereference_alias_caller(int value) { std::optional<Alias> current; auto&& alias = *std::move(current); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_dereference_alias_caller",
            "api::Counter::adjust(int) &",
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
fn resolves_cpp_optional_wrapped_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_wrapped_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint moved_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::move(*current); return alias.adjust(value); }\nint as_const_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::as_const(*current); return alias.adjust(value); }\nint forwarded_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::forward<Alias&&>(*current); return alias.adjust(value); }\nint const_forwarded_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::forward<const Alias&&>(*current); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::as_const_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::forwarded_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_forwarded_alias_caller",
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
fn resolves_cpp_forwarded_base_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("forwarded_base_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Base { public: int adjust(int value) & { return value; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { Derived target{}; auto&& alias = std::forward<Base&&>(target); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"],
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
        vec!["api::Base::adjust(int) &"],
    );
}

#[test]
fn resolves_cpp_forwarded_optional_base_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("forwarded_optional_base_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Base { public: int adjust(int value) & { return value; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { std::optional<Derived> current; auto&& alias = std::forward<Base&&>(*current); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"],
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
        vec!["api::Base::adjust(int) &"],
    );
}

#[test]
fn resolves_cpp_cast_optional_base_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("cast_optional_base_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Base { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { std::optional<Derived> current; auto&& alias = static_cast<Base&&>(*current); return alias.adjust(value); }\nint const_caller(int value) { std::optional<Derived> current; auto&& alias = static_cast<const Base&&>(*current); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::caller", "api::Base::adjust(int) &"),
        ("api::const_caller", "api::Base::adjust(int) const &"),
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
fn resolves_cpp_addressof_reference_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("addressof_reference_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } };\nusing Alias = Counter;\nint caller(int value) { Alias target{}; auto& alias = *std::addressof(target); return alias.adjust(value); }\nint const_caller(int value) { const Alias target{}; auto&& alias = *std::addressof(target); return alias.adjust(value); }\nint wrapped_const_caller(int value) { Alias target{}; auto& alias = *std::addressof(std::as_const(target)); return alias.adjust(value); }\nint native_caller(int value) { Alias target{}; auto& alias = *&target; return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        (
            "api::wrapped_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::native_caller", "api::Counter::adjust(int) &"),
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
fn resolves_cpp_cast_addressof_reference_aliases_with_the_cast_static_type() {
    let dir = temporary_dir();
    let source = dir.join("cast_addressof_reference_alias.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Base { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { Derived target{}; auto& alias = *std::addressof(static_cast<Base&>(target)); return alias.adjust(value); }\nint const_caller(int value) { Derived target{}; auto& alias = *std::addressof(std::as_const(static_cast<const Base&>(target))); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::caller", "api::Base::adjust(int) &"),
        ("api::const_caller", "api::Base::adjust(int) const &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_volatile_const_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("volatile_const_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter { public: int adjust(int value) & { return value; } int adjust(int value) volatile const & { return value + 1; } int const_caller(int value) volatile const { return adjust(value); } };\nint caller(int value) { const Counter current{}; return current.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::caller", "api::Counter::adjust(int) volatile const &"),
        (
            "api::Counter::const_caller(int) volatile const",
            "api::Counter::adjust(int) volatile const &",
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_decltype_auto_reference_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("decltype_auto_reference_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } };\nusing Alias = Counter;\nint copied_caller(int value) { Alias target{}; decltype(auto) alias = target; return alias.adjust(value); }\nint copied_const_caller(int value) { const Alias target{}; decltype(auto) alias = target; return alias.adjust(value); }\nint parenthesized_caller(int value) { Alias target{}; decltype(auto) alias = (target); return alias.adjust(value); }\nint const_caller(int value) { const Alias target{}; decltype(auto) alias = (target); return alias.adjust(value); }\nint moved_caller(int value) { Alias target{}; decltype(auto) alias = std::move(target); return alias.adjust(value); }\nint pointer_caller(int value) { Alias* pointer = nullptr; decltype(auto) alias = *pointer; return alias.adjust(value); }\nint optional_caller(int value) { std::optional<Alias> current; decltype(auto) alias = current.value(); return alias.adjust(value); }\nint wrapper_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); decltype(auto) alias = current.get(); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::copied_caller", "api::Counter::adjust(int) &"),
        (
            "api::copied_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::parenthesized_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        ("api::optional_caller", "api::Counter::adjust(int) &"),
        ("api::wrapper_caller", "api::Counter::adjust(int) &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn preserves_cpp_decltype_auto_parenthesized_binding_access_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("decltype_auto_parenthesized_bindings.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter { public: int adjust(int value) & { return value; } };\nusing Alias = Counter;\nint pointer_caller(int value) { Alias* current = nullptr; decltype(auto) alias = (current); return alias->adjust(value); }\nint optional_caller(int value) { std::optional<Alias> current; decltype(auto) alias = (current); return alias->adjust(value); }\nint wrapper_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); decltype(auto) alias = (current); return alias.get().adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        ("api::optional_caller", "api::Counter::adjust(int) &"),
        ("api::wrapper_caller", "api::Counter::adjust(int) &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_smart_pointer_dereference_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("smart_pointer_dereference_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint unique_alias_caller(int value) { std::unique_ptr<Alias> current; auto& alias = *current; return alias.adjust(value); }\nint const_shared_alias_caller(int value) { std::shared_ptr<const Alias> current; auto&& alias = *current; return alias.adjust(value); }\nint unique_copy_caller(int value) { std::unique_ptr<Alias> current; auto alias = *current; return alias.adjust(value); }\nint const_shared_copy_caller(int value) { std::shared_ptr<const Alias> current; auto alias = *current; return alias.adjust(value); }\nint unique_get_copy_caller(int value) { std::unique_ptr<Alias> current; auto pointer = current.get(); return pointer->adjust(value); }\nint const_shared_get_copy_caller(int value) { std::shared_ptr<const Alias> current; auto pointer = current.get(); return pointer->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::unique_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::unique_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::unique_get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_get_copy_caller",
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
fn resolves_cpp_range_for_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("range_for_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint caller() { for (Alias current : values) { return current.adjust(1); } return 0; }\nint const_caller() { for (const Alias current : values) { return current.adjust(1); } return 0; }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
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
fn resolves_cpp_condition_binding_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("condition_binding_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    operator bool() const { return true; }\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nAlias make_counter() { return Alias{}; }\nint if_caller(int value) { if (Alias current = make_counter()) { return current.adjust(value); } else { return current.adjust(value); } }\nint const_switch_caller(int value) { switch (const Alias current = make_counter()) { default: return current.adjust(value); } }\nint while_caller(int value) { while (Alias current = make_counter()) { return current.adjust(value); } return value; }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::if_caller",
            ["api::Counter::adjust(int) &", "api::make_counter()"],
        ),
        (
            "api::const_switch_caller",
            ["api::Counter::adjust(int) const &", "api::make_counter()"],
        ),
        (
            "api::while_caller",
            ["api::Counter::adjust(int) &", "api::make_counter()"],
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
            expected_callee,
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
            expected_callee,
            "{caller}",
        );
    }
}

#[test]
fn resolves_cpp_parameter_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("parameter_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nusing Alias = Counter;\nint lvalue_caller(Alias& current, int value) { return current.adjust(value); }\nint const_lvalue_caller(const Alias& current, int value) { return current.adjust(value); }\nint postfix_const_lvalue_caller(Alias const& current, int value) { return current.adjust(value); }\nint rvalue_reference_caller(Alias&& current, int value) { return current.adjust(value); }\nint moved_rvalue_reference_caller(Alias&& current, int value) { return std::move(current).adjust(value); }\nint moved_caller(Alias& current, int value) { return std::move(current).adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::lvalue_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::rvalue_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_rvalue_reference_caller",
            "api::Counter::adjust(int) &&",
        ),
        ("api::moved_caller", "api::Counter::adjust(int) &&"),
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
fn resolves_cpp_pointer_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("pointer_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint parameter_caller(Alias* current, int value) { return current->adjust(value); }\nint const_parameter_caller(const Alias* current, int value) { return current->adjust(value); }\nint postfix_const_parameter_caller(Alias const* current, int value) { return current->adjust(value); }\nint const_pointer_parameter_caller(Alias* const current, int value) { return current->adjust(value); }\nint pointer_reference_caller(Alias* const& current, int value) { return current->adjust(value); }\nint const_pointer_local_caller(int value) { Alias* const current = nullptr; return current->adjust(value); }\nint local_caller(int value) { Alias* current = nullptr; return current->adjust(value); }\nint dereference_caller(Alias* current, int value) { return (*current).adjust(value); }\nint addressof_local_caller(int value) { Alias current{}; return std::addressof(current)->adjust(value); }\nint addressof_const_local_caller(int value) { const Alias current{}; return std::addressof(current)->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::parameter_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_parameter_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_const_parameter_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointer_parameter_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::pointer_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_pointer_local_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::local_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::addressof_local_caller", "api::Counter::adjust(int) &"),
        (
            "api::addressof_const_local_caller",
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
fn resolves_cpp_wrapped_pointer_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_pointer_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint moved_parameter_caller(Alias* current, int value) { return std::move(current)->adjust(value); }\nint as_const_parameter_caller(Alias* current, int value) { return std::as_const(current)->adjust(value); }\nint forwarded_const_parameter_caller(const Alias* current, int value) { return std::forward<const Alias*&>(current)->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_parameter_caller", "api::Counter::adjust(int) &"),
        (
            "api::as_const_parameter_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::forwarded_const_parameter_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_optional_smart_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_optional_smart_pointer_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_value_get_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return current.error().value().get()->adjust(value); }\nint error_dereference_get_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return (*current.error()).get()->adjust(value); }\nint value_value_get_caller(std::expected<std::optional<std::shared_ptr<Counter>>, Value> current, int value) { return current.value().value().get()->adjust(value); }\nint value_dereference_get_caller(std::expected<std::optional<std::shared_ptr<Counter>>, Value> current, int value) { return (*current.value()).get()->adjust(value); }\nint error_value_arrow_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return current.error().value()->adjust(value); }\nint error_dereference_arrow_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return (*current.error())->adjust(value); }\nint const_error_pointee_caller(std::expected<Value, std::optional<std::shared_ptr<const Counter>>> current, int value) { return (*current.error()).get()->adjust(value); }\nint get_copy_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { auto pointer = current.error().value().get(); return pointer->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_value_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::error_dereference_get_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_value_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::value_dereference_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::error_value_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::error_dereference_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_error_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_optional_expected_nested_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_expected_nested_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint value_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return current.value().value().adjust(value); }\nint dereference_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return (*current).value().adjust(value); }\nint value_error_caller(std::optional<std::expected<Value, Counter>> current, int value) { return current.value().error().adjust(value); }\nint arrow_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return current->value().adjust(value); }\nint smart_pointer_value_get_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return current.value().value().get()->adjust(value); }\nint smart_pointer_arrow_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return (*current).value()->adjust(value); }\nint nested_optional_value_arrow_caller(std::optional<std::expected<std::optional<Counter>, Value>> current, int value) { return (*current).value()->adjust(value); }\nint nested_optional_value_value_caller(std::optional<std::expected<std::optional<Counter>, Value>> current, int value) { return current.value().value().value().adjust(value); }\nint const_value_value_caller(const std::optional<std::expected<Counter, Value>> current, int value) { return current.value().value().adjust(value); }\nint const_arrow_error_caller(const std::optional<std::expected<Value, Counter>> current, int value) { return current->error().adjust(value); }\nint arrow_error_smart_pointer_get_caller(std::optional<std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return current->error().get()->adjust(value); }\nint arrow_error_smart_pointer_arrow_caller(std::optional<std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return current->error()->adjust(value); }\nint arrow_error_reference_wrapper_caller(std::optional<std::expected<Value, std::reference_wrapper<Counter>>> current, int value) { return current->error().get().adjust(value); }\nint arrow_error_weak_pointer_caller(std::optional<std::expected<Value, std::weak_ptr<Counter>>> current, int value) { return current->error().lock()->adjust(value); }\nint auto_arrow_error_caller(std::optional<std::expected<Value, Counter>> current, int value) { auto nested = current->error(); return nested.adjust(value); }\nint auto_const_arrow_error_caller(const std::optional<std::expected<Value, Counter>> current, int value) { auto nested = current->error(); return nested.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::dereference_value_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_error_caller", "api::Counter::adjust(int) &"),
        ("api::arrow_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::smart_pointer_value_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::smart_pointer_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_value_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_value_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_value_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_arrow_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::arrow_error_smart_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::arrow_error_smart_pointer_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::arrow_error_reference_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::arrow_error_weak_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_arrow_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_arrow_error_caller",
            "api::Counter::adjust(int) &",
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_nested_optional_expected_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("nested_optional_expected_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nint nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return (*current)->value().adjust(value); }\nint nested_opt_opt_exp_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return current.value().value().value().adjust(value); }\nint nested_opt_opt_exp_double_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return current->value()->value().adjust(value); }\nint nested_opt_opt_exp_deref_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return (**current).value().adjust(value); }\nint nested_opt_opt_exp_error_arrow_caller(std::optional<std::optional<std::expected<Value, Counter>>> current, int value) { return (*current)->error().adjust(value); }\nint nested_opt_opt_exp_error_value_caller(std::optional<std::optional<std::expected<Value, Counter>>> current, int value) { return current.value().value().error().adjust(value); }\nint nested_opt_opt_exp_auto_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { auto nested = (*current)->value(); return nested.adjust(value); }\nint moved_nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return std::move(*current)->value().adjust(value); }\nint as_const_nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return std::as_const(*current)->value().adjust(value); }\nint exp_opt_exp_error_caller(std::expected<std::optional<std::expected<Value, Counter>>, Value> current, int value) { return current.value().value().error().adjust(value); }\nint exp_opt_exp_error_arrow_caller(std::expected<std::optional<std::expected<Value, Counter>>, Value> current, int value) { return (*current)->error().adjust(value); }\nint opt_exp_error_opt_sp_arrow_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { return current->error()->adjust(value); }\nint opt_exp_error_opt_sp_get_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { return current->error().value().get()->adjust(value); }\nint opt_exp_error_opt_weak_arrow_caller(std::optional<std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { return current->error()->lock()->adjust(value); }\nint opt_exp_error_opt_ref_get_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { return current->error()->get().adjust(value); }\nint opt_exp_opt_exp_error_caller(std::optional<std::expected<std::optional<std::expected<Value, Counter>>, Value>> current, int value) { return current->value()->error().adjust(value); }\nint exp_error_opt_exp_value_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { return current.error()->value().adjust(value); }\nint exp_error_opt_exp_arrow_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { return (*current.error())->adjust(value); }\nint auto_opt_exp_error_opt_sp_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { auto nested = current->error(); return nested->adjust(value); }\nint decltype_auto_exp_error_opt_exp_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { decltype(auto) nested = (*current.error())->value(); return nested.adjust(value); }\nint decltype_auto_opt_exp_error_opt_sp_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) nested = current->error(); return nested->adjust(value); }\nint decltype_auto_exp_error_opt_exp_arrow_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { decltype(auto) nested = (*current.error()); return nested->adjust(value); }\nint decltype_auto_opt_exp_error_opt_weak_lock_caller(std::optional<std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { decltype(auto) nested = current->error()->lock(); return nested->adjust(value); }\nint decltype_auto_opt_exp_value_sp_get_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { decltype(auto) pointer = current->value().get(); return pointer->adjust(value); }\nint decltype_auto_const_opt_exp_error_opt_sp_get_caller(const std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) pointer = current->error().value().get(); return pointer->adjust(value); }\nint decltype_auto_opt_exp_error_opt_sp_get_arrow_caller(std::optional<std::expected<Value, std::optional<std::shared_ptr<Counter>>>> current, int value) { decltype(auto) pointer = current->error()->get(); return pointer->adjust(value); }\nint decltype_auto_opt_opt_sp_arrow_caller(std::optional<std::optional<std::unique_ptr<Counter>>> current, int value) { decltype(auto) nested = *current; return nested->adjust(value); }\nint decltype_auto_opt_exp_error_opt_sp_deref_arrow_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) nested = *current->error(); return nested->adjust(value); }\nint auto_opt_exp_error_opt_ref_via_nested_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { auto nested = current->error(); return nested->get().adjust(value); }\nint decltype_auto_opt_exp_error_opt_ref_via_nested_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { decltype(auto) nested = current->error(); return nested->get().adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::nested_opt_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_double_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_deref_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_error_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_error_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_auto_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_nested_opt_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::as_const_nested_opt_opt_exp_arrow_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::exp_opt_exp_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::exp_opt_exp_error_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_error_opt_sp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_error_opt_sp_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_error_opt_weak_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_error_opt_ref_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_opt_exp_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::exp_error_opt_exp_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::exp_error_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_opt_exp_error_opt_sp_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_exp_error_opt_exp_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_sp_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_exp_error_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_weak_lock_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_value_sp_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_const_opt_exp_error_opt_sp_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_sp_get_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_opt_sp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_sp_deref_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_opt_exp_error_opt_ref_via_nested_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_ref_via_nested_caller",
            "api::Counter::adjust(int) &",
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_optional_reference_wrapper_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_optional_reference_wrapper_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_value_caller(std::expected<Value, std::optional<std::reference_wrapper<Counter>>> current, int value) { return current.error().value().get().adjust(value); }\nint error_dereference_caller(std::expected<Value, std::optional<std::reference_wrapper<Counter>>> current, int value) { return (*current.error()).get().adjust(value); }\nint value_value_caller(std::expected<std::optional<std::reference_wrapper<Counter>>, Value> current, int value) { return current.value().value().get().adjust(value); }\nint value_dereference_caller(std::expected<std::optional<std::reference_wrapper<Counter>>, Value> current, int value) { return (*current.value()).get().adjust(value); }\nint const_error_pointee_caller(std::expected<Value, std::optional<std::reference_wrapper<const Counter>>> current, int value) { return (*current.error()).get().adjust(value); }\nint get_copy_caller(std::expected<Value, std::optional<std::reference_wrapper<Counter>>> current, int value) { auto target = current.error().value().get(); return target.adjust(value); }\nint dereference_get_copy_caller(std::expected<std::optional<std::reference_wrapper<Counter>>, Value> current, int value) { auto target = (*current.value()).get(); return target.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::error_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::value_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_error_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::dereference_get_copy_caller",
            "api::Counter::adjust(int) &",
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_expected_optional_weak_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("expected_optional_weak_pointer_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint error_value_caller(std::expected<Value, std::optional<std::weak_ptr<Counter>>> current, int value) { return current.error().value().lock()->adjust(value); }\nint error_dereference_caller(std::expected<Value, std::optional<std::weak_ptr<Counter>>> current, int value) { return (*current.error()).lock()->adjust(value); }\nint value_value_caller(std::expected<std::optional<std::weak_ptr<Counter>>, Value> current, int value) { return current.value().value().lock()->adjust(value); }\nint value_dereference_caller(std::expected<std::optional<std::weak_ptr<Counter>>, Value> current, int value) { return (*current.value()).lock()->adjust(value); }\nint const_error_pointee_caller(std::expected<Value, std::optional<std::weak_ptr<const Counter>>> current, int value) { return (*current.error()).lock()->adjust(value); }\nint lock_copy_caller(std::expected<Value, std::optional<std::weak_ptr<Counter>>> current, int value) { auto shared = current.error().value().lock(); return shared->adjust(value); }\nint dereference_lock_copy_caller(std::expected<std::optional<std::weak_ptr<Counter>>, Value> current, int value) { auto shared = (*current.value()).lock(); return shared->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::error_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::error_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::value_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_error_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::lock_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::dereference_lock_copy_caller",
            "api::Counter::adjust(int) &",
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
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_optional_reference_wrapper_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_reference_wrapper_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint value_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { return current.value().get().adjust(value); }\nint dereference_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { return (*current).get().adjust(value); }\nint moved_value_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { return std::move(current).value().get().adjust(value); }\nint const_pointee_caller(std::optional<std::reference_wrapper<const Counter>> current, int value) { return (*current).get().adjust(value); }\nint get_alias_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { auto& target = (*current).get(); return target.adjust(value); }\nint get_copy_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { auto target = current.value().get(); return target.adjust(value); }\nint const_get_copy_caller(std::optional<std::reference_wrapper<const Counter>> current, int value) { auto target = (*current).get(); return target.adjust(value); }\nint const_auto_get_copy_caller(std::optional<std::reference_wrapper<Counter>> current, int value) { const auto target = current.value().get(); return target.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::get_alias_caller", "api::Counter::adjust(int) &"),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        ("api::const_get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_get_copy_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_optional_weak_pointer_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_weak_pointer_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint value_caller(std::optional<std::weak_ptr<Counter>> current, int value) { return current.value().lock()->adjust(value); }\nint dereference_caller(std::optional<std::weak_ptr<Counter>> current, int value) { return (*current).lock()->adjust(value); }\nint moved_value_caller(std::optional<std::weak_ptr<Counter>> current, int value) { return std::move(current).value().lock()->adjust(value); }\nint const_pointee_caller(std::optional<std::weak_ptr<const Counter>> current, int value) { return (*current).lock()->adjust(value); }\nint lock_copy_caller(std::optional<std::weak_ptr<Counter>> current, int value) { auto shared = current.value().lock(); return shared->adjust(value); }\nint dereference_lock_copy_caller(std::optional<std::weak_ptr<Counter>> current, int value) { auto shared = (*current).lock(); return shared->adjust(value); }\nint const_lock_copy_caller(std::optional<std::weak_ptr<const Counter>> current, int value) { auto shared = (*current).lock(); return shared->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::lock_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::dereference_lock_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_lock_copy_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
}

#[test]
fn resolves_cpp_optional_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nusing Alias = Counter;\nint arrow_caller(int value) { std::optional<Alias> current; return current->adjust(value); }\nint auto_arrow_caller(int value) { auto current = std::optional<Alias>{}; return current->adjust(value); }\nint auto_const_arrow_caller(int value) { const auto current = std::optional<Alias>{}; return current->adjust(value); }\nint nested_unique_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return (*current)->adjust(value); }\nint nested_unique_value_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return current.value()->adjust(value); }\nint moved_arrow_caller(int value) { std::optional<Alias> current; return std::move(current)->adjust(value); }\nint as_const_arrow_caller(int value) { std::optional<Alias> current; return std::as_const(current)->adjust(value); }\nint forwarded_arrow_caller(int value) { std::optional<Alias> current; return std::forward<std::optional<Alias>&&>(current)->adjust(value); }\nint value_caller(int value) { std::optional<Alias> current; return current.value().adjust(value); }\nint dereference_caller(int value) { std::optional<Alias> current; return (*current).adjust(value); }\nint moved_value_caller(int value) { std::optional<Alias> current; return std::move(current).value().adjust(value); }\nint moved_dereference_caller(int value) { std::optional<Alias> current; return (*std::move(current)).adjust(value); }\nint as_const_value_caller(int value) { std::optional<Alias> current; return std::as_const(current).value().adjust(value); }\nint as_const_dereference_caller(int value) { std::optional<Alias> current; return (*std::as_const(current)).adjust(value); }\nint forwarded_value_caller(int value) { std::optional<Alias> current; return std::forward<std::optional<Alias>&&>(current).value().adjust(value); }\nint forwarded_dereference_caller(int value) { std::optional<Alias> current; return (*std::forward<std::optional<Alias>&&>(current)).adjust(value); }\nint const_arrow_caller(int value) { const std::optional<Alias> current{}; return current->adjust(value); }\nint const_value_caller(int value) { const std::optional<Alias> current{}; return current.value().adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::arrow_caller", "api::Counter::adjust(int) &"),
        ("api::auto_arrow_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_const_arrow_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::nested_unique_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_unique_value_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::moved_arrow_caller", "api::Counter::adjust(int) &"),
        (
            "api::as_const_arrow_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::forwarded_arrow_caller", "api::Counter::adjust(int) &"),
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &&"),
        (
            "api::moved_dereference_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::as_const_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::as_const_dereference_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarded_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::forwarded_dereference_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::const_arrow_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_value_caller",
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
            "{caller}",
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
