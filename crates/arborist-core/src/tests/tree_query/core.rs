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
