use super::*;

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
fn execute_tree_query_reports_owner_for_decorator_captures() {
    let source = "@logged\ndef top_level(value):\n    return value\n";
    let query = "(decorator (identifier) @decorator)";

    let captures = execute_tree_query(Path::new("sample.py"), source, query).unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].capture_name, "decorator");
    assert_eq!(captures[0].text, "logged");
    assert_eq!(captures[0].owner_symbol_id.as_deref(), Some("top_level"));
    assert_eq!(
        captures[0].owner_semantic_path.as_deref(),
        Some("top_level")
    );
    assert_eq!(captures[0].owner_scope_path, None);
}

#[test]
fn execute_tree_query_reports_owner_for_nested_decorator_captures() {
    let source = "def outer(value):\n    @logged\n    def inner():\n        return value\n    return inner()\n";
    let query = "(decorator (identifier) @decorator)";

    let captures = execute_tree_query(Path::new("sample.py"), source, query).unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].capture_name, "decorator");
    assert_eq!(captures[0].text, "logged");
    assert_eq!(captures[0].owner_symbol_id.as_deref(), Some("outer.inner"));
    assert_eq!(
        captures[0].owner_semantic_path.as_deref(),
        Some("outer.inner")
    );
    assert_eq!(captures[0].owner_scope_path.as_deref(), Some("outer"));
}

#[test]
fn execute_tree_query_reports_owner_for_c_body_captures() {
    let source = "int helper(int value) { return value + 1; }\nint orchestrate(int value) { return helper(value); }\n";
    let query = "(call_expression function: (identifier) @callee)";

    let captures = execute_tree_query(Path::new("sample.c"), source, query).unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].capture_name, "callee");
    assert_eq!(captures[0].text, "helper");
    assert!(
        captures[0]
            .owner_symbol_id
            .as_deref()
            .is_some_and(|owner| owner.ends_with("/sample.c::orchestrate"))
    );
    assert_eq!(
        captures[0].owner_semantic_path.as_deref(),
        Some("orchestrate")
    );
    assert_eq!(captures[0].owner_scope_path, None);
}

#[test]
fn execute_tree_query_reports_owner_for_cpp_namespace_function_captures() {
    let source = "namespace alpha::detail {\nint helper(int value) { return value + 1; }\nint orchestrate(int value) { return helper(value); }\n}\n";
    let query = "(identifier) @candidate";

    let captures = execute_tree_query(Path::new("sample.cpp"), source, query).unwrap();
    let capture = captures
        .iter()
        .find(|capture| {
            capture.text == "orchestrate"
                && capture.owner_semantic_path.as_deref() == Some("alpha::detail::orchestrate")
        })
        .expect("namespace function should report its semantic owner");

    assert_eq!(
        capture.owner_symbol_id.as_deref(),
        Some("alpha::detail::orchestrate")
    );
    assert_eq!(
        capture.owner_semantic_path.as_deref(),
        Some("alpha::detail::orchestrate")
    );
    assert_eq!(capture.owner_scope_path.as_deref(), Some("alpha::detail"));
}

#[test]
fn execute_tree_query_reports_owner_for_c_declaration_captures() {
    let source = "int helper(int value);\n";
    let query = "(function_declarator declarator: (identifier) @name)";

    let captures = execute_tree_query(Path::new("sample.h"), source, query).unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].capture_name, "name");
    assert_eq!(captures[0].text, "helper");
    assert!(
        captures[0]
            .owner_symbol_id
            .as_deref()
            .is_some_and(|owner| owner.ends_with("/sample.h::helper"))
    );
    assert_eq!(captures[0].owner_semantic_path.as_deref(), Some("helper"));
    assert_eq!(captures[0].owner_scope_path, None);
}
