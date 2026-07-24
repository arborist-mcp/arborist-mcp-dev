use super::*;

#[test]
fn resolves_python_comprehension_target_patch_bindings() {
    let source = r#"
def item() -> int:
    return 1

def top_level(values: list[int]) -> object:
    return values
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(values: list[int]) -> object:\n    return [item for item in values]\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    let item_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "item")
        .unwrap();
    assert_eq!(item_binding.symbol.node_kind, "comprehension_target");
    assert_eq!(item_binding.symbol.scope_path.as_deref(), Some("top_level"));
}

#[test]
fn resolves_python_comprehension_target_body_bindings_without_global_shadowing() {
    let source = r#"
def top_level(values: list[int]) -> object:
    return values
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(values: list[int]) -> object:\n    return [item for item in values]\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    let item_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "item")
        .unwrap();
    assert_eq!(item_binding.symbol.node_kind, "comprehension_target");
    assert_eq!(item_binding.symbol.scope_path.as_deref(), Some("top_level"));
}

#[test]
fn resolves_python_comprehension_target_filter_bindings() {
    let source = r#"
def top_level(values: list[int]) -> object:
    return values
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(values: list[int]) -> object:\n    return [item for item in values if item]\n",
            None,
        )
        .unwrap();

    assert!(result.applied);
    let item_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "item")
        .unwrap();
    assert_eq!(item_binding.symbol.node_kind, "comprehension_target");
    assert_eq!(item_binding.symbol.scope_path.as_deref(), Some("top_level"));
}

#[test]
fn resolves_python_named_expression_bindings() {
    let source = r#"
def top_level(items: list[int]) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(items: list[int]) -> int:\n    if (count := len(items)):\n        return count\n    return 0\n",
            None,
        )
        .unwrap();

    assert!(result.applied);
    let count_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "count")
        .unwrap();
    assert_eq!(count_binding.symbol.node_kind, "named_expression");
    assert_eq!(
        count_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "items" && decision.status == "resolved")
    );
}

#[test]
fn rejects_python_pre_named_expression_references() {
    let source = r#"
def target() -> int:
    return 1

def top_level(flag: bool) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(flag: bool) -> int:\n    before = target\n    if flag and (target := 3):\n        return before\n    return before\n",
            None,
        )
        .unwrap();

    assert!(!result.applied);
    assert_eq!(result.validation.unresolved_identifiers, vec!["target"]);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "flag" && decision.status == "resolved")
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "target" && decision.status == "unresolved")
    );
}

#[test]
fn rejects_python_pre_named_expression_references_inside_comprehensions() {
    let source = r#"
def target() -> int:
    return 1

def top_level(values: list[int]) -> object:
    return values
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(values: list[int]) -> object:\n    return [target + (target := item) for item in values]\n",
            None,
        )
        .unwrap();

    assert!(!result.applied);
    assert_eq!(result.validation.unresolved_identifiers, vec!["target"]);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "item" && decision.status == "resolved")
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "target" && decision.status == "unresolved")
    );
}

#[test]
fn resolves_python_named_expression_references_after_binding_inside_comprehensions() {
    let source = r#"
def top_level(values: list[int]) -> object:
    return values
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(values: list[int]) -> object:\n    return [(target := item) + target for item in values]\n",
            None,
        )
        .unwrap();

    assert!(result.applied);
    let target_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "target")
        .unwrap();
    assert_eq!(target_binding.symbol.node_kind, "named_expression");
    let item_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "item")
        .unwrap();
    assert_eq!(item_binding.symbol.node_kind, "comprehension_target");
}

#[test]
fn resolves_python_lambda_parameter_bindings() {
    let source = r#"
def target() -> int:
    return 1

def top_level() -> int:
    return 0
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level() -> int:\n    return (lambda target: target)(3)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    let target_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "target")
        .unwrap();
    assert_eq!(target_binding.symbol.node_kind, "parameter");
    assert_eq!(
        target_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn resolves_python_lambda_default_parameter_references() {
    let source = r#"
def target() -> int:
    return 1

def top_level() -> int:
    return 0
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level() -> int:\n    return (lambda x=target(): x)()\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    let target_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "target")
        .unwrap();
    assert_eq!(target_binding.symbol.node_kind, "function_definition");
    let x_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "x")
        .unwrap();
    assert_eq!(x_binding.symbol.node_kind, "parameter");
}

#[test]
fn resolves_python_async_function_patch_bindings() {
    let source = r#"
def helper(value: int) -> int:
    return value + 1

async def top_level(value: int) -> int:
    return value
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "async def top_level(value: int) -> int:\n    return helper(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "top_level");
    let helper_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "helper")
        .unwrap();
    assert_eq!(helper_binding.symbol.node_kind, "function_definition");
    assert_eq!(helper_binding.symbol.semantic_path, "helper");
}
