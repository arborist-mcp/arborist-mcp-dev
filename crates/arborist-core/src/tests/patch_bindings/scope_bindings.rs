use super::*;

#[test]
fn resolves_python_with_statement_bindings() {
    let source = r#"
def manager():
    return object()

def top_level() -> object:
    return manager()
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level() -> object:\n    with manager() as handle:\n        return handle\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "manager" && decision.status == "resolved")
    );
    let handle_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "handle")
        .unwrap();
    assert_eq!(handle_binding.symbol.node_kind, "with_target");
    assert_eq!(
        handle_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn resolves_python_except_clause_bindings() {
    let source = r#"
def risky():
    raise ValueError()

def top_level() -> object:
    return risky()
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> object:\n    try:\n        risky()\n    except ValueError as exc:\n        return exc\n",
            None,
        )
        .unwrap();

    assert!(result.applied);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "risky" && decision.status == "resolved")
    );
    let exc_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "exc")
        .unwrap();
    assert_eq!(exc_binding.symbol.node_kind, "except_target");
    assert_eq!(exc_binding.symbol.scope_path.as_deref(), Some("top_level"));
}

#[test]
fn rejects_python_post_except_target_references() {
    let source = r#"
def exc() -> int:
    return 1

def risky() -> int:
    raise ValueError()

def top_level() -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    try:\n        risky()\n    except ValueError as exc:\n        return 0\n    return exc()\n",
            None,
        )
        .unwrap();

    assert!(!result.applied);
    assert_eq!(result.validation.unresolved_identifiers, vec!["exc"]);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "risky" && decision.status == "resolved")
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "exc" && decision.status == "unresolved")
    );
}

#[test]
fn rejects_python_pre_except_target_references() {
    let source = r#"
def exc() -> int:
    return 1

def risky() -> int:
    return 2

def top_level() -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    before = exc\n    try:\n        risky()\n    except ValueError as exc:\n        return before\n    return 0\n",
            None,
        )
        .unwrap();

    assert!(!result.applied);
    assert_eq!(result.validation.unresolved_identifiers, vec!["exc"]);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "risky" && decision.status == "resolved")
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "exc" && decision.status == "unresolved")
    );
}

#[test]
fn rejects_python_mixed_except_target_reference_states() {
    let source = r#"
def exc() -> int:
    return 1

def risky() -> int:
    return 2

def handle(value: object) -> object:
    return value

def top_level() -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    try:\n        risky()\n    except ValueError as exc:\n        handle(exc)\n    return exc()\n",
            None,
        )
        .unwrap();

    assert!(!result.applied);
    assert_eq!(result.validation.unresolved_identifiers, vec!["exc"]);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "risky" && decision.status == "resolved")
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "handle" && decision.status == "resolved")
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "exc" && decision.status == "unresolved")
    );
}

#[test]
fn resolves_python_block_local_bindings() {
    let source = r#"
def top_level(flag: bool) -> int:
    return 1
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(flag: bool) -> int:\n    if flag:\n        branch_value = 2\n    return branch_value\n",
            None,
        )
        .unwrap();

    assert!(result.applied);
    let branch_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "branch_value")
        .unwrap();
    assert_eq!(branch_binding.symbol.node_kind, "assignment");
    assert_eq!(
        branch_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn resolves_python_global_declared_patch_bindings() {
    let source = r#"
def helper() -> int:
    return 1

def top_level() -> int:
    return 0
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level() -> int:\n    global helper\n    helper = helper\n    return helper()\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    let helper_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "helper")
        .unwrap();
    assert_eq!(helper_binding.symbol.semantic_path, "helper");
    assert_eq!(helper_binding.symbol.node_kind, "function_definition");
}

#[test]
fn resolves_python_global_references_inside_nested_functions_despite_outer_shadowing() {
    let source = r#"
def helper() -> int:
    return 1

def top_level() -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    helper = 2\n\n    def inner() -> int:\n        global helper\n        return helper()\n\n    return inner()\n",
            None,
        )
        .unwrap();

    assert!(result.applied);
    let helper_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "helper")
        .unwrap();
    assert_eq!(helper_binding.symbol.semantic_path, "helper");
    assert_eq!(helper_binding.symbol.node_kind, "function_definition");
}
