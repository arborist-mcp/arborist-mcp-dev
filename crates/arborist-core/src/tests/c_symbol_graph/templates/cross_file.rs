use super::*;

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
