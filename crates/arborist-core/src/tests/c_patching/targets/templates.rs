use super::*;

#[test]
fn patches_cpp_explicit_function_template_instantiation() {
    let dir = temporary_dir();
    let file = dir.join("instantiations.cpp");
    fs::write(
        &file,
        "namespace api {\ntemplate <typename T> T increment(T value) { return value; }\n}\n\ntemplate int api::increment<int>(int);\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "api::increment<int>",
        "template double api::increment<double>(double);",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::increment<double>");
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("api::increment<double>")
    );
}

#[test]
fn patches_cpp_explicit_class_template_specialization_method() {
    let dir = temporary_dir();
    let file = dir.join("templates.cpp");
    fs::write(
        &file,
        "template <typename T>\nclass Box {\npublic:\n    T value() { return T{}; }\n};\n\ntemplate <>\nclass Box<int> {\npublic:\n    int value() { return 1; }\n};\n",
    )
    .unwrap();

    let result =
        patch_ast_node_from_path(&file, "Box<int>::value", "int value() { return 2; }", None)
            .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "Box<int>::value");
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("int value() { return 2; }")
    );
}

#[test]
fn validates_non_type_cpp_template_parameters_as_local_bindings() {
    let dir = temporary_dir();
    let file = dir.join("templates.cpp");
    fs::write(
        &file,
        "template <int Offset>\nint adjust(int value) { return value; }\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "adjust",
        "int adjust(int value) { return value + Offset; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(result.validation.binding_decisions.is_empty());
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("return value + Offset;")
    );
}

#[test]
fn validates_defaulted_and_variadic_non_type_cpp_template_parameters_as_local_bindings() {
    let dir = temporary_dir();
    let file = dir.join("templates.cpp");
    fs::write(
        &file,
        "template <int Offset = 1>\nint adjust(int value) { return value; }\n\ntemplate <int... Offsets>\nint count() { return 0; }\n",
    )
    .unwrap();

    let defaulted = patch_ast_node_from_path(
        &file,
        "adjust",
        "int adjust(int value) { return value + Offset; }",
        None,
    )
    .unwrap();
    assert!(defaulted.applied);
    assert!(defaulted.validation.unresolved_identifiers.is_empty());

    let variadic = patch_ast_node_from_path(
        &file,
        "count",
        "int count() { return sizeof...(Offsets); }",
        None,
    )
    .unwrap();
    assert!(variadic.applied);
    assert!(variadic.validation.unresolved_identifiers.is_empty());
}
