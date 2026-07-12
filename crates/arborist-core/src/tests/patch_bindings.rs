use std::fs;
use std::path::Path;

use super::support::temporary_dir;
use super::{patch_ast_node, patch_ast_node_from_path, preview_patch_ast_node_from_path};

#[test]
fn rejects_patch_with_unresolved_identifier_without_bypass() {
    let source = r#"
def helper(value: int) -> int:
    return value + 1

def top_level(value: int) -> int:
    return helper(value)
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: int) -> int:\n    return missing_helper(value)\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(!result.validation.commit_gate.allowed);
    assert_eq!(result.validation.commit_gate.status, "rejected");
    assert_eq!(
        result.validation.commit_gate.reason,
        "symbol binding is unresolved"
    );
    assert_eq!(
        result.validation.unresolved_identifiers,
        vec!["missing_helper"]
    );
    assert_eq!(result.validation.binding_decisions.len(), 2);
    let missing_helper_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "missing_helper")
        .unwrap();
    assert_eq!(missing_helper_decision.status, "unresolved");
    assert_eq!(missing_helper_decision.selected_symbol_id, None);
    assert!(missing_helper_decision.candidates.is_empty());
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "value" && decision.status == "resolved")
    );
}

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

#[test]
fn replaces_python_decorated_function_without_retaining_old_decorators() {
    let source = r#"
def decorator(func):
    return func

@decorator
def top_level() -> int:
    return 1
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level() -> int:\n    return 2\n",
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
    assert!(result.updated_source.contains("def top_level() -> int:"));
    assert_eq!(result.resolved_path, "top_level");
}

#[test]
fn replaces_python_decorated_function_when_new_code_keeps_decorator() {
    let source = r#"
def decorator(func):
    return func

@decorator
def top_level() -> int:
    return 1
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "@decorator\ndef top_level() -> int:\n    return 2\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.syntax_errors.is_empty());
    assert!(
        result
            .updated_source
            .contains("@decorator\ndef top_level() -> int:")
    );
    assert!(result.updated_source.contains("return 2"));
}

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
fn replaces_python_decorated_async_function_without_retaining_old_decorators() {
    let source = r#"
def decorator(func):
    return func

@decorator
async def top_level() -> int:
    return 1
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "async def top_level() -> int:\n    return 2\n",
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
    assert!(
        result
            .updated_source
            .contains("async def top_level() -> int:")
    );
    assert_eq!(result.resolved_path, "top_level");
}

#[test]
fn reindents_python_nested_method_replacements() {
    let source = r#"
class Product:
    def price_with_tax(self, rate: float) -> float:
        return self.price
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.price_with_tax",
        "def price_with_tax(self, rate: float) -> float:\n    return self.price * rate\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.syntax_errors.is_empty());
    assert!(result.updated_source.contains(
        "    def price_with_tax(self, rate: float) -> float:\n        return self.price * rate"
    ));
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn reindents_python_tab_indented_method_replacements_without_mixing_spaces() {
    let source = "class Product:\n\tdef price_with_tax(self, rate: float) -> float:\n\t\treturn self.price\n";

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.price_with_tax",
        "def price_with_tax(self, rate: float) -> float:\n    return self.price * rate\n",
        None,
    )
    .unwrap();

    assert!(
        result.applied,
        "{updated_source:?}\n{validation:#?}",
        updated_source = result.updated_source,
        validation = result.validation
    );
    assert!(result.validation.syntax_errors.is_empty());
    assert!(result.updated_source.contains(
        "class Product:\n\tdef price_with_tax(self, rate: float) -> float:\n\t\treturn self.price * rate\n"
    ));
    assert!(!result.updated_source.contains("\t    return"));
}

#[test]
fn preserves_python_crlf_line_endings_in_replacements() {
    let source = "def helper() -> int:\r\n    return 1\r\n";

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "helper",
        "def helper() -> int:\n    return 2\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.syntax_errors.is_empty());
    assert!(
        result
            .updated_source
            .contains("def helper() -> int:\r\n    return 2\r\n")
    );
    assert!(!result.updated_source.replace("\r\n", "").contains('\n'));
}

