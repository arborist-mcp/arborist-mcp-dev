use super::*;

#[test]
fn patches_cpp_inline_friend_function_targeted_by_namespace_path() {
    let dir = temporary_dir();
    let file = dir.join("token.hpp");
    fs::write(
        &file,
        "namespace api {\nclass Token {\n    friend int inspect(const Token&) { return 1; }\n};\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "api::inspect",
        "int inspect(const Token&) { return 2; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::inspect");
    assert!(result.validation.commit_gate.allowed);
    assert!(fs::read_to_string(&file).unwrap().contains("return 2"));
}

#[test]
fn patches_cpp_extern_c_function_targeted_by_path() {
    let dir = temporary_dir();
    let file = dir.join("bridge.cpp");
    fs::write(
        &file,
        "extern \"C\" int helper(int value) { return value + 1; }\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "helper",
        "int helper(int value) { return value + 2; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "helper");
    assert!(result.validation.commit_gate.allowed);
    assert!(fs::read_to_string(&file).unwrap().contains("value + 2"));
}

#[test]
fn patches_conditionally_compiled_cpp_class_method() {
    let dir = temporary_dir();
    let file = dir.join("config.hpp");
    fs::write(
        &file,
        "namespace api {\nclass Config {\n#if ENABLED\npublic:\n    int enabled() { return 1; }\n#endif\n};\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "api::Config::enabled",
        "int enabled() { return 2; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::Config::enabled");
    assert!(result.validation.commit_gate.allowed);
    assert!(fs::read_to_string(&file).unwrap().contains("return 2"));
}

#[test]
fn patches_cpp_using_alias_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("aliases.hpp");
    fs::write(&file, "namespace api {\nusing Size = unsigned long;\n}\n").unwrap();

    let result =
        patch_ast_node_from_path(&file, "api::Size", "using Size = unsigned int;", None).unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::Size");
    assert_eq!(result.resolved_symbol_id, "api::Size");
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("using Size = unsigned int;")
    );
}

#[test]
fn patches_cpp_using_declaration_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("imports.hpp");
    fs::write(
        &file,
        "namespace api {\nnamespace base { int convert(int value) { return value; } }\nusing base::convert;\n}\n",
    )
    .unwrap();

    let result =
        patch_ast_node_from_path(&file, "api::convert", "using base::convert_value;", None)
            .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::convert_value");
    assert_eq!(result.resolved_symbol_id, "api::convert_value");
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("using base::convert_value;")
    );
}

#[test]
fn patches_cpp_namespace_alias_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("aliases.hpp");
    fs::write(
        &file,
        "namespace api {\nnamespace detail {}\nnamespace alternate {}\nnamespace vendor = detail;\n}\n",
    )
    .unwrap();

    let result =
        patch_ast_node_from_path(&file, "api::vendor", "namespace vendor = alternate;", None)
            .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::vendor");
    assert_eq!(result.resolved_symbol_id, "api::vendor");
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("namespace vendor = alternate;")
    );
}

#[test]
fn patches_cpp_concept_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("concepts.hpp");
    fs::write(
        &file,
        "namespace api {\ntemplate <typename T>\nconcept Incrementable = requires(T value) { value + 1; };\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "api::Incrementable",
        "concept Incrementable = requires(T value) { value + 2; };",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::Incrementable");
    assert_eq!(result.resolved_symbol_id, "api::Incrementable");
    assert!(result.validation.commit_gate.allowed);
    assert!(fs::read_to_string(&file).unwrap().contains("value + 2"));
}

#[test]
fn patches_cpp_class_definition_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("config.hpp");
    fs::write(
        &file,
        "namespace api {\nclass Config {\npublic:\n    int value() const { return 1; }\n};\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "api::Config",
        "class Config {\npublic:\n    int value() const { return 2; }\n};",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::Config");
    assert_eq!(result.resolved_symbol_id, "api::Config");
    assert!(result.validation.commit_gate.allowed);
    assert!(fs::read_to_string(&file).unwrap().contains("return 2"));
}

#[test]
fn patches_named_c_struct_definition_targeted_by_path() {
    let dir = temporary_dir();
    let file = dir.join("packet.c");
    fs::write(&file, "struct Packet { int id; };\n").unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "Packet",
        "struct Packet { int id; int priority; };",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "Packet");
    assert!(result.validation.commit_gate.allowed);
    assert!(fs::read_to_string(&file).unwrap().contains("priority"));
}

#[test]
fn patches_cpp_enum_definition_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("status.hpp");
    fs::write(
        &file,
        "namespace api {\nenum class Status { idle, busy };\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "api::Status",
        "enum class Status { idle, busy, failed };",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::Status");
    assert_eq!(result.resolved_symbol_id, "api::Status");
    assert!(result.validation.commit_gate.allowed);
    assert!(fs::read_to_string(&file).unwrap().contains("failed"));
}

#[test]
fn patches_cpp_scoped_enum_member_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("status.hpp");
    fs::write(
        &file,
        "namespace api {\nenum class Status { idle, busy };\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(&file, "api::Status::idle", "idle = 7", None).unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::Status::idle");
    assert_eq!(result.resolved_symbol_id, "api::Status::idle");
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("idle = 7, busy")
    );
}

