use super::super::*;

#[test]
fn executes_tree_query() {
    let source = "def add(left, right):\n    return left + right\n";
    let query = "(function_definition name: (identifier) @name)";

    let captures = execute_tree_query(Path::new("sample.py"), source, query).unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].capture_name, "name");
    assert_eq!(captures[0].text, "add");
    assert_eq!(captures[0].owner_symbol_id.as_deref(), Some("add"));
    assert_eq!(captures[0].owner_semantic_path.as_deref(), Some("add"));
    assert_eq!(captures[0].owner_scope_path, None);
}

#[test]
fn rejects_blank_tree_queries() {
    let source = "def add(left, right):\n    return left + right\n";

    let error = execute_tree_query(Path::new("sample.py"), source, " \t")
        .expect_err("blank Tree-sitter queries should be rejected");

    assert!(error.to_string().contains("query"));
    assert!(error.to_string().contains("blank"));
}

#[test]
fn rejects_oversized_tree_queries() {
    let source = "def add(left, right):\n    return left + right\n";
    let query = "(".repeat(DEFAULT_TREE_QUERY_MAX_BYTES + 1);

    let error = execute_tree_query(Path::new("sample.py"), source, &query)
        .expect_err("oversized Tree-sitter queries should be rejected before compilation");

    assert!(error.to_string().contains("max query bytes"));
}

#[test]
fn execute_tree_query_rejects_capture_limit_overflow() {
    let source = "def add(left, right):\n    total = left + right\n    return total\n";
    let query = "(identifier) @name";

    let error = execute_tree_query_with_limit(Path::new("sample.py"), source, query, 2)
        .expect_err("queries should fail once max_captures is exceeded");

    assert!(error.to_string().contains("capture limit exceeded"));
    assert!(error.to_string().contains("max_captures=2"));
}

#[test]
fn executes_tree_queries_for_javascript_typescript_rust_go_java_kotlin_and_csharp_adapters() {
    for (path, source, query, expected) in [
        (
            "Sample.cs",
            "public class Sample { public int Add(int left, int right) => left + right; }",
            "(method_declaration name: (identifier) @name)",
            "Add",
        ),
        (
            "sample.js",
            "export function add(left, right) { return left + right; }",
            "(function_declaration name: (identifier) @name)",
            "add",
        ),
        (
            "sample.ts",
            "export function add(left: number, right: number): number { return left + right; }",
            "(function_declaration name: (identifier) @name)",
            "add",
        ),
        (
            "sample.tsx",
            "export const App = () => <main>ready</main>;",
            "(identifier) @name",
            "App",
        ),
        (
            "sample.rs",
            "pub fn add(left: i32, right: i32) -> i32 { left + right }",
            "(function_item name: (identifier) @name)",
            "add",
        ),
        (
            "sample.go",
            "package sample\nfunc Add(left int, right int) int { return left + right }\n",
            "(function_declaration name: (identifier) @name)",
            "Add",
        ),
        (
            "Sample.java",
            "class Sample { int add(int left, int right) { return left + right; } }",
            "(method_declaration name: (identifier) @name)",
            "add",
        ),
        (
            "Sample.kt",
            "package demo; class Sample { fun add(left: Int, right: Int) = left + right; }",
            "(function_declaration name: (identifier) @name)",
            "add",
        ),
    ] {
        let captures = execute_tree_query(Path::new(path), source, query).unwrap();
        assert!(captures.iter().any(|capture| capture.text == expected));
        assert!(
            captures
                .iter()
                .all(|capture| capture.owner_symbol_id.is_none())
        );
    }
}
