use std::fs;

use rusqlite::Connection;

use super::support::{
    create_legacy_symbol_index_schema_without_reference_names, create_minimal_symbol_index_schema,
    symbol_table_columns, temporary_dir,
};
use crate::language::normalize_path;
use crate::{
    TraceDirection, rebuild_symbol_index, refresh_symbol_index_for_file,
    trace_symbol_graph_from_index,
};
#[test]
fn refreshes_single_file_symbol_index() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &helper,
            "def helper(value: int) -> int:\n    return leaf(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
            &helper,
            "def helper(value: int) -> int:\n    return branch(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
        )
        .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    let trace = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "branch")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "leaf")
    );
}

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

#[test]
fn refreshes_c_include_dependents_for_header_change() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let wrapper_header = dir.join("wrapper.h");
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
    fs::write(&wrapper_header, "#include \"alpha.h\"\n").unwrap();
    fs::write(
        &caller,
        "#include \"wrapper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(initial_trace.callees.len(), 1);
    assert_eq!(
        initial_trace.callees[0].file_path,
        alpha_source.to_string_lossy().replace('\\', "/")
    );

    fs::write(&wrapper_header, "#include \"zeta.h\"\n").unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &wrapper_header).unwrap();
    assert_eq!(stats.indexed_files, 6);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 4);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(updated_trace.callees.len(), 1);
    assert_eq!(
        updated_trace.callees[0].file_path,
        zeta_source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn refreshes_c_include_dependents_for_parent_relative_header() {
    let dir = temporary_dir();
    let include_dir = dir.join("include");
    let source_dir = dir.join("src");
    let alpha_header = include_dir.join("alpha.h");
    let alpha_source = include_dir.join("alpha.c");
    let zeta_header = include_dir.join("zeta.h");
    let zeta_source = include_dir.join("zeta.c");
    let wrapper_header = include_dir.join("wrapper.h");
    let caller = source_dir.join("caller.c");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&include_dir).unwrap();
    fs::create_dir_all(&source_dir).unwrap();
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
    fs::write(&wrapper_header, "#include \"alpha.h\"\n").unwrap();
    fs::write(
            &caller,
            "#include \"../include/wrapper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(initial_trace.callees.len(), 1);
    assert_eq!(
        initial_trace.callees[0].file_path,
        alpha_source.to_string_lossy().replace('\\', "/")
    );

    fs::write(&wrapper_header, "#include \"zeta.h\"\n").unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &wrapper_header).unwrap();
    assert_eq!(stats.indexed_files, 6);
    assert_eq!(stats.rebuilt_files, 2);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(updated_trace.callees.len(), 1);
    assert_eq!(
        updated_trace.callees[0].file_path,
        zeta_source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn refreshes_c_include_dependents_for_hpp_header_change() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.HPP");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.HPP");
    let zeta_source = dir.join("zeta.c");
    let wrapper_header = dir.join("wrapper.hpp");
    let caller = dir.join("caller.c");
    let db_path = dir.join("symbols.db");

    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.HPP\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.HPP\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(&wrapper_header, "#include \"alpha.HPP\"\n").unwrap();
    fs::write(
        &caller,
        "#include \"wrapper.hpp\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(initial_trace.callees.len(), 1);
    assert_eq!(
        initial_trace.callees[0].file_path,
        alpha_source.to_string_lossy().replace('\\', "/")
    );

    fs::write(&wrapper_header, "#include \"zeta.HPP\"\n").unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &wrapper_header).unwrap();
    assert_eq!(stats.indexed_files, 6);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 4);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(updated_trace.callees.len(), 1);
    assert_eq!(
        updated_trace.callees[0].file_path,
        zeta_source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn refreshes_c_include_dependents_for_deleted_header() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let wrapper_header = dir.join("wrapper.h");
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
    fs::write(&wrapper_header, "#include \"alpha.h\"\n").unwrap();
    fs::write(
        &caller,
        "#include \"wrapper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(initial_trace.callees.len(), 1);
    assert_eq!(
        initial_trace.callees[0].file_path,
        alpha_source.to_string_lossy().replace('\\', "/")
    );

    fs::remove_file(&wrapper_header).unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &wrapper_header).unwrap();
    assert_eq!(stats.indexed_files, 5);
    assert_eq!(stats.rebuilt_files, 2);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(updated_trace.callees.len(), 1);
    assert_eq!(
        updated_trace.callees[0].file_path,
        zeta_source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn does_not_refresh_dependents_for_missing_system_include() {
    let dir = temporary_dir();
    let helper_header = dir.join("helper.h");
    let helper_source = dir.join("helper.c");
    let caller = dir.join("caller.c");
    let db_path = dir.join("symbols.db");

    fs::write(&helper_header, "int helper(int value);\n").unwrap();
    fs::write(
        &helper_source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "#include <stdio.h>\n#include \"helper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();

    let missing_system_header = dir.join("stdio.h");
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &missing_system_header).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 0);
    assert_eq!(stats.reused_files, 3);
}

#[test]
fn refreshes_index_when_symbol_becomes_resolvable() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def assist(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();

    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(initial_trace.callees.is_empty());

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        updated_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn refreshes_index_when_symbol_becomes_unresolvable() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        initial_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );

    fs::write(
        &helper,
        "def assist(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(updated_trace.callees.is_empty());
}

#[test]
fn refreshes_index_when_symbol_file_is_deleted() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        initial_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );

    fs::remove_file(&helper).unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.indexed_files, 1);
    assert_eq!(stats.rebuilt_files, 1);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(updated_trace.callees.is_empty());
    assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_err());
}

