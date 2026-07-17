use super::*;

#[test]
fn allows_c_patch_when_symbol_is_declared_in_included_header() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &caller,
        "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(result.validation.commit_gate.allowed);
    assert_eq!(result.validation.commit_gate.status, "allowed");
    assert_eq!(
        result.validation.commit_gate.reason,
        "syntax and symbol binding validation passed"
    );
    assert_eq!(result.validation.commit_gate.syntax_error_count, 0);
    assert!(result.validation.commit_gate.blocking_decisions.is_empty());
    assert_eq!(result.validation.commit_gate.evidence_invariants.len(), 1);
    assert_eq!(
        result.validation.commit_gate.evidence_invariants[0].status,
        "passed"
    );
    assert_eq!(result.validation.ambiguous_identifiers.len(), 0);
    assert_eq!(result.validation.resolved_identifiers.len(), 1);
    assert_eq!(result.validation.binding_decisions.len(), 1);
    assert_eq!(result.validation.binding_decisions[0].name, "helper");
    assert_eq!(result.validation.binding_decisions[0].status, "resolved");
    assert_eq!(result.validation.resolved_identifiers[0].name, "helper");
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );
    assert_eq!(
        result.validation.binding_decisions[0]
            .selected_symbol_id
            .as_deref(),
        Some(
            result.validation.resolved_identifiers[0]
                .symbol
                .symbol_id
                .as_str()
        )
    );
    assert_eq!(result.validation.binding_decisions[0].candidates.len(), 1);
    assert_eq!(
        result.validation.commit_gate.evidence_invariants[0]
            .selected_evidence_key
            .as_deref(),
        Some(
            result.validation.binding_decisions[0].candidates[0]
                .evidence_key
                .as_str()
        )
    );
    let header_text = fs::read_to_string(&header).unwrap();
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.node_kind,
        "declaration"
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.origin_type,
        "include_header"
    );
    assert!(
        result.validation.resolved_identifiers[0]
            .symbol
            .evidence_key
            .contains("declaration|include_header")
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.byte_range,
        (0, header_text.find(';').map(|index| index + 1).unwrap())
    );
    assert_eq!(
        result.validation.resolved_identifiers[0]
            .symbol
            .signature
            .as_deref(),
        Some("int helper(int value);")
    );
    let updated = fs::read_to_string(&caller).unwrap();
    assert!(updated.contains("return helper(value);"));
}

#[test]
fn allows_c_patch_with_uppercase_header_companion_source() {
    let dir = temporary_dir();
    let header = dir.join("helper.H");
    let source = dir.join("helper.C");
    let caller = dir.join("caller.C");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.H\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.H\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert_eq!(result.validation.resolved_identifiers.len(), 1);
    assert_eq!(result.validation.binding_decisions.len(), 1);
    assert_eq!(result.validation.resolved_identifiers[0].name, "helper");
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.node_kind,
        "function_definition"
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.origin_type,
        "companion_source"
    );
    assert!(result.validation.commit_gate.allowed);

    let updated = fs::read_to_string(&caller).unwrap();
    assert!(updated.contains("return helper(value);"));
}

#[test]
fn allows_c_patch_with_hpp_header_companion_source() {
    let dir = temporary_dir();
    let header = dir.join("helper.HPP");
    let source = dir.join("helper.c");
    let caller = dir.join("caller.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.HPP\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"helper.HPP\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert_eq!(result.validation.resolved_identifiers.len(), 1);
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        result.validation.resolved_identifiers[0].symbol.origin_type,
        "companion_source"
    );
    assert!(result.validation.commit_gate.allowed);
}

#[test]
fn patches_c_definition_when_declaration_and_definition_share_name() {
    let dir = temporary_dir();
    let file = dir.join("helper.c");

    fs::write(
        &file,
        "int helper(int value);\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "helper",
        "int helper(int value) {\n    return value + 9;\n}\n",
        None,
    )
    .unwrap();

    let updated = fs::read_to_string(&file).unwrap();
    assert!(result.applied);
    assert_eq!(result.resolved_path, "helper");
    assert_eq!(
        result.resolved_symbol_id,
        format!("{}::helper", file.to_string_lossy().replace('\\', "/"))
    );
    assert!(updated.starts_with("int helper(int value);\n\n"));
    assert!(updated.contains("int helper(int value) {\n    return value + 9;\n}"));
    assert!(updated.contains("return value + 9;"));
    assert_eq!(updated.matches("int helper(int value);").count(), 1);
}

