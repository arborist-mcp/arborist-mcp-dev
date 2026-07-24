use super::*;

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
