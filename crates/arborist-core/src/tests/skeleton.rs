#[test]
fn semantic_skeleton_timeout_variants_reject_invalid_budgets_before_path_work() {
    let path = Path::new("");
    let errors = [
        get_semantic_skeleton_with_timeout(
            path,
            "value = 1
",
            2,
            &[],
            Some(0),
        )
        .expect_err("inline skeleton should reject zero timeout"),
        get_semantic_skeleton_from_path_with_timeout(path, 2, &[], Some(0))
            .expect_err("path skeleton should reject zero timeout"),
        get_semantic_skeleton_with_timeout(
            path,
            "value = 1
",
            2,
            &[],
            Some(MAX_SEMANTIC_SKELETON_TIMEOUT_MS + 1),
        )
        .expect_err("inline skeleton should reject excessive timeout"),
        get_semantic_skeleton_from_path_with_timeout(
            path,
            2,
            &[],
            Some(MAX_SEMANTIC_SKELETON_TIMEOUT_MS + 1),
        )
        .expect_err("path skeleton should reject excessive timeout"),
    ];

    for error in errors {
        assert!(error.to_string().contains("semantic skeleton timeout_ms"));
    }
}

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

#[test]
fn rejects_excessive_skeleton_depth_before_parsing_source() {
    let error = get_semantic_skeleton(
        Path::new("sample.py"),
        "this is not valid Python",
        crate::MAX_SEMANTIC_SKELETON_DEPTH + 1,
        &[],
    )
    .expect_err("excessive skeleton depth should be rejected");

    assert!(error.to_string().contains("depth_limit"));
    assert!(
        error
            .to_string()
            .contains(&crate::MAX_SEMANTIC_SKELETON_DEPTH.to_string())
    );
}

#[test]
fn rejects_too_many_expand_selectors_before_parsing_source() {
    let selectors = vec!["top_level".to_string(); crate::MAX_SEMANTIC_EXPAND_NODES + 1];
    let error = get_semantic_skeleton(
        Path::new("sample.py"),
        "this is not valid Python",
        1,
        &selectors,
    )
    .expect_err("too many expand selectors should be rejected");

    assert!(error.to_string().contains("expand_nodes"));
    assert!(
        error
            .to_string()
            .contains(&crate::MAX_SEMANTIC_EXPAND_NODES.to_string())
    );
}

#[test]
fn builds_javascript_typescript_and_tsx_semantic_skeletons() {
    for (path, source, expected_path, expected_signature, expected_return_type) in [
        (
            "sample.js",
            "export function helper(value) { return value + 1; }\n",
            "helper",
            "function helper(value)",
            None,
        ),
        (
            "sample.ts",
            "export function helper(value: number): number { return value + 1; }\n",
            "helper",
            "function helper(value: number): number",
            Some("number"),
        ),
        (
            "sample.tsx",
            "export function App(props: { title: string }): JSX.Element { return <main>{props.title}</main>; }\n",
            "App",
            "function App(props: { title: string }): JSX.Element",
            Some("JSX.Element"),
        ),
    ] {
        let skeleton = get_semantic_skeleton(Path::new(path), source, 1, &[]).unwrap();
        let symbol = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == expected_path)
            .unwrap();

        assert_eq!(skeleton.available_paths, vec![expected_path]);
        assert!(skeleton.skeleton.contains(expected_signature));
        assert_eq!(symbol.signature.as_deref(), Some(expected_signature));
        assert_eq!(symbol.return_type.as_deref(), expected_return_type);
    }
}

#[test]
fn expands_javascript_semantic_nodes_without_duplicating_members() {
    let source = "export class Counter { increment(value) { return value + 1; } }\n";

    let skeleton =
        get_semantic_skeleton(Path::new("sample.js"), source, 2, &["Counter".to_string()]).unwrap();

    assert!(
        skeleton
            .skeleton
            .contains("class Counter { increment(value)")
    );
    assert_eq!(skeleton.skeleton.matches("increment(value)").count(), 1);
    assert_eq!(
        skeleton.available_paths,
        vec!["Counter", "Counter::increment"]
    );
    assert_eq!(
        skeleton.available_symbols[1].scope_path.as_deref(),
        Some("Counter")
    );
}

#[test]
fn builds_rust_skeleton_through_public_entrypoint() {
    let source = r#"
pub struct Counter;
impl Counter {
    pub fn increment(&self, amount: u64) -> u64 { amount }
}
"#;
    let skeleton = get_semantic_skeleton(Path::new("sample.rs"), source, 2, &[]).unwrap();

    assert_eq!(
        skeleton.available_paths,
        vec!["Counter", "Counter::increment"]
    );
    let increment = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "Counter::increment")
        .unwrap();
    assert_eq!(increment.parameters, vec!["&self", "amount: u64"]);
    assert_eq!(increment.return_type.as_deref(), Some("u64"));
}

#[test]
fn builds_csharp_skeleton_through_public_entrypoint() {
    let source = r#"
namespace Demo.Tools;

public class Counter {
    public Counter(int initial) {}
    public int Increment(int amount) => amount;
}
"#;
    let skeleton = get_semantic_skeleton(Path::new("Counter.cs"), source, 4, &[]).unwrap();

    assert_eq!(
        skeleton.available_paths,
        vec![
            "Demo::Tools::Counter",
            "Demo::Tools::Counter::Counter",
            "Demo::Tools::Counter::Increment",
        ]
    );
    let increment = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "Demo::Tools::Counter::Increment")
        .unwrap();
    assert_eq!(increment.parameters, vec!["int amount"]);
    assert_eq!(increment.return_type.as_deref(), Some("int"));
}

#[test]
fn builds_go_skeleton_through_public_entrypoint() {
    let source = r#"
package metrics

type Counter struct { value int }
func (counter *Counter) Increment(amount int) int { return counter.value + amount }
"#;
    let skeleton = get_semantic_skeleton(Path::new("metrics.go"), source, 2, &[]).unwrap();

    assert_eq!(
        skeleton.available_paths,
        vec!["Counter", "Counter::Increment"]
    );
    let increment = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "Counter::Increment")
        .unwrap();
    assert_eq!(increment.parameters, vec!["amount int"]);
    assert_eq!(increment.return_type.as_deref(), Some("int"));
}
