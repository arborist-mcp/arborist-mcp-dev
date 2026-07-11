use super::*;

#[test]
fn replays_patch_evidence_against_matching_trace() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let patch = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();
    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    let replay = replay_patch_evidence_against_trace(&patch, &trace).unwrap();

    assert!(replay.consistent);
    assert_eq!(replay.matched_items, 1);
    assert_eq!(replay.blocked_items, 0);
    assert_eq!(replay.items.len(), 1);
    assert_eq!(replay.items[0].status, "matched");
    assert!(replay.items[0].matched_in_trace);
    assert_eq!(replay.items[0].trace_match_scope, "callees");
}

#[test]
fn traces_python_symbol_metadata_through_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    \"\"\"Shared helper.\"\"\"\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    \"\"\"Coordinate the helper call.\"\"\"\n    return helper(value)\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(
        live_trace.symbol.docstring.as_deref(),
        Some("\"\"\"Coordinate the helper call.\"\"\"")
    );
    assert_eq!(live_trace.symbol.parameters, vec!["value: int".to_string()]);
    assert_eq!(live_trace.symbol.return_type.as_deref(), Some("int"));
    assert_eq!(live_trace.callees.len(), 1);
    assert_eq!(
        live_trace.callees[0].docstring.as_deref(),
        Some("\"\"\"Shared helper.\"\"\"")
    );
    assert_eq!(
        live_trace.callees[0].parameters,
        vec!["value: int".to_string()]
    );
    assert_eq!(live_trace.callees[0].return_type.as_deref(), Some("int"));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace.symbol.docstring.as_deref(),
        Some("\"\"\"Coordinate the helper call.\"\"\"")
    );
    assert_eq!(
        persisted_trace.symbol.parameters,
        vec!["value: int".to_string()]
    );
    assert_eq!(persisted_trace.symbol.return_type.as_deref(), Some("int"));
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(
        persisted_trace.callees[0].docstring.as_deref(),
        Some("\"\"\"Shared helper.\"\"\"")
    );
    assert_eq!(
        persisted_trace.callees[0].parameters,
        vec!["value: int".to_string()]
    );
    assert_eq!(
        persisted_trace.callees[0].return_type.as_deref(),
        Some("int")
    );
}

#[test]
fn traces_duplicate_c_globals_by_precise_symbol_id() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let alpha_caller = dir.join("alpha_caller.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let zeta_caller = dir.join("zeta_caller.c");
    let db_path = dir.join("symbols.db");

    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &alpha_caller,
        "#include \"alpha.h\"\n\nint call_alpha(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(
        &zeta_caller,
        "#include \"zeta.h\"\n\nint call_zeta(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    let alpha_symbol_id = format!(
        "{}::helper",
        alpha_header.to_string_lossy().replace('\\', "/")
    );
    let zeta_symbol_id = format!(
        "{}::helper",
        zeta_header.to_string_lossy().replace('\\', "/")
    );

    let alpha_trace = trace_symbol_graph(&dir, &alpha_symbol_id, TraceDirection::Both).unwrap();
    assert_eq!(alpha_trace.symbol.symbol_id, alpha_symbol_id);
    assert_eq!(
        alpha_trace.symbol.file_path,
        alpha_source.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(alpha_trace.callers.len(), 1);
    assert_eq!(alpha_trace.callers[0].semantic_path, "call_alpha");
    assert_eq!(
        alpha_trace.callers[0].file_path,
        alpha_caller.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_zeta_trace =
        trace_symbol_graph_from_index(&db_path, &zeta_symbol_id, TraceDirection::Both).unwrap();
    assert_eq!(persisted_zeta_trace.symbol.symbol_id, zeta_symbol_id);
    assert_eq!(
        persisted_zeta_trace.symbol.file_path,
        zeta_source.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(persisted_zeta_trace.callers.len(), 1);
    assert_eq!(persisted_zeta_trace.callers[0].semantic_path, "call_zeta");
    assert_eq!(
        persisted_zeta_trace.callers[0].file_path,
        zeta_caller.to_string_lossy().replace('\\', "/")
    );
}
