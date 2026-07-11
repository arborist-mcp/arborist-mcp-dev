use super::*;

#[test]
fn builds_python_skeleton_with_nested_members() {
    let source = r#"
class Greeter:
    """Helpful greeter."""

    def greet(self, name: str) -> str:
        """Return a greeting."""
        return f"hello, {name}"

def top_level(value: int) -> int:
    """Top level orchestration."""

    def nested(inner: int) -> int:
        """Inner increment helper."""
        return inner + 1

    return nested(value)
"#;

    let skeleton = get_semantic_skeleton(Path::new("sample.py"), source, 2, &[]).unwrap();

    assert!(skeleton.skeleton.contains("class Greeter: ..."));
    assert!(
        skeleton
            .skeleton
            .contains("def top_level(value: int) -> int: ...")
    );
    assert!(
        skeleton
            .skeleton
            .contains("def nested(inner: int) -> int: ...")
    );
    assert_eq!(
        skeleton.available_paths,
        vec!["Greeter", "Greeter.greet", "top_level", "top_level.nested"]
    );
    assert_eq!(skeleton.available_symbols.len(), 4);
    assert_eq!(skeleton.available_symbols[0].symbol_id, "Greeter");
    assert_eq!(skeleton.available_symbols[0].semantic_path, "Greeter");
    assert_eq!(skeleton.available_symbols[0].scope_path, None);
    assert_eq!(skeleton.available_symbols[0].node_kind, "class_definition");
    assert_eq!(
        skeleton.available_symbols[0].signature.as_deref(),
        Some("class Greeter:")
    );
    assert!(skeleton.available_symbols[0].parameters.is_empty());
    assert_eq!(skeleton.available_symbols[0].return_type, None);
    assert_eq!(
        skeleton.available_symbols[0].docstring.as_deref(),
        Some("\"\"\"Helpful greeter.\"\"\"")
    );
    assert_eq!(skeleton.available_symbols[3].symbol_id, "top_level.nested");
    assert_eq!(
        skeleton.available_symbols[3].scope_path.as_deref(),
        Some("top_level")
    );
    assert_eq!(
        skeleton.available_symbols[3].signature.as_deref(),
        Some("def nested(inner: int) -> int:")
    );
    assert_eq!(
        skeleton.available_symbols[3].parameters,
        vec!["inner: int".to_string()]
    );
    assert_eq!(
        skeleton.available_symbols[3].return_type.as_deref(),
        Some("int")
    );
    assert_eq!(
        skeleton.available_symbols[3].docstring.as_deref(),
        Some("\"\"\"Inner increment helper.\"\"\"")
    );
}

#[test]
fn builds_python_skeleton_with_async_members() {
    let source = r#"
async def top_level(value: int) -> int:
    """Top level async orchestration."""

    async def nested(inner: int) -> int:
        """Inner async helper."""
        return inner + 1

    return await nested(value)
"#;

    let skeleton = get_semantic_skeleton(Path::new("sample.py"), source, 2, &[]).unwrap();

    assert!(
        skeleton
            .skeleton
            .contains("async def top_level(value: int) -> int: ...")
    );
    assert!(
        skeleton
            .skeleton
            .contains("async def nested(inner: int) -> int: ...")
    );
    assert_eq!(
        skeleton.available_paths,
        vec!["top_level", "top_level.nested"]
    );
    assert_eq!(skeleton.available_symbols.len(), 2);
    assert_eq!(
        skeleton.available_symbols[0].node_kind,
        "function_definition"
    );
    assert_eq!(
        skeleton.available_symbols[0].signature.as_deref(),
        Some("async def top_level(value: int) -> int:")
    );
    assert_eq!(
        skeleton.available_symbols[1].scope_path.as_deref(),
        Some("top_level")
    );
    assert_eq!(
        skeleton.available_symbols[1].signature.as_deref(),
        Some("async def nested(inner: int) -> int:")
    );
}

#[test]
fn builds_python_skeleton_with_decorated_members() {
    let source = r#"
def decorator(func):
    return func

@decorator
def top_level(value: int) -> int:
    return value
"#;

    let skeleton = get_semantic_skeleton(Path::new("sample.py"), source, 1, &[]).unwrap();

    assert!(skeleton.skeleton.contains("@decorator"));
    assert!(
        skeleton
            .skeleton
            .contains("def top_level(value: int) -> int: ...")
    );
    let top_level = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "top_level")
        .unwrap();
    assert_eq!(
        top_level.signature.as_deref(),
        Some("@decorator\ndef top_level(value: int) -> int:")
    );
}

#[test]
fn uses_decorated_python_member_ranges_in_skeleton_metadata() {
    let source = r#"
def decorator(func):
    return func

@decorator
def top_level(value: int) -> int:
    return value
"#;

    let skeleton = get_semantic_skeleton(Path::new("sample.py"), source, 1, &[]).unwrap();
    let top_level = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "top_level")
        .unwrap();
    let decorated_symbol = "@decorator\ndef top_level(value: int) -> int:\n    return value";
    let start = source.find(decorated_symbol).unwrap();
    let end = start + decorated_symbol.len();

    assert_eq!(top_level.byte_range, (start, end));
}

#[test]
fn expands_selected_python_nodes_without_duplicating_children() {
    let source = r#"
class Greeter:
    def greet(self, name: str) -> str:
        return f"hello, {name}"

def top_level(value: int) -> int:
    def nested(inner: int) -> int:
        return inner + 1

    return nested(value)
"#;

    let skeleton = get_semantic_skeleton(
        Path::new("sample.py"),
        source,
        2,
        &["Greeter".to_string(), "top_level.nested".to_string()],
    )
    .unwrap();

    assert!(skeleton.skeleton.contains("class Greeter:\n    def greet"));
    assert!(!skeleton.skeleton.contains("class Greeter: ..."));
    assert_eq!(skeleton.skeleton.matches("def greet").count(), 1);
    assert!(
        skeleton
            .skeleton
            .contains("def nested(inner: int) -> int:\n        return inner + 1")
    );
}

#[test]
fn expands_selected_python_nodes_beyond_depth_limit() {
    let source = r#"
def top_level(value: int) -> int:
    def nested(inner: int) -> int:
        return inner + 1

    return nested(value)
"#;

    let skeleton = get_semantic_skeleton(
        Path::new("sample.py"),
        source,
        1,
        &["top_level.nested".to_string()],
    )
    .unwrap();

    assert!(
        skeleton
            .skeleton
            .contains("def nested(inner: int) -> int:\n        return inner + 1")
    );
    assert!(
        skeleton
            .available_paths
            .contains(&"top_level.nested".to_string())
    );
}

#[test]
fn expands_decorated_python_nodes_with_decorators() {
    let source = r#"
def decorator(func):
    return func

@decorator
def top_level(value: int) -> int:
    return value + 1
"#;

    let skeleton = get_semantic_skeleton(
        Path::new("sample.py"),
        source,
        1,
        &["top_level".to_string()],
    )
    .unwrap();

    assert!(skeleton.skeleton.contains("@decorator\ndef top_level"));
    assert!(skeleton.skeleton.contains("return value + 1"));
}

#[test]
fn rejects_blank_expand_selectors() {
    let source = "def top_level(value: int) -> int:\n    return value\n";

    let error = get_semantic_skeleton(Path::new("sample.py"), source, 1, &["   ".to_string()])
        .expect_err("blank expand selectors should be rejected");

    assert!(error.to_string().contains("expand_nodes"));
}
