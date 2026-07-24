use super::*;

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
