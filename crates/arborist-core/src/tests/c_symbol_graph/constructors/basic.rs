use super::*;

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
