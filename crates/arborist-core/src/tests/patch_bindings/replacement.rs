use super::*;

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
fn validates_python_crlf_replacement_bindings() {
    let source = "def helper(value: int) -> int:\r\n    return value + 1\r\n";

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "helper",
        "def helper(value: int) -> int:\n    return missing_helper(value)\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert_eq!(result.validation.commit_gate.status, "rejected");
    assert_eq!(
        result.validation.unresolved_identifiers,
        vec!["missing_helper"]
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "missing_helper" && decision.status == "unresolved")
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