#[test]
fn rejects_refresh_path_that_escapes_workspace_after_normalization() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let nested = workspace.join("child");
    let helper = workspace.join("helper.py");
    let db_path = workspace.join("symbols.db");
    let outside = dir.join("outside.py");

    fs::create_dir_all(&nested).unwrap();
    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &outside,
        "def outside(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();

    rebuild_symbol_index(&workspace, &db_path).unwrap();

    let escaping_path = nested.join("..").join("..").join("outside.py");
    let error = refresh_symbol_index_for_file(&workspace, &db_path, &escaping_path)
        .expect_err("refresh should reject paths outside the workspace");
    assert!(error.to_string().contains("outside workspace"));
}

#[test]
fn rejects_refresh_path_outside_workspace_before_missing_index_rebuild() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let outside = dir.join("outside.py");
    let missing_db_path = workspace.join("missing-symbols.db");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        workspace.join("helper.py"),
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &outside,
        "def outside(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();

    let error = refresh_symbol_index_for_file(&workspace, &missing_db_path, &outside)
        .expect_err("refresh should reject outside files before rebuilding a missing index");
    assert!(error.to_string().contains("outside workspace"));
    assert!(!missing_db_path.exists());
}

#[test]
fn rejects_refresh_with_symbol_index_from_different_workspace() {
    let dir = temporary_dir();
    let workspace_a = dir.join("workspace-a");
    let workspace_b = dir.join("workspace-b");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&workspace_a).unwrap();
    fs::create_dir_all(&workspace_b).unwrap();
    fs::write(
        workspace_a.join("helper.py"),
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        workspace_b.join("helper.py"),
        "def helper(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();

    rebuild_symbol_index(&workspace_a, &db_path).unwrap();

    let error =
        refresh_symbol_index_for_file(&workspace_b, &db_path, &workspace_b.join("helper.py"))
            .expect_err("refresh should reject a database built for another workspace");

    assert!(error.to_string().contains("belongs to workspace"));
}

#[test]
fn rejects_rebuild_with_symbol_index_from_different_workspace() {
    let dir = temporary_dir();
    let workspace_a = dir.join("workspace-a");
    let workspace_b = dir.join("workspace-b");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&workspace_a).unwrap();
    fs::create_dir_all(&workspace_b).unwrap();
    fs::write(
        workspace_a.join("helper.py"),
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        workspace_b.join("helper.py"),
        "def helper(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();

    rebuild_symbol_index(&workspace_a, &db_path).unwrap();

    let error = rebuild_symbol_index(&workspace_b, &db_path)
        .expect_err("rebuild should reject a database built for another workspace");

    assert!(error.to_string().contains("belongs to workspace"));
}

