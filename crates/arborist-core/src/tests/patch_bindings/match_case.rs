use super::*;

#[test]
fn resolves_python_match_keyword_patch_bindings() {
    let source = r#"
class Point:
    __match_args__ = ()

def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case Point(value=target):\n            return target\n        case _:\n            return 0\n",
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
    assert_eq!(target_binding.symbol.node_kind, "match_capture");
    assert_eq!(
        target_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn resolves_python_match_capture_patch_bindings() {
    let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case {\"target\": target}:\n            return target\n        case _:\n            return 0\n",
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
    assert_eq!(target_binding.symbol.node_kind, "match_capture");
    assert_eq!(
        target_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn resolves_python_match_alias_patch_bindings() {
    let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case int() as target:\n            return target\n        case _:\n            return 0\n",
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
    assert_eq!(target_binding.symbol.node_kind, "match_capture");
    assert_eq!(
        target_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn resolves_python_match_splat_patch_bindings() {
    let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case [*target]:\n            return target\n        case _:\n            return 0\n",
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
    assert_eq!(target_binding.symbol.node_kind, "match_capture");
    assert_eq!(
        target_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn resolves_python_match_list_capture_patch_bindings() {
    let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case [target]:\n            return target\n        case _:\n            return 0\n",
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
    assert_eq!(target_binding.symbol.node_kind, "match_capture");
    assert_eq!(
        target_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn resolves_python_match_mapping_splat_patch_bindings() {
    let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case {\"x\": _, **target}:\n            return target\n        case _:\n            return 0\n",
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
    assert_eq!(target_binding.symbol.node_kind, "match_capture");
    assert_eq!(
        target_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn resolves_python_match_class_positional_patch_bindings() {
    let source = r#"
class Point:
    __match_args__ = ("value",)

def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case Point(target):\n            return target\n        case _:\n            return 0\n",
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
    assert_eq!(target_binding.symbol.node_kind, "match_capture");
    assert_eq!(
        target_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn resolves_python_match_union_patch_bindings() {
    let source = r#"
class Point:
    __match_args__ = ("value",)

class Value:
    __match_args__ = ("value",)

def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case Point(target) | Value(target):\n            return target\n        case _:\n            return 0\n",
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
    assert_eq!(target_binding.symbol.node_kind, "match_capture");
    assert_eq!(
        target_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );
}

#[test]
fn rejects_python_match_guard_references_after_prior_capture() {
    let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case [target]:\n            return 0\n        case _ if target():\n            return 1\n        case _:\n            return 2\n",
            None,
        )
        .unwrap();

    assert!(!result.applied);
    assert!(
        result
            .validation
            .unresolved_identifiers
            .iter()
            .any(|name| name == "target")
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
fn resolves_python_match_mixed_mapping_patch_bindings() {
    let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

    let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case {\"x\": x, **target}:\n            return x + target\n        case _:\n            return 0\n",
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
    assert_eq!(target_binding.symbol.node_kind, "match_capture");
    assert_eq!(
        target_binding.symbol.scope_path.as_deref(),
        Some("top_level")
    );

    let x_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "x")
        .unwrap();
    assert_eq!(x_binding.symbol.node_kind, "match_capture");
    assert_eq!(x_binding.symbol.scope_path.as_deref(), Some("top_level"));
}
