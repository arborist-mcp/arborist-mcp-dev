use super::*;

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
