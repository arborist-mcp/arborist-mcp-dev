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
fn execute_tree_query_reports_owner_for_cpp_extern_c_function() {
    let source = "extern \"C\" int helper(int value) { return value + 1; }\n";
    let captures = execute_tree_query(
        Path::new("bridge.cpp"),
        source,
        "(function_definition) @function",
    )
    .unwrap();

    assert_eq!(captures.len(), 1);
    assert!(
        captures[0]
            .owner_symbol_id
            .as_deref()
            .is_some_and(|symbol_id| symbol_id.ends_with("/bridge.cpp::helper"))
    );
    assert_eq!(captures[0].owner_semantic_path.as_deref(), Some("helper"));
    assert_eq!(captures[0].owner_scope_path, None);
}

#[test]
fn execute_tree_query_reports_owner_for_conditionally_compiled_cpp_class_method() {
    let source = "namespace api {\nclass Config {\n#if ENABLED\npublic:\n    int enabled() { return 1; }\n#endif\n};\n}\n";
    let captures = execute_tree_query(
        Path::new("config.hpp"),
        source,
        "(field_identifier) @method",
    )
    .unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].text, "enabled");
    assert_eq!(
        captures[0].owner_semantic_path.as_deref(),
        Some("api::Config::enabled")
    );
    assert_eq!(captures[0].owner_scope_path.as_deref(), Some("api::Config"));
}

#[test]
fn execute_tree_query_reports_owner_for_cpp_class_method_captures() {
    let source = "namespace api {\nclass Counter {\npublic:\n    int increment(int value) { return value + 1; }\n};\n}\n";
    let captures = execute_tree_query(
        Path::new("counter.cpp"),
        source,
        "[(identifier) @candidate (field_identifier) @candidate]",
    )
    .unwrap();
    let capture = captures
        .iter()
        .find(|capture| {
            capture.text == "increment"
                && capture.owner_semantic_path.as_deref() == Some("api::Counter::increment")
        })
        .expect("class method should report its semantic owner");

    assert_eq!(
        capture.owner_symbol_id.as_deref(),
        Some("api::Counter::increment")
    );
    assert_eq!(capture.owner_scope_path.as_deref(), Some("api::Counter"));
}

#[test]
fn execute_tree_query_reports_owner_for_cpp_inline_friend_function() {
    let source = "namespace api {\nclass Token {\n    friend int inspect(const Token&) { return 1; }\n};\n}\n";
    let captures = execute_tree_query(
        Path::new("token.hpp"),
        source,
        "[(identifier) @function (field_identifier) @function]",
    )
    .unwrap();
    let capture = captures
        .iter()
        .find(|capture| capture.text == "inspect")
        .expect("friend function name should be captured");

    assert_eq!(capture.owner_semantic_path.as_deref(), Some("api::inspect"));
    assert_eq!(capture.owner_scope_path.as_deref(), Some("api"));
}

#[test]
fn execute_tree_query_reports_owner_for_cpp_struct_method_captures() {
    let source = "namespace api {\nstruct Counter {\n    int increment(int value) { return value + 1; }\n};\n}\n";
    let captures = execute_tree_query(
        Path::new("counter.cpp"),
        source,
        "(field_identifier) @method",
    )
    .unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].text, "increment");
    assert_eq!(
        captures[0].owner_semantic_path.as_deref(),
        Some("api::Counter::increment")
    );
    assert_eq!(
        captures[0].owner_scope_path.as_deref(),
        Some("api::Counter")
    );
}

#[test]
fn execute_tree_query_reports_owner_for_cpp_class_definition() {
    let source = "namespace api {\nclass Counter {\npublic:\n    int increment(int value) { return value + 1; }\n};\n}\n";
    let captures =
        execute_tree_query(Path::new("counter.cpp"), source, "(type_identifier) @class").unwrap();
    let capture = captures
        .iter()
        .find(|capture| capture.text == "Counter")
        .expect("class name should be captured");

    assert_eq!(capture.owner_symbol_id.as_deref(), Some("api::Counter"));
    assert_eq!(capture.owner_semantic_path.as_deref(), Some("api::Counter"));
    assert_eq!(capture.owner_scope_path.as_deref(), Some("api"));
}

