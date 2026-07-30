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
fn reindents_python_decorated_method_replacements_from_expanded_source() {
    let source = r#"
class Product:
    @staticmethod
    def normalize(value: int) -> int:
        return value
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        "@staticmethod\n    def normalize(value: int) -> int:\n        return value + 1\n",
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
    assert!(
        result.updated_source.contains(
            "class Product:\n    @staticmethod\n    def normalize(value: int) -> int:\n        return value + 1\n"
        ),
        "{updated_source:?}",
        updated_source = result.updated_source
    );
    assert!(!result.updated_source.contains("        def normalize"));
    assert!(
        !result
            .updated_source
            .contains("            return value + 1")
    );
}

#[test]
fn reindents_python_decorated_method_replacements_from_file_indentation() {
    let source = r#"
class Product:
    @staticmethod
    def normalize(value: int) -> int:
        return value
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        "    @staticmethod
    def normalize(value: int) -> int:
        return value + 1
",
        None,
    )
    .unwrap();

    assert!(
        result.applied,
        "{updated_source:?}
{validation:#?}",
        updated_source = result.updated_source,
        validation = result.validation
    );
    assert!(result.validation.syntax_errors.is_empty());
    assert!(result.updated_source.contains(
        "class Product:
    @staticmethod
    def normalize(value: int) -> int:
        return value + 1
"
    ));
}

#[test]
fn preserves_relative_indentation_for_multiline_decorators() {
    let source = r#"
class Product:
    @decorator(
        value=1,
    )
    def normalize(self) -> int:
        return 1
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        "@decorator(
    value=2,
)
def normalize(self) -> int:
    return 2
",
        None,
    )
    .unwrap();

    assert!(
        result.applied,
        "{updated_source:?}
{validation:#?}",
        updated_source = result.updated_source,
        validation = result.validation
    );
    assert!(result.validation.syntax_errors.is_empty());
    assert!(result.updated_source.contains(
        "class Product:
    @decorator(
        value=2,
    )
    def normalize(self) -> int:
        return 2
"
    ));
}