#[test]
fn rejects_bad_python_nested_method_indentation_before_binding_validation() {
    let source = r#"
class Product:
    def price_with_tax(self, rate: float) -> float:
        return self.price
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.price_with_tax",
        "def price_with_tax(self, rate: float) -> float:\nreturn self.price * rate\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(
        result
            .validation
            .syntax_errors
            .iter()
            .any(|issue| issue.kind == "indentation")
    );
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn replaces_python_async_function_without_retaining_old_async_keyword() {
    let source = r#"
async def top_level() -> int:
    return 1
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level() -> int:\n    return 2\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(
        !result
            .updated_source
            .contains("async def top_level() -> int:\n    return 2")
    );
    assert!(
        result
            .updated_source
            .contains("def top_level() -> int:\n    return 2")
    );
    assert_eq!(result.resolved_path, "top_level");
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

#[test]
fn resolves_python_import_alias_patch_bindings_to_local_module_symbols() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("caller.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    \"\"\"Imported helper.\"\"\"\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "import graph_b as gb\nfrom graph_b import helper as h\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return gb.helper(value) + h(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let alias_attribute = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "gb.helper")
        .unwrap();
    assert_eq!(alias_attribute.symbol.semantic_path, "helper");
    assert_eq!(
        alias_attribute.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(alias_attribute.symbol.origin_type, "imported_module");
    assert_eq!(
        alias_attribute.symbol.docstring.as_deref(),
        Some("\"\"\"Imported helper.\"\"\"")
    );

    let alias_import = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "h")
        .unwrap();
    assert_eq!(alias_import.symbol.semantic_path, "helper");
    assert_eq!(
        alias_import.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(alias_import.symbol.origin_type, "imported_module");
}

#[test]
fn resolves_python_relative_import_alias_patch_bindings_to_local_module_symbols() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let subpackage = package.join("sub");
    let helper = package.join("graph_b.py");
    let local_helper = subpackage.join("local_mod.py");
    let caller = subpackage.join("caller.py");

    fs::create_dir_all(&subpackage).unwrap();
    fs::write(package.join("__init__.py"), "").unwrap();
    fs::write(subpackage.join("__init__.py"), "").unwrap();
    fs::write(
            &helper,
            "def helper(value: int) -> int:\n    \"\"\"Parent package helper.\"\"\"\n    return value + 1\n",
        )
        .unwrap();
    fs::write(
            &local_helper,
            "def helper2(value: int) -> int:\n    \"\"\"Sibling package helper.\"\"\"\n    return value + 2\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "from ..graph_b import helper as h\nfrom .local_mod import helper2 as h2\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return h(value) + h2(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let imported_helper = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "h")
        .unwrap();
    assert_eq!(imported_helper.symbol.semantic_path, "helper");
    assert_eq!(
        imported_helper.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(imported_helper.symbol.origin_type, "imported_module");
    assert_eq!(
        imported_helper.symbol.docstring.as_deref(),
        Some("\"\"\"Parent package helper.\"\"\"")
    );

    let sibling_helper = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "h2")
        .unwrap();
    assert_eq!(sibling_helper.symbol.semantic_path, "helper2");
    assert_eq!(
        sibling_helper.symbol.file_path,
        local_helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(sibling_helper.symbol.origin_type, "imported_module");
    assert_eq!(
        sibling_helper.symbol.docstring.as_deref(),
        Some("\"\"\"Sibling package helper.\"\"\"")
    );
}

#[test]
fn resolves_python_absolute_package_import_alias_patch_bindings_to_local_module_symbols() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let subpackage = package.join("sub");
    let helper = package.join("graph_c.py");
    let caller = subpackage.join("caller.py");

    fs::create_dir_all(&subpackage).unwrap();
    fs::write(package.join("__init__.py"), "").unwrap();
    fs::write(subpackage.join("__init__.py"), "").unwrap();
    fs::write(
            &helper,
            "def worker(value: int) -> int:\n    \"\"\"Absolute package worker.\"\"\"\n    return value + 3\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "import pkg.graph_c as gc\nfrom pkg.graph_c import worker as w\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return gc.worker(value) + w(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let module_alias = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "gc.worker")
        .unwrap();
    assert_eq!(module_alias.symbol.semantic_path, "worker");
    assert_eq!(
        module_alias.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(module_alias.symbol.origin_type, "imported_module");

    let symbol_alias = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "w")
        .unwrap();
    assert_eq!(symbol_alias.symbol.semantic_path, "worker");
    assert_eq!(
        symbol_alias.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(symbol_alias.symbol.origin_type, "imported_module");
    assert_eq!(
        symbol_alias.symbol.docstring.as_deref(),
        Some("\"\"\"Absolute package worker.\"\"\"")
    );
}

#[test]
fn resolves_python_import_from_module_alias_patch_bindings_to_local_module_symbols() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let subpackage = package.join("sub");
    let helper = package.join("graph_c.py");
    let local_helper = subpackage.join("local_mod.py");
    let caller = subpackage.join("caller.py");

    fs::create_dir_all(&subpackage).unwrap();
    fs::write(package.join("__init__.py"), "").unwrap();
    fs::write(subpackage.join("__init__.py"), "").unwrap();
    fs::write(
            &helper,
            "def worker(value: int) -> int:\n    \"\"\"Absolute package worker.\"\"\"\n    return value + 3\n",
        )
        .unwrap();
    fs::write(
            &local_helper,
            "def helper2(value: int) -> int:\n    \"\"\"Sibling module helper.\"\"\"\n    return value + 2\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "from pkg import graph_c as gc\nfrom . import local_mod as lm\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return gc.worker(value) + lm.helper2(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let package_module_alias = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "gc.worker")
        .unwrap();
    assert_eq!(package_module_alias.symbol.semantic_path, "worker");
    assert_eq!(
        package_module_alias.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(package_module_alias.symbol.origin_type, "imported_module");

    let sibling_module_alias = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "lm.helper2")
        .unwrap();
    assert_eq!(sibling_module_alias.symbol.semantic_path, "helper2");
    assert_eq!(
        sibling_module_alias.symbol.file_path,
        local_helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(sibling_module_alias.symbol.origin_type, "imported_module");
}

#[test]
fn resolves_python_package_reexport_patch_bindings_to_underlying_local_symbols() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let helper = package.join("graph_c.py");
    let caller = dir.join("caller.py");

    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("__init__.py"),
        "from .graph_c import worker as worker\n",
    )
    .unwrap();
    fs::write(
            &helper,
            "def worker(value: int) -> int:\n    \"\"\"Re-exported package worker.\"\"\"\n    return value + 4\n",
        )
        .unwrap();
    fs::write(
        &caller,
        "from pkg import worker\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return worker(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let imported_worker = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "worker")
        .unwrap();
    assert_eq!(imported_worker.symbol.semantic_path, "worker");
    assert_eq!(
        imported_worker.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(imported_worker.symbol.origin_type, "imported_module");
    assert_eq!(
        imported_worker.symbol.docstring.as_deref(),
        Some("\"\"\"Re-exported package worker.\"\"\"")
    );
}

#[test]
fn resolves_decorated_python_local_bindings_for_patch_validation() {
    let source = r#"
def decorator(func):
    return func

@decorator
def helper(value: int) -> int:
    return value + 1

def top_level(value: int) -> int:
    return value + 1
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: int) -> int:\n    return helper(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let helper_text = "@decorator\ndef helper(value: int) -> int:\n    return value + 1";
    let helper_start = source.find(helper_text).unwrap();
    let helper_end = helper_start + helper_text.len();

    let helper_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "helper")
        .unwrap();
    assert_eq!(helper_binding.symbol.semantic_path, "helper");
    assert_eq!(helper_binding.symbol.origin_type, "module_scope");
    assert_eq!(
        helper_binding.symbol.signature.as_deref(),
        Some("@decorator\ndef helper(value: int) -> int:")
    );
    assert_eq!(helper_binding.symbol.byte_range, (helper_start, helper_end));
}

#[test]
fn resolves_decorated_python_import_metadata_for_patch_validation() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("caller.py");

    fs::write(
            &helper,
            "def decorator(func):\n    return func\n\n@decorator\ndef helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "import graph_b as gb\nfrom graph_b import helper as h\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return gb.helper(value) + h(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let helper_source = fs::read_to_string(&helper).unwrap();
    let helper_text = "@decorator\ndef helper(value: int) -> int:\n    return value + 1";
    let helper_start = helper_source.find(helper_text).unwrap();
    let helper_end = helper_start + helper_text.len();

    let imported_helper = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "h")
        .unwrap();
    assert_eq!(imported_helper.symbol.semantic_path, "helper");
    assert_eq!(imported_helper.symbol.origin_type, "imported_module");
    assert_eq!(
        imported_helper.symbol.signature.as_deref(),
        Some("@decorator\ndef helper(value: int) -> int:")
    );
    assert_eq!(
        imported_helper.symbol.byte_range,
        (helper_start, helper_end)
    );
    assert!(
        imported_helper
            .symbol
            .evidence_key
            .contains("@decorator\ndef helper(value: int) -> int:")
    );
}

#[test]
fn resolves_imported_module_attribute_calls_for_patch_validation() {
    let source = r#"
import json
import os

def top_level() -> str:
    return ""
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level() -> str:\n    return json.dumps({'pid': os.getpid()})\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(
        result
            .validation
            .resolved_identifiers
            .iter()
            .any(|binding| binding.name == "json.dumps")
    );
    assert!(
        result
            .validation
            .resolved_identifiers
            .iter()
            .any(|binding| binding.name == "os.getpid")
    );
}

#[test]
fn allows_patch_with_bypass_reason() {
    let source = r#"
def top_level(value: int) -> int:
    return value + 1
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: int) -> int:\n    return dynamic_runtime(value)\n",
        Some("resolved at runtime by the embedding host"),
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.bypass_applied);
    assert!(result.validation.commit_gate.allowed);
    assert_eq!(result.validation.commit_gate.status, "allowed_with_bypass");
    assert_eq!(
        result.validation.commit_gate.bypass_reason.as_deref(),
        Some("resolved at runtime by the embedding host")
    );
}

#[test]
fn rejects_blank_patch_bypass_reasons() {
    let source = r#"
def top_level(value: int) -> int:
    return value + 1
"#;

    let error = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 2\n",
        Some(" \t"),
    )
    .expect_err("blank bypass reasons should be rejected");

    assert!(error.to_string().contains("bypass_reason"));
    assert!(error.to_string().contains("blank"));
}

#[test]
fn writes_applied_patch_to_disk() {
    let dir = temporary_dir();
    let file = dir.join("patch_target.py");
    fs::write(
        &file,
        "def top_level(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 2\n",
        None,
    )
    .unwrap();

    let updated = fs::read_to_string(&file).unwrap();
    assert!(result.applied);
    assert!(updated.contains("return value + 2"));
}

#[test]
fn previews_patch_diff_without_writing_to_disk() {
    let dir = temporary_dir();
    let file = dir.join("patch_target.py");
    let original = "def top_level(value: int) -> int:\n    return value + 1\n";
    fs::write(&file, original).unwrap();

    let result = preview_patch_ast_node_from_path(
        &file,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 2\n",
        None,
    )
    .unwrap();

    let disk_source = fs::read_to_string(&file).unwrap();
    assert_eq!(disk_source, original);
    assert!(result.patch.applied);
    assert!(result.changed);
    assert!(result.unified_diff.contains("--- a/"));
    assert!(result.unified_diff.contains("-    return value + 1"));
    assert!(result.unified_diff.contains("+    return value + 2"));
}