#[test]
fn patches_c_enum_member_targeted_by_path() {
    let dir = temporary_dir();
    let file = dir.join("status.c");
    fs::write(&file, "enum Status { STATUS_READY, STATUS_FAILED };\n").unwrap();

    let result = patch_ast_node_from_path(&file, "STATUS_READY", "STATUS_READY = 1", None).unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "STATUS_READY");
    assert_eq!(
        result.resolved_symbol_id,
        format!(
            "{}::STATUS_READY",
            file.to_string_lossy().replace('\\', "/")
        )
    );
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("STATUS_READY = 1, STATUS_FAILED")
    );
}

#[test]
fn rejects_bare_scoped_enum_member_during_patch_validation() {
    let dir = temporary_dir();
    let file = dir.join("status.cpp");
    fs::write(
        &file,
        "enum class Status { idle, busy };\n\nint status() { return 0; }\n",
    )
    .unwrap();

    let result =
        patch_ast_node_from_path(&file, "status", "int status() { return idle; }", None).unwrap();

    assert!(!result.applied);
    assert_eq!(result.validation.commit_gate.status, "rejected");
    assert_eq!(result.validation.unresolved_identifiers, vec!["idle"]);
}

#[test]
fn allows_bare_unscoped_enum_member_during_patch_validation() {
    let dir = temporary_dir();
    let file = dir.join("status.cpp");
    fs::write(
        &file,
        "enum Status { idle, busy };\n\nint status() { return 0; }\n",
    )
    .unwrap();

    let result =
        patch_ast_node_from_path(&file, "status", "int status() { return idle; }", None).unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn rejects_unresolved_cpp_qualified_calls_during_patch_validation() {
    let dir = temporary_dir();
    let file = dir.join("caller.cpp");
    fs::write(&file, "int caller() { return 0; }\n").unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "caller",
        "int caller() { return missing::convert(1); }",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert_eq!(result.validation.commit_gate.status, "rejected");
    assert!(
        result
            .validation
            .unresolved_identifiers
            .iter()
            .any(|identifier| identifier == "missing" || identifier == "convert")
    );
}

#[test]
fn patches_cpp_class_method_defined_outside_class() {
    let dir = temporary_dir();
    let file = dir.join("counter.cpp");
    fs::write(
        &file,
        "int api::Counter::increment(int value) { return value + 1; }\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "api::Counter::increment",
        "int api::Counter::increment(int value) { return value + 2; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::Counter::increment");
    assert_eq!(result.resolved_symbol_id, "api::Counter::increment(int)");
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("return value + 2;")
    );
}

#[test]
fn patches_cpp_destructor_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("counter.cpp");
    fs::write(&file, "api::Counter::~Counter() {}\n").unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "api::Counter::~Counter",
        "api::Counter::~Counter() { int value = 0; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::Counter::~Counter");
    assert_eq!(result.resolved_symbol_id, "api::Counter::~Counter()");
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("int value = 0;")
    );
}

#[test]
fn patches_defaulted_cpp_method_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("lifecycle.hpp");
    fs::write(
        &file,
        "namespace api {\nclass Defaulted {\npublic:\n    Defaulted() = default;\n};\n}\n",
    )
    .unwrap();

    let result =
        patch_ast_node_from_path(&file, "api::Defaulted::Defaulted", "Defaulted() {}", None)
            .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::Defaulted::Defaulted");
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("Defaulted() {}")
    );
}

#[test]
fn patches_cpp_operator_method_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("number.cpp");
    fs::write(
        &file,
        "namespace math {\nclass Number {\npublic:\n    Number operator+(const Number& other) const { return *this; }\n};\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "math::Number::operator+",
        "Number operator+(const Number& other) const { return *this; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "math::Number::operator+");
    assert!(result.validation.commit_gate.allowed);
}

#[test]
fn patches_cpp_overload_targeted_by_exact_symbol_id() {
    let dir = temporary_dir();
    let file = dir.join("convert.cpp");
    fs::write(
        &file,
        "namespace api {\nint convert(int value) { return value; }\ndouble convert(double value) { return value; }\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "api::convert(double)",
        "double convert(double value) { return value + 0.5; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::convert");
    assert_eq!(result.resolved_symbol_id, "api::convert(double)");
    let updated = fs::read_to_string(&file).unwrap();
    assert!(updated.contains("int convert(int value) { return value; }"));
    assert!(updated.contains("double convert(double value) { return value + 0.5; }"));
}

#[test]
fn patches_cpp_conversion_operator_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("flag.cpp");
    fs::write(
        &file,
        "namespace config {\nclass Flag {\npublic:\n    explicit operator bool() const { return true; }\n};\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "config::Flag::operator bool",
        "explicit operator bool() const { return false; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "config::Flag::operator bool");
    assert!(result.validation.commit_gate.allowed);
    assert!(fs::read_to_string(&file).unwrap().contains("return false;"));
}