#[test]
fn preserves_relative_decorator_strings_containing_definition_text() {
    let source = r#"
def decorator(value):
    return lambda function: function

class Product:
    @decorator("original")
    def normalize(self) -> int:
        return 1
"#;
    let replacement = r#"@decorator("""
    def example
""")
def normalize(self) -> int:
    return 2
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        replacement,
        None,
    )
    .unwrap();

    assert!(
        result.applied,
        "{updated_source:?}
{validation:#?}",
        updated_source = result.updated_source,
        validation = result.validation
    );
    assert!(result.validation.syntax_errors.is_empty());
    assert!(result.updated_source.contains(
        r#"    @decorator("""
    def example
""")
    def normalize(self) -> int:
        return 2
"#
    ));
}

#[test]
fn rejects_decorated_method_definition_indented_beyond_decorator() {
    let source = r#"
class Product:
    @staticmethod
    def normalize(value: int) -> int:
        return value
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        "@staticmethod
        def normalize(value: int) -> int:
            return value + 1
",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert_eq!(result.validation.commit_gate.status, "rejected");
    assert!(!result.validation.commit_gate.allowed);
    assert!(result.validation.syntax_errors.iter().any(|issue| {
        issue.kind == "indentation" && issue.message.contains("definition after decorator")
    }));
}

#[test]
fn reindents_multiple_decorators_from_expanded_source() {
    let source = r#"
def decorator(function):
    return function

class Product:
    @staticmethod
    @decorator
    def normalize(value: int) -> int:
        return value
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        "@staticmethod
    @decorator
    def normalize(value: int) -> int:
        return value + 1
",
        None,
    )
    .unwrap();

    assert!(
        result.applied,
        "{updated_source:?}
{validation:#?}",
        updated_source = result.updated_source,
        validation = result.validation
    );
    assert!(result.updated_source.contains(
        "    @staticmethod
    @decorator
    def normalize(value: int) -> int:
        return value + 1
"
    ));
}

#[test]
fn preserves_hanging_multiline_decorator_alignment() {
    let source = r#"
def decorator(**kwargs):
    return lambda function: function

class Product:
    @decorator(value=1,
               mode=2)
    def normalize(self) -> int:
        return 1
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        "@decorator(value=2,\n               mode=3)\n    def normalize(self) -> int:\n        return 2\n",
        None,
    )
    .unwrap();

    assert!(
        result.applied,
        "{updated_source:?}\n{validation:#?}",
        updated_source = result.updated_source,
        validation = result.validation
    );
    assert!(
        result.updated_source.contains(
            "    @decorator(value=2,\n               mode=3)\n    def normalize(self) -> int:\n        return 2\n"
        ),
        "{:?}",
        result.updated_source
    );
}

#[test]
fn preserves_decorator_continuation_alignment_when_converting_indent_units() {
    let source = "def decorator(**kwargs):\n\treturn lambda function: function\n\nclass Product:\n\t@decorator(value=1,\n\t           mode=2)\n\tdef normalize(self) -> int:\n\t\treturn 1\n";

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        "@decorator(value=2,\n           mode=3)\ndef normalize(self) -> int:\n    return 2\n",
        None,
    )
    .unwrap();

    assert!(
        result.applied,
        "{updated_source:?}\n{validation:#?}",
        updated_source = result.updated_source,
        validation = result.validation
    );
    assert!(
        result.updated_source.contains(
            "\t@decorator(value=2,\n\t           mode=3)\n\tdef normalize(self) -> int:\n\t\treturn 2\n"
        ),
        "{:?}",
        result.updated_source
    );
}

#[test]
fn preserves_multiline_string_indentation_in_decorated_replacement() {
    let source = r#"
class Product:
    @staticmethod
    def normalize() -> str:
        return "original"
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        "@staticmethod\ndef normalize() -> str:\n    return \"\"\"alpha\n      beta\n    \"\"\"\n",
        None,
    )
    .unwrap();

    assert!(
        result.applied,
        "{updated_source:?}\n{validation:#?}",
        updated_source = result.updated_source,
        validation = result.validation
    );
    assert!(result.updated_source.contains(
        "    @staticmethod\n    def normalize() -> str:\n        return \"\"\"alpha\n      beta\n    \"\"\"\n"
    ));
}

#[test]
fn rejects_partially_indented_multiple_decorators() {
    let source = r#"
def decorator(function):
    return function

class Product:
    @staticmethod
    @decorator
    def normalize(value: int) -> int:
        return value
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        "@staticmethod\n@decorator\n    def normalize(value: int) -> int:\n        return value + 1\n",
        None,
    )
    .unwrap();

    assert!(!result.applied, "{:#?}", result.validation);
    assert_eq!(result.validation.commit_gate.status, "rejected");
    assert!(!result.validation.commit_gate.allowed);
    assert!(result.validation.syntax_errors.iter().any(|issue| {
        issue.kind == "indentation" && issue.message.contains("definition after decorator")
    }));
}

#[test]
fn reindents_tab_decorated_method_from_expanded_source() {
    let source =
        "class Product:\n\t@staticmethod\n\tdef normalize(value: int) -> int:\n\t\treturn value\n";

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        "@staticmethod\n\tdef normalize(value: int) -> int:\n\t\treturn value + 1\n",
        None,
    )
    .unwrap();

    assert!(
        result.applied,
        "{updated_source:?}\n{validation:#?}",
        updated_source = result.updated_source,
        validation = result.validation
    );
    assert!(result.updated_source.contains(
        "class Product:\n\t@staticmethod\n\tdef normalize(value: int) -> int:\n\t\treturn value + 1\n"
    ));
    assert!(!result.updated_source.contains("\t    "));
}

#[test]
fn reindents_crlf_decorated_method_without_mixing_line_endings() {
    let source = "class Product:\r\n    @staticmethod\r\n    def normalize(value: int) -> int:\r\n        return value\r\n";

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "Product.normalize",
        "@staticmethod\n    def normalize(value: int) -> int:\n        return value + 1\n",
        None,
    )
    .unwrap();

    assert!(
        result.applied,
        "{updated_source:?}\n{validation:#?}",
        updated_source = result.updated_source,
        validation = result.validation
    );
    assert!(result.updated_source.contains(
        "    @staticmethod\r\n    def normalize(value: int) -> int:\r\n        return value + 1\r\n"
    ));
    assert!(!result.updated_source.replace("\r\n", "").contains('\n'));
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
    assert!(
        result.updated_source.contains(
            "class Product:\n\tdef price_with_tax(self, rate: float) -> float:\n\t\treturn self.price * rate\n"
        ),
        "{:?}",
        result.updated_source
    );
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
