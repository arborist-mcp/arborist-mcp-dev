use super::*;

#[test]
fn replaces_python_decorated_class_without_retaining_old_decorators() {
    let source = r#"
def decorator(cls):
    return cls

@decorator
class Container:
    value = 1
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Container",
        "class Container:\n    value = 2\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(
        result
            .validation
            .syntax_errors
            .iter()
            .any(|issue| issue.kind == "decorator_guard")
    );
    assert!(result.updated_source.contains("class Container:"));
    assert_eq!(result.resolved_path, "Container");
}

#[test]
fn resolves_python_nested_default_parameter_closure_bindings() {
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
            "def top_level() -> int:\n    helper = 3\n\n    def inner(value=helper) -> int:\n        return value\n\n    return inner()\n",
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
    assert_eq!(helper_binding.symbol.node_kind, "assignment");
    assert_eq!(
        helper_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn rejects_python_class_lambda_references_to_class_locals() {
    let source = r#"
class Container:
    value = 0
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Container",
        "class Container:\n    helper = 2\n    value = (lambda: helper)()\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert_eq!(result.validation.unresolved_identifiers, vec!["helper"]);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "helper" && decision.status == "unresolved")
    );
}

#[test]
fn resolves_python_class_body_references_to_class_locals() {
    let source = r#"
class Container:
    value = 0
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Container",
        "class Container:\n    helper = 2\n    value = helper\n",
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
    assert_eq!(helper_binding.symbol.node_kind, "assignment");
    assert_eq!(
        helper_binding.symbol.scope_path.as_deref(),
        Some("Container")
    );
}

#[test]
fn resolves_python_class_method_default_parameter_references_to_class_locals() {
    let source = r#"
def helper() -> int:
    return 1

class Container:
    helper = 2

    def method(value=None) -> object:
        return 0
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Container.method",
        "def method(value=helper) -> object:\n    return 0\n",
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
    assert_eq!(helper_binding.symbol.node_kind, "assignment");
    assert_eq!(
        helper_binding.symbol.scope_path.as_deref(),
        Some("Container")
    );
}

#[test]
fn resolves_python_nested_decorator_closure_bindings() {
    let source = r#"
def helper(func):
    return func

def top_level() -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    helper = lambda func: func\n\n    @helper\n    def inner() -> int:\n        return 1\n\n    return 0\n",
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
    assert_eq!(helper_binding.symbol.node_kind, "assignment");
    assert_eq!(
        helper_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn resolves_python_class_method_decorator_references_to_class_locals() {
    let source = r#"
def helper(func):
    return func

class Container:
    def method(self) -> int:
        return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "Container",
            "class Container:\n    helper = helper\n\n    @helper\n    def method(self) -> int:\n        return 0\n",
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
    assert_eq!(helper_binding.symbol.node_kind, "assignment");
    assert_eq!(
        helper_binding.symbol.scope_path.as_deref(),
        Some("Container")
    );
}

#[test]
fn resolves_python_nested_class_base_closure_bindings() {
    let source = r#"
class GlobalBase:
    pass

def top_level() -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    Base = GlobalBase\n\n    class Inner(Base):\n        pass\n\n    return 0\n",
            None,
        )
        .unwrap();

    assert!(result.applied);
    let base_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "Base")
        .unwrap();
    assert_eq!(base_binding.symbol.node_kind, "assignment");
    assert_eq!(base_binding.symbol.scope_path.as_deref(), Some("top_level"));
}

#[test]
fn resolves_python_class_base_references_to_globals_not_class_locals() {
    let source = r#"
class Base:
    pass

class Container:
    value = 0
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Container",
        "class Container(Base):\n    Base = 1\n    value = 0\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    let base_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "Base")
        .unwrap();
    assert_eq!(base_binding.symbol.node_kind, "class_definition");
    assert_eq!(base_binding.symbol.semantic_path, "Base");
}

#[test]
fn resolves_python_nested_class_metaclass_closure_bindings() {
    let source = r#"
class GlobalMeta(type):
    pass

def top_level() -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    Meta = GlobalMeta\n\n    class Inner(metaclass=Meta):\n        pass\n\n    return 0\n",
            None,
        )
        .unwrap();

    assert!(result.applied);
    let meta_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "Meta")
        .unwrap();
    assert_eq!(meta_binding.symbol.node_kind, "assignment");
    assert_eq!(meta_binding.symbol.scope_path.as_deref(), Some("top_level"));
}

#[test]
fn resolves_python_class_metaclass_references_to_globals_not_class_locals() {
    let source = r#"
class Meta(type):
    pass

class Container:
    value = 0
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Container",
        "class Container(metaclass=Meta):\n    Meta = 1\n    value = 0\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    let meta_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "Meta")
        .unwrap();
    assert_eq!(meta_binding.symbol.node_kind, "class_definition");
    assert_eq!(meta_binding.symbol.semantic_path, "Meta");
}

#[test]
fn resolves_python_class_comprehension_references_to_globals_not_class_locals() {
    let source = r#"
def helper() -> int:
    return 1

class Container:
    value = 0
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Container",
        "class Container:\n    helper = 2\n    value = [helper for item in range(1)]\n",
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
    assert_eq!(helper_binding.symbol.node_kind, "function_definition");
    assert_eq!(helper_binding.symbol.semantic_path, "helper");
}

#[test]
fn resolves_python_nested_lambda_closure_bindings() {
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
        "def top_level() -> int:\n    return (lambda target: (lambda: target)())(3)\n",
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