#[test]
fn execute_tree_query_reports_owner_for_cpp_namespace_alias() {
    let source = "namespace api {\nnamespace vendor = third_party::vendor;\n}\n";
    let captures = execute_tree_query(
        Path::new("aliases.hpp"),
        source,
        "(namespace_identifier) @namespace",
    )
    .unwrap();
    let capture = captures
        .iter()
        .find(|capture| {
            capture.text == "vendor"
                && capture.owner_semantic_path.as_deref() == Some("api::vendor")
        })
        .expect("namespace alias name should report its semantic owner");

    assert_eq!(capture.owner_symbol_id.as_deref(), Some("api::vendor"));
    assert_eq!(capture.owner_scope_path.as_deref(), Some("api"));
}

#[test]
fn execute_tree_query_reports_owner_for_cpp_class_method_defined_outside_class() {
    let source = "int api::Counter::increment(int value) { return value + 1; }\n";
    let captures = execute_tree_query(
        Path::new("counter.cpp"),
        source,
        "(qualified_identifier) @method",
    )
    .unwrap();

    let capture = captures
        .iter()
        .find(|capture| capture.text == "api::Counter::increment")
        .expect("fully qualified method name should be captured");
    assert_eq!(
        capture.owner_symbol_id.as_deref(),
        Some("api::Counter::increment")
    );
    assert_eq!(
        capture.owner_semantic_path.as_deref(),
        Some("api::Counter::increment")
    );
    assert_eq!(capture.owner_scope_path.as_deref(), Some("api::Counter"));
}

#[test]
fn execute_tree_query_reports_owner_for_cpp_destructor_definition() {
    let source = "api::Counter::~Counter() {}\n";
    let captures = execute_tree_query(
        Path::new("counter.cpp"),
        source,
        "(destructor_name) @destructor",
    )
    .unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].text, "~Counter");
    assert_eq!(
        captures[0].owner_semantic_path.as_deref(),
        Some("api::Counter::~Counter")
    );
    assert_eq!(
        captures[0].owner_scope_path.as_deref(),
        Some("api::Counter")
    );
}

#[test]
fn execute_tree_query_reports_owner_for_defaulted_cpp_method() {
    let source = "namespace api {\nclass Defaulted {\npublic:\n    Defaulted() = default;\n};\n}\n";
    let captures = execute_tree_query(
        Path::new("lifecycle.hpp"),
        source,
        "(function_definition) @method",
    )
    .unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(
        captures[0].owner_semantic_path.as_deref(),
        Some("api::Defaulted::Defaulted")
    );
    assert_eq!(
        captures[0].owner_scope_path.as_deref(),
        Some("api::Defaulted")
    );
}

#[test]
fn execute_tree_query_reports_owner_for_cpp_template_function() {
    let source = "template <typename T>\nT increment(T value) { return value + 1; }\n";
    let captures = execute_tree_query(
        Path::new("templates.cpp"),
        source,
        "(function_definition) @function",
    )
    .unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(
        captures[0].owner_semantic_path.as_deref(),
        Some("increment")
    );
    assert_eq!(captures[0].owner_scope_path, None);
}

#[test]
fn execute_tree_query_reports_owner_for_cpp_operator_method() {
    let source = "namespace math {\nclass Number {\npublic:\n    Number operator+(const Number& other) const { return *this; }\n};\n}\n";
    let captures =
        execute_tree_query(Path::new("number.cpp"), source, "(operator_name) @operator").unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].text, "operator+");
    assert_eq!(
        captures[0].owner_semantic_path.as_deref(),
        Some("math::Number::operator+")
    );
    assert_eq!(
        captures[0].owner_scope_path.as_deref(),
        Some("math::Number")
    );
}

#[test]
fn execute_tree_query_reports_owner_for_cpp_conversion_operator() {
    let source = "namespace config {\nclass Flag {\npublic:\n    explicit operator bool() const { return true; }\n};\n}\n";
    let captures =
        execute_tree_query(Path::new("flag.cpp"), source, "(operator_cast) @conversion").unwrap();

    assert_eq!(captures.len(), 1);
    assert_eq!(
        captures[0].owner_semantic_path.as_deref(),
        Some("config::Flag::operator bool")
    );
    assert_eq!(
        captures[0].owner_scope_path.as_deref(),
        Some("config::Flag")
    );
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
