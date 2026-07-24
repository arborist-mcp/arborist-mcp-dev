use super::*;

#[test]
fn refreshes_unqualified_cpp_using_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("definitions.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace api { namespace base { int convert(int value) { return value + 1; } } }\n",
    )
    .unwrap();
    fs::write(&caller, "namespace api { int caller() { return 0; } }\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &caller,
        "namespace api { using base::convert; int caller() { return convert(1); } }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &caller).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let trace =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::base::convert(int)"]
    );
}

#[test]
fn refreshes_cpp_moved_this_member_call_dependencies() {
    let dir = temporary_dir();
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) && { return value + 1; } int caller(int value) && { return value; } }; }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) && { return value + 1; } int caller(int value) && { return std::move(*this).adjust(value); } }; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &source).unwrap();
    assert_eq!(stats.rebuilt_files, 1);

    let trace =
        trace_symbol_graph_from_index(&db_path, "api::Counter::caller", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) &&"]
    );
}

#[test]
fn refreshes_cpp_forward_const_this_member_call_dependencies() {
    let dir = temporary_dir();
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) const & { return value + 1; } int adjust(int value) & { return value; } int caller(int value) { return value; } }; }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) const & { return value + 1; } int adjust(int value) & { return value; } int caller(int value) { return std::forward<Counter const &>(*this).adjust(value); } }; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &source).unwrap();
    assert_eq!(stats.rebuilt_files, 1);

    let trace =
        trace_symbol_graph_from_index(&db_path, "api::Counter::caller", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) const &"]
    );
}

#[test]
fn refreshes_qualified_cpp_constructor_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("definitions.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int value) {} }; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "api::Counter caller(int value) { return api::Counter{value}; }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );

    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int left, int right) {} }; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &definitions).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}

#[test]
fn refreshes_cpp_new_constructor_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("counter.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int value) {} }; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "int caller(int value) { auto counter = new api::Counter(value); return value; }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );

    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int left, int right) {} }; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &definitions).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}

#[test]
fn refreshes_cpp_default_new_constructor_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("counter.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter() {} }; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "int caller() { auto counter = new api::Counter; return 0; }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter()"]
    );

    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int value) {} }; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &definitions).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}

#[test]
fn refreshes_cpp_braced_initializer_constructor_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("counter.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int value) {} }; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "int caller(int value) { api::Counter counter{value}; return value; }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );

    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int left, int right) {} }; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &definitions).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}

#[test]
fn refreshes_cpp_type_alias_constructor_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("counter.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int value) {} }; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace app { using First = api::Counter; using Alias = First; int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );

    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int left, int right) {} }; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &definitions).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}

#[test]
fn refreshes_cpp_typedef_constructor_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("counter.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int value) {} }; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace app { typedef api::Counter Alias; int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );

    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int left, int right) {} }; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &definitions).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}

#[test]
fn refreshes_cpp_cv_qualified_type_alias_constructor_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("counter.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int value) {} }; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace app { using Alias = volatile api::Counter; int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );

    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int left, int right) {} }; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &definitions).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}

#[test]
fn refreshes_cpp_header_type_alias_constructor_call_dependencies() {
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
        "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );

    fs::write(
        &header,
        "namespace api { class Counter { public: Counter(int left, int right) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &header).unwrap();
    assert!(stats.rebuilt_files >= 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}

#[test]
fn refreshes_cpp_template_type_alias_constructor_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("box.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace api { template <typename T> class Box { public: Box(T value) {} }; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace app { template <typename T> using Alias = api::Box<T>; int caller(int value) { Alias<int> box{value}; return value; } }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
    );

    fs::write(
        &definitions,
        "namespace api { template <typename T> class Box { public: Box(T left, T right) {} }; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &definitions).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}

#[test]
fn refreshes_template_cpp_braced_initializer_constructor_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("box.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int value) {} };\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "int caller(int value) { api::Box<int> box{value}; return value; }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box<int>::Box(int)"]
    );

    fs::write(
        &definitions,
        "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int left, int right) {} };\n}\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &definitions).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}

#[test]
fn refreshes_template_cpp_new_constructor_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("box.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int value) {} };\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "int caller(int value) { auto box = new api::Box<int>(value); return value; }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box<int>::Box(int)"]
    );

    fs::write(
        &definitions,
        "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int left, int right) {} };\n}\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &definitions).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}

#[test]
fn refreshes_template_cpp_constructor_call_dependencies() {
    let dir = temporary_dir();
    let definitions = dir.join("box.cpp");
    let caller = dir.join("caller.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &definitions,
        "namespace detail { struct Tag {}; }\nnamespace api { template <typename T> class Box { public: Box(T value) {} }; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "int caller(int value) { auto box = api::Box<detail::Tag>{value}; return value; }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        initial_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
    );

    fs::write(
        &definitions,
        "namespace detail { struct Tag {}; }\nnamespace api { template <typename T> class Box { public: Box(T left, T right) {} }; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &definitions).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    let refreshed_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert!(refreshed_trace.callees.is_empty());
}