#[test]
fn allows_c_patch_targeting_precise_symbol_id() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let symbol_id = format!("{}::helper", header.to_string_lossy().replace('\\', "/"));
    let result = patch_ast_node_from_path(
        &source,
        &symbol_id,
        "int helper(int value) {\n    return value + 5;\n}\n",
        None,
    )
    .unwrap();

    let updated = fs::read_to_string(&source).unwrap();
    assert!(result.applied);
    assert_eq!(result.target_path, symbol_id);
    assert_eq!(result.resolved_path, "helper");
    assert_eq!(result.resolved_symbol_id, result.target_path);
    assert!(updated.contains("return value + 5;"));
}

#[test]
fn patches_cpp_function_targeted_by_nested_namespace_path() {
    let dir = temporary_dir();
    let file = dir.join("api.cpp");

    fs::write(
        &file,
        "namespace alpha::detail {\nint helper(int value) { return value + 1; }\n\nint orchestrate(int value) { return helper(value); }\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "alpha::detail::orchestrate",
        "int orchestrate(int value) { return helper(value) + 2; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "alpha::detail::orchestrate");
    assert_eq!(result.resolved_symbol_id, "alpha::detail::orchestrate");
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("return helper(value) + 2;")
    );
}

#[test]
fn patches_cpp_class_method_targeted_by_qualified_path() {
    let dir = temporary_dir();
    let file = dir.join("counter.cpp");
    fs::write(
        &file,
        "namespace api {\nclass Counter {\npublic:\n    int increment(int value) { return value + 1; }\n};\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "api::Counter::increment",
        "int increment(int value) { return value + 2; }",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert_eq!(result.resolved_path, "api::Counter::increment");
    assert_eq!(result.resolved_symbol_id, "api::Counter::increment");
    assert!(result.validation.commit_gate.allowed);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("return value + 2;")
    );
}

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
    assert_eq!(result.resolved_symbol_id, "api::Counter::increment");
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
    assert_eq!(result.resolved_symbol_id, "api::Counter::~Counter");
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

