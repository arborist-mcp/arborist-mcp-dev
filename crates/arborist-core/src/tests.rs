use std::fs;
use std::path::Path;

use super::{
    DEFAULT_TREE_QUERY_MAX_BYTES, Position, TraceDirection, VirtualFileSystem, execute_tree_query,
    execute_tree_query_from_path, execute_tree_query_with_limit, get_semantic_skeleton,
    get_semantic_skeleton_from_path, list_symbols, list_symbols_context,
    list_symbols_context_from_index, list_symbols_discovery_context,
    list_symbols_discovery_context_from_index, list_symbols_filtered, list_symbols_from_index,
    list_symbols_from_index_filtered, list_symbols_neighborhood_context,
    list_symbols_neighborhood_context_from_index, patch_ast_node, patch_ast_node_at_position,
    patch_ast_node_from_path, preview_patch_ast_node_from_path, read_symbol,
    read_symbol_at_position, read_symbol_at_position_from_index, read_symbol_context,
    read_symbol_context_from_index, read_symbol_discovery_context,
    read_symbol_discovery_context_at_position,
    read_symbol_discovery_context_at_position_from_index,
    read_symbol_discovery_context_at_position_with_source,
    read_symbol_discovery_context_from_index, read_symbol_from_index,
    read_symbol_neighborhood_context, read_symbol_neighborhood_context_from_index,
    rebuild_symbol_index, replay_patch_evidence_against_trace, search_symbols,
    search_symbols_context, search_symbols_context_from_index, search_symbols_discovery_context,
    search_symbols_discovery_context_from_index, search_symbols_filtered,
    search_symbols_from_index, search_symbols_from_index_filtered,
    search_symbols_neighborhood_context, search_symbols_neighborhood_context_from_index,
    trace_symbol_graph, trace_symbol_graph_at_position, trace_symbol_graph_at_position_from_index,
    trace_symbol_graph_at_position_with_source, trace_symbol_graph_from_index,
    trace_symbol_neighborhood, trace_symbol_neighborhood_at_position,
    trace_symbol_neighborhood_at_position_from_index, trace_symbol_neighborhood_from_index,
    validate_patch_commit_with_trace, validate_patch_trace_validation_result,
    validate_patch_with_discovery_context, validate_patch_with_discovery_context_at_position,
    validate_patch_with_discovery_context_from_path, validate_patch_with_graph_context,
    validate_patch_with_graph_context_from_path, validate_patch_with_neighborhood_context,
    validate_patch_with_neighborhood_context_from_path, validate_patch_with_trace_context,
    validate_patch_with_trace_context_at_position, validate_patch_with_trace_context_from_path,
    validate_trace_backed_patch_result, validate_trace_patch_evidence_replay_result,
};
mod index_refresh;
mod patch_bindings;
mod patch_replay;
mod path_entrypoints;
mod persisted_index;
mod query_parity;
mod skeleton;
mod source_overlay;
mod support;
mod trace_semantics;
mod tree_query;

use support::temporary_dir;

#[test]
fn rejects_empty_file_paths() {
    let source = "def top_level(value: int) -> int:\n    return value\n";

    let error = get_semantic_skeleton(Path::new(""), source, 1, &[])
        .expect_err("empty file paths should be rejected");

    assert!(error.to_string().contains("path"));
    assert!(error.to_string().contains("empty"));
}

#[test]
fn rejects_blank_patch_targets() {
    let source = "def top_level():\n    return 1\n";

    let error = patch_ast_node(
        Path::new("sample.py"),
        source,
        " \t",
        "def top_level():\n    return 2\n",
        None,
    )
    .expect_err("blank patch targets should be rejected");

    assert!(error.to_string().contains("semantic target"));
    assert!(error.to_string().contains("blank"));
}

#[test]
fn rejects_blank_patch_replacements() {
    let source = "def top_level():\n    return 1\n";

    let error = patch_ast_node(Path::new("sample.py"), source, "top_level", " \t", None)
        .expect_err("blank patch replacements should be rejected");

    assert!(error.to_string().contains("new_code"));
    assert!(error.to_string().contains("blank"));
}

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

