use super::*;

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
