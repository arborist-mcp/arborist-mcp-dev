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