#[test]
fn refresh_rejects_different_workspace_before_legacy_migration() {
    let dir = temporary_dir();
    let workspace_a = dir.join("workspace-a");
    let workspace_b = dir.join("workspace-b");
    let db_path = dir.join("symbols.db");
    let helper = workspace_b.join("helper.py");

    fs::create_dir_all(&workspace_a).unwrap();
    fs::create_dir_all(&workspace_b).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    let connection = Connection::open(&db_path).unwrap();
    create_legacy_symbol_index_schema_without_reference_names(
        &connection,
        Some(&normalize_path(&workspace_a)),
        Some("0"),
    );
    drop(connection);

    let error = refresh_symbol_index_for_file(&workspace_b, &db_path, &helper)
        .expect_err("wrong-workspace refresh should reject before legacy migration");

    assert!(error.to_string().contains("belongs to workspace"));
    let connection = Connection::open(&db_path).unwrap();
    assert!(!symbol_table_columns(&connection).contains(&"reference_names_json".to_string()));
}

#[test]
fn refresh_rejects_empty_persisted_symbol_identity_without_rewrite() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute(
            "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)",
            [normalize_path(&dir)],
        )
        .unwrap();
    connection
        .execute_batch(
            "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    '', 'helper', 'helper.py', 'function_definition', 0, 5,
                    '[]', '[]', '[]', '[]'
                );
                ",
        )
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject persisted rows with empty identity fields");

    assert!(error.to_string().contains("empty symbol_id"));
    let connection = Connection::open(&db_path).unwrap();
    let persisted_symbol_id: String = connection
        .query_row("SELECT symbol_id FROM symbols", [], |row| row.get(0))
        .unwrap();
    assert_eq!(persisted_symbol_id, "");
}

#[test]
fn refresh_rejects_empty_persisted_reference_names_without_rewrite() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute(
            "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)",
            [normalize_path(&dir)],
        )
        .unwrap();
    connection
        .execute_batch(
            "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    'helper', 'helper', 'helper.py', 'function_definition', 0, 5,
                    '[]', '[]', '[]', '[\"\"]'
                );
                ",
        )
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject empty persisted reference names");

    assert!(
        error
            .to_string()
            .contains("empty reference_names_json entry")
    );
    let connection = Connection::open(&db_path).unwrap();
    let reference_names_json: String = connection
        .query_row("SELECT reference_names_json FROM symbols", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(reference_names_json, "[\"\"]");
}

#[test]
fn refresh_rejects_empty_persisted_file_state_path_without_rewrite() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute(
            "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)",
            [normalize_path(&dir)],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO file_state(file_path, fingerprint) VALUES('', 1)",
            [],
        )
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject empty persisted file_state paths");

    assert!(error.to_string().contains("empty file_state.file_path"));
    let connection = Connection::open(&db_path).unwrap();
    let persisted_file_path: String = connection
        .query_row("SELECT file_path FROM file_state", [], |row| row.get(0))
        .unwrap();
    assert_eq!(persisted_file_path, "");
}

#[test]
fn refresh_rejects_persisted_symbol_paths_outside_workspace_without_rewrite() {
    let root = temporary_dir();
    let dir = root.join("workspace");
    let helper = dir.join("helper.py");
    let outside = root.join("outside.py");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&dir).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&outside, "def outside() -> int:\n    return 2\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let outside_path = normalize_path(&outside);
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE symbols SET file_path = ?1 WHERE semantic_path = 'helper'",
            [&outside_path],
        )
        .unwrap();
    drop(connection);

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh must reject persisted paths outside the workspace");
    assert!(error.to_string().contains("symbols.file_path"));
    assert!(error.to_string().contains("outside indexed workspace"));

    let connection = Connection::open(&db_path).unwrap();
    let persisted_path: String = connection
        .query_row("SELECT file_path FROM symbols", [], |row| row.get(0))
        .unwrap();
    assert_eq!(persisted_path, outside_path);
}