#[test]
fn reports_ambiguous_c_identifier_bindings() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let caller = dir.join("caller.c");

    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"alpha.h\"\n#include \"zeta.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(result.validation.resolved_identifiers.is_empty());
    assert!(!result.validation.commit_gate.allowed);
    assert_eq!(result.validation.commit_gate.status, "rejected");
    assert_eq!(
        result.validation.commit_gate.reason,
        "symbol binding is ambiguous"
    );
    assert_eq!(result.validation.commit_gate.syntax_error_count, 0);
    assert_eq!(result.validation.commit_gate.blocking_decisions.len(), 1);
    assert_eq!(
        result.validation.commit_gate.blocking_decisions[0].status,
        "ambiguous"
    );
    assert_eq!(result.validation.commit_gate.evidence_invariants.len(), 1);
    assert_eq!(
        result.validation.commit_gate.evidence_invariants[0].status,
        "blocked"
    );
    assert_eq!(
        result.validation.commit_gate.evidence_invariants[0]
            .candidate_evidence_keys
            .len(),
        2
    );
    assert_eq!(result.validation.ambiguous_identifiers.len(), 1);
    assert_eq!(result.validation.ambiguous_identifiers[0].name, "helper");
    assert_eq!(result.validation.binding_decisions.len(), 1);
    assert_eq!(result.validation.binding_decisions[0].name, "helper");
    assert_eq!(result.validation.binding_decisions[0].status, "ambiguous");
    assert_eq!(
        result.validation.binding_decisions[0].selected_symbol_id,
        None
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates.len(),
        2
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].reason,
        "multiple equally-ranked definitions across include families"
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .active_include_family,
        None
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .preferred_family,
        None
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .visible_include_families,
        vec![
            alpha_header.to_string_lossy().replace('\\', "/"),
            zeta_header.to_string_lossy().replace('\\', "/")
        ]
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .candidate_include_families,
        vec![
            alpha_header.to_string_lossy().replace('\\', "/"),
            zeta_header.to_string_lossy().replace('\\', "/")
        ]
    );
    assert_eq!(
        result.validation.binding_decisions[0].reason,
        result.validation.ambiguous_identifiers[0].reason
    );
    assert_eq!(result.validation.binding_decisions[0].candidates.len(), 2);
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0].symbol_id,
        format!(
            "{}::helper",
            alpha_header.to_string_lossy().replace('\\', "/")
        )
    );
    let alpha_source_text = fs::read_to_string(&alpha_source).unwrap();
    let alpha_start = alpha_source_text.find("int helper(int value) {").unwrap();
    let alpha_end = alpha_source_text.find('}').map(|index| index + 1).unwrap();
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0].node_kind,
        "function_definition"
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0].origin_type,
        "companion_source"
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0].evidence_key,
        result.validation.binding_decisions[0].candidates[0].evidence_key
    );
    assert!(
        result.validation.ambiguous_identifiers[0].candidates[0]
            .evidence_key
            .contains("function_definition|companion_source")
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0].byte_range,
        (alpha_start, alpha_end)
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[0]
            .signature
            .as_deref(),
        Some("int helper(int value);")
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[1].symbol_id,
        format!(
            "{}::helper",
            zeta_header.to_string_lossy().replace('\\', "/")
        )
    );
    let zeta_source_text = fs::read_to_string(&zeta_source).unwrap();
    let zeta_start = zeta_source_text.find("int helper(int value) {").unwrap();
    let zeta_end = zeta_source_text.find('}').map(|index| index + 1).unwrap();
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[1].node_kind,
        "function_definition"
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[1].origin_type,
        "companion_source"
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[1].byte_range,
        (zeta_start, zeta_end)
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates[1]
            .signature
            .as_deref(),
        Some("int helper(int value);")
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .candidate_symbol_ids,
        vec![
            format!(
                "{}::helper",
                alpha_header.to_string_lossy().replace('\\', "/")
            ),
            format!(
                "{}::helper",
                zeta_header.to_string_lossy().replace('\\', "/")
            )
        ]
    );
}

#[test]
fn allows_ambiguous_c_identifier_bindings_with_bypass() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let caller = dir.join("caller.c");

    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"alpha.h\"\n#include \"zeta.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        Some("runtime wiring guarantees the intended helper target"),
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.bypass_applied);
    assert!(result.validation.commit_gate.allowed);
    assert_eq!(result.validation.commit_gate.status, "allowed_with_bypass");
    assert_eq!(
        result.validation.commit_gate.bypass_reason.as_deref(),
        Some("runtime wiring guarantees the intended helper target")
    );
    assert_eq!(result.validation.commit_gate.blocking_decisions.len(), 1);
    assert_eq!(
        result.validation.commit_gate.evidence_invariants[0].status,
        "blocked"
    );
    assert_eq!(result.validation.ambiguous_identifiers.len(), 1);
    let updated = fs::read_to_string(&caller).unwrap();
    assert!(updated.contains("return helper(value);"));
}

#[test]
fn reports_transitive_visible_include_families_for_c_ambiguity() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let wrapper_header = dir.join("wrapper.h");
    let caller = dir.join("caller.c");

    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(
        &wrapper_header,
        "#include \"alpha.h\"\n#include \"zeta.h\"\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"wrapper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &caller,
        "orchestrate",
        "int orchestrate(int value) {\n    return helper(value);\n}\n",
        None,
    )
    .unwrap();

    assert_eq!(result.validation.ambiguous_identifiers.len(), 1);
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .visible_include_families,
        vec![
            wrapper_header.to_string_lossy().replace('\\', "/"),
            alpha_header.to_string_lossy().replace('\\', "/"),
            zeta_header.to_string_lossy().replace('\\', "/")
        ]
    );
    assert_eq!(
        result.validation.ambiguous_identifiers[0]
            .disambiguation_context
            .candidate_include_families,
        vec![
            alpha_header.to_string_lossy().replace('\\', "/"),
            zeta_header.to_string_lossy().replace('\\', "/")
        ]
    );
}