#[test]
fn allows_c_patch_when_symbol_is_declared_in_included_header() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(result.validation.commit_gate.allowed);
    assert_eq!(result.validation.commit_gate.status, "allowed");
    assert_eq!(
        result.validation.commit_gate.reason,
        "syntax and symbol binding validation passed"
    );
    assert_eq!(result.validation.commit_gate.syntax_error_count, 0);
    assert!(result.validation.commit_gate.blocking_decisions.is_empty());
    assert_eq!(result.validation.commit_gate.evidence_invariants.len(), 1);
    assert_eq!(
        result.validation.commit_gate.evidence_invariants[0].status,
        "passed"
    );
    assert_eq!(result.validation.ambiguous_identifiers.len(), 0);
    assert_eq!(result.validation.resolved_identifiers.len(), 1);
    assert_eq!(result.validation.binding_decisions.len(), 1);
    assert_eq!(result.validation.binding_decisions[0].name, "helper");
    assert_eq!(result.validation.binding_decisions[0].status, "resolved");
    assert_eq!(result.validation.resolved_identifiers[0].name, "helper");
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );
    assert_eq!(
        result.validation.binding_decisions[0]
            .selected_symbol_id
            .as_deref(),
        Some(
            result.validation.resolved_identifiers[0]
                .symbol
                .symbol_id
                .as_str()
        )
    );
    assert_eq!(result.validation.binding_decisions[0].candidates.len(), 1);
    assert_eq!(
        result.validation.commit_gate.evidence_invariants[0]
            .selected_evidence_key
            .as_deref(),
        Some(
            result.validation.binding_decisions[0].candidates[0]
                .evidence_key
                .as_str()
        )
    );
    let header_text = fs::read_to_string(&header).unwrap();
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.node_kind,
        "declaration"
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.origin_type,
        "include_header"
    );
    assert!(
        result.validation.resolved_identifiers[0]
            .symbol
            .evidence_key
            .contains("declaration|include_header")
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.byte_range,
        (0, header_text.find(';').map(|index| index + 1).unwrap())
    );
    assert_eq!(
        result.validation.resolved_identifiers[0]
            .symbol
            .signature
            .as_deref(),
        Some("int helper(int value);")
    );
    let updated = fs::read_to_string(&caller).unwrap();
    assert!(updated.contains("return helper(value);"));
}

#[test]
fn allows_c_patch_with_uppercase_header_companion_source() {
    let dir = temporary_dir();
    let header = dir.join("helper.H");
    let source = dir.join("helper.C");
    let caller = dir.join("caller.C");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.H\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.H\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert_eq!(result.validation.resolved_identifiers.len(), 1);
    assert_eq!(result.validation.binding_decisions.len(), 1);
    assert_eq!(result.validation.resolved_identifiers[0].name, "helper");
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.node_kind,
        "function_definition"
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.origin_type,
        "companion_source"
    );
    assert!(result.validation.commit_gate.allowed);

    let updated = fs::read_to_string(&caller).unwrap();
    assert!(updated.contains("return helper(value);"));
}

#[test]
fn allows_c_patch_with_hpp_header_companion_source() {
    let dir = temporary_dir();
    let header = dir.join("helper.HPP");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.HPP\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.HPP\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert_eq!(result.validation.resolved_identifiers.len(), 1);
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.origin_type,
        "companion_source"
    );
    assert!(result.validation.commit_gate.allowed);
}

#[test]
fn patches_c_definition_when_declaration_and_definition_share_name() {
    let dir = temporary_dir();
    let file = dir.join("helper.c");

    fs::write(
        &file,
        "int helper(int value);\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "helper",
        "int helper(int value) {\n    return value + 9;\n}\n",
        None,
    )
    .unwrap();

    let updated = fs::read_to_string(&file).unwrap();
    assert!(result.applied);
    assert_eq!(result.resolved_path, "helper");
    assert_eq!(
        result.resolved_symbol_id,
        format!("{}::helper", file.to_string_lossy().replace('\\', "/"))
    );
    assert!(updated.starts_with("int helper(int value);\n\n"));
    assert!(updated.contains("int helper(int value) {\n    return value + 9;\n}"));
    assert!(updated.contains("return value + 9;"));
    assert_eq!(updated.matches("int helper(int value);").count(), 1);
}

#[test]
fn allows_c_patch_targeting_precise_symbol_id() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let symbol_id = format!("{}::helper", header.to_string_lossy().replace('\\', "/"));
    let result = patch_ast_node_from_path(
        &source,
        &symbol_id,
        "int helper(int value) {\n    return value + 5;\n}\n",
        None,
    )
    .unwrap();

    let updated = fs::read_to_string(&source).unwrap();
    assert!(result.applied);
    assert_eq!(result.target_path, symbol_id);
    assert_eq!(result.resolved_path, "helper");
    assert_eq!(result.resolved_symbol_id, result.target_path);
    assert!(updated.contains("return value + 5;"));
}

