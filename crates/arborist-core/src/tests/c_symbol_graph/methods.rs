use super::*;

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
