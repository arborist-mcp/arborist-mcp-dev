use super::*;

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
fn patches_cpp_function_targeted_by_nested_namespace_path() {
    let dir = temporary_dir();
    let file = dir.join("api.cpp");

    fs::write(
        &file,
        "namespace alpha::detail {\nint helper(int value) { return value + 1; }\n\nint orchestrate(int value) { return helper(value); }\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "alpha::detail::orchestrate",
        "int orchestrate(int value) { return helper(value) + 2; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "alpha::detail::orchestrate");
    assert_eq!(result.resolved_symbol_id, "alpha::detail::orchestrate(int)");
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("return helper(value) + 2;")
    );
}

#[test]
fn patches_cpp_class_method_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("counter.cpp");
    fs::write(
        &file,
        "namespace api {\nclass Counter {\npublic:\n    int increment(int value) { return value + 1; }\n};\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "api::Counter::increment",
        "int increment(int value) { return value + 2; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::Counter::increment");
    assert_eq!(result.resolved_symbol_id, "api::Counter::increment(int)");
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("return value + 2;")
    );
}