#[test]
fn reports_ambiguous_c_identifier_bindings() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let caller = dir.join("caller.c");

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
            "#include \"alpha.h\"\n#include \"zeta.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(result.validation.resolved_identifiers.is_empty());
    assert!(!result.validation.commit_gate.allowed);
    assert_eq!(result.validation.commit_gate.status, "rejected");
    assert_eq!(
        result.validation.commit_gate.reason,
        "symbol binding is ambiguous"
    );
    assert_eq!(result.validation.commit_gate.syntax_error_count, 0);
    assert_eq!(result.validation.commit_gate.blocking_decisions.len(), 1);
    assert_eq!(
        result.validation.commit_gate.blocking_decisions[0].status,
        "ambiguous"
    );
    assert_eq!(result.validation.commit_gate.evidence_invariants.len(), 1);
    assert_eq!(
        result.validation.commit_gate.evidence_invariants[0].status,
        "blocked"
    );
    assert_eq!(
        result.validation.commit_gate.evidence_invariants[0]
            .candidate_evidence_keys
            .len(),
        2
    );
    assert_eq!(result.validation.ambiguous_identifiers.len(), 1);
    assert_eq!(result.validation.ambiguous_identifiers[0].name, "helper");
    assert_eq!(result.validation.binding_decisions.len(), 1);
    assert_eq!(result.validation.binding_decisions[0].name, "helper");
    assert_eq!(result.validation.binding_decisions[0].status, "ambiguous");
    assert_eq!(
        result.validation.binding_decisions[0].selected_symbol_id,
        None
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates.len(),
        2
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].reason,
        "multiple equally-ranked definitions across include families"
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .active_include_family,
        None
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .preferred_family,
        None
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .visible_include_families,
        vec![
            alpha_header.to_string_lossy().replace('\\', "/"),
            zeta_header.to_string_lossy().replace('\\', "/")
        ]
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .candidate_include_families,
        vec![
            alpha_header.to_string_lossy().replace('\\', "/"),
            zeta_header.to_string_lossy().replace('\\', "/")
        ]
    );
    assert_eq!(
        result.validation.binding_decisions[0].reason,
        result.validation.ambiguous_identifiers[0].reason
    );
    assert_eq!(result.validation.binding_decisions[0].candidates.len(), 2);
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0].symbol_id,
        format!(
            "{}::helper",
            alpha_header.to_string_lossy().replace('\\', "/")
        )
    );
    let alpha_source_text = fs::read_to_string(&alpha_source).unwrap();
    let alpha_start = alpha_source_text.find("int helper(int value) {").unwrap();
    let alpha_end = alpha_source_text.find('}').map(|index| index + 1).unwrap();
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0].node_kind,
        "function_definition"
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0].origin_type,
        "companion_source"
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0].evidence_key,
        result.validation.binding_decisions[0].candidates[0].evidence_key
    );
    assert!(
        result.validation.ambiguous_identifiers[0].candidates[0]
            .evidence_key
            .contains("function_definition|companion_source")
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0].byte_range,
        (alpha_start, alpha_end)
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0]
            .signature
            .as_deref(),
        Some("int helper(int value);")
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[1].symbol_id,
        format!(
            "{}::helper",
            zeta_header.to_string_lossy().replace('\\', "/")
        )
    );
    let zeta_source_text = fs::read_to_string(&zeta_source).unwrap();
    let zeta_start = zeta_source_text.find("int helper(int value) {").unwrap();
    let zeta_end = zeta_source_text.find('}').map(|index| index + 1).unwrap();
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[1].node_kind,
        "function_definition"
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[1].origin_type,
        "companion_source"
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[1].byte_range,
        (zeta_start, zeta_end)
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[1]
            .signature
            .as_deref(),
        Some("int helper(int value);")
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .candidate_symbol_ids,
        vec![
            format!(
                "{}::helper",
                alpha_header.to_string_lossy().replace('\\', "/")
            ),
            format!(
                "{}::helper",
                zeta_header.to_string_lossy().replace('\\', "/")
            )
        ]
    );
}

#[test]
fn allows_ambiguous_c_identifier_bindings_with_bypass() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let caller = dir.join("caller.c");

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
            "#include \"alpha.h\"\n#include \"zeta.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        Some("runtime wiring guarantees the intended helper target"),
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.bypass_applied);
    assert!(result.validation.commit_gate.allowed);
    assert_eq!(result.validation.commit_gate.status, "allowed_with_bypass");
    assert_eq!(
        result.validation.commit_gate.bypass_reason.as_deref(),
        Some("runtime wiring guarantees the intended helper target")
    );
    assert_eq!(result.validation.commit_gate.blocking_decisions.len(), 1);
    assert_eq!(
        result.validation.commit_gate.evidence_invariants[0].status,
        "blocked"
    );
    assert_eq!(result.validation.ambiguous_identifiers.len(), 1);
    let updated = fs::read_to_string(&caller).unwrap();
    assert!(updated.contains("return helper(value);"));
}

#[test]
fn reports_transitive_visible_include_families_for_c_ambiguity() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let wrapper_header = dir.join("wrapper.h");
    let caller = dir.join("caller.c");

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
        &wrapper_header,
        "#include \"alpha.h\"\n#include \"zeta.h\"\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"wrapper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();

    assert_eq!(result.validation.ambiguous_identifiers.len(), 1);
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .visible_include_families,
        vec![
            wrapper_header.to_string_lossy().replace('\\', "/"),
            alpha_header.to_string_lossy().replace('\\', "/"),
            zeta_header.to_string_lossy().replace('\\', "/")
        ]
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .candidate_include_families,
        vec![
            alpha_header.to_string_lossy().replace('\\', "/"),
            zeta_header.to_string_lossy().replace('\\', "/")
        ]
    );
}

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
