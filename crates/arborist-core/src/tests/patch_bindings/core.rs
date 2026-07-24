use super::*;

#[test]
fn ignores_python_type_annotations_during_patch_binding_validation() {
    let source = r#"
def top_level(value: int) -> int:
    return value
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: MissingType) -> MissingReturn:\n    return value\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "value" && decision.status == "resolved")
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .all(|decision| decision.name != "MissingType" && decision.name != "MissingReturn")
    );
}

#[test]
fn validates_python_default_parameter_references() {
    let source = r#"
def top_level(value: int) -> int:
    return value
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: int = missing_default) -> int:\n    return value\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert_eq!(
        result.validation.unresolved_identifiers,
        vec!["missing_default"]
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "missing_default" && decision.status == "unresolved")
    );
}

#[test]
fn validates_python_default_parameter_scope() {
    let source = r#"
MODULE_DEFAULT = 1

def top_level(value: int) -> int:
    return value
"#;

    let allowed = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: int = MODULE_DEFAULT) -> int:\n    return value\n",
        None,
    )
    .unwrap();

    assert!(allowed.applied);
    assert!(
        allowed
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "MODULE_DEFAULT" && decision.status == "resolved")
    );

    let rejected = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: int, other=value) -> int:\n    return other\n",
        None,
    )
    .unwrap();

    assert!(!rejected.applied);
    assert_eq!(rejected.validation.unresolved_identifiers, vec!["value"]);
    assert!(
        rejected
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "value" && decision.status == "unresolved")
    );
    assert!(
        rejected
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "other" && decision.status == "resolved")
    );
}

#[test]
fn resolves_python_patch_bindings_with_semantic_metadata() {
    let source = r#"
def helper(value: int) -> int:
    """Shared helper."""
    return value + 1

def top_level(value: int) -> int:
    local_bonus = 2
    return helper(value) + local_bonus
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value: int) -> int:\n    local_bonus = 3\n    return helper(value) + local_bonus\n",
            None,
        )
        .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    assert_eq!(result.validation.ambiguous_identifiers.len(), 0);
    assert_eq!(result.validation.resolved_identifiers.len(), 3);

    let helper_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "helper")
        .unwrap();
    assert_eq!(helper_binding.symbol.semantic_path, "helper");
    assert_eq!(helper_binding.symbol.scope_path, None);
    assert_eq!(
        helper_binding.symbol.signature.as_deref(),
        Some("def helper(value: int) -> int:")
    );
    assert_eq!(
        helper_binding.symbol.parameters,
        vec!["value: int".to_string()]
    );
    assert_eq!(helper_binding.symbol.return_type.as_deref(), Some("int"));
    assert_eq!(
        helper_binding.symbol.docstring.as_deref(),
        Some("\"\"\"Shared helper.\"\"\"")
    );

    let local_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "local_bonus")
        .unwrap();
    assert_eq!(local_binding.symbol.semantic_path, "local_bonus");
    assert_eq!(
        local_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
    assert_eq!(local_binding.symbol.node_kind, "assignment");
    assert_eq!(local_binding.symbol.origin_type, "local_scope");
    assert!(local_binding.symbol.signature.is_none());
    assert!(local_binding.symbol.parameters.is_empty());
    assert!(local_binding.symbol.return_type.is_none());
    assert!(local_binding.symbol.docstring.is_none());
}
