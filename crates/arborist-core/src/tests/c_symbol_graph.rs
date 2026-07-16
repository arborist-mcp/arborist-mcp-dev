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
