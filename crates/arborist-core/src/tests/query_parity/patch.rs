use super::*;

#[test]
fn patches_python_symbol_at_position_from_decorator_line() {
    let file = temporary_dir().join("helper.py");
    fs::write(
            &file,
            "def decorator(func):\n    return func\n\n@decorator\ndef helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let result = patch_ast_node_at_position(
        &file,
        &fs::read_to_string(&file).unwrap(),
        &Position { row: 3, column: 1 },
        "def helper(value: int) -> int:\n    return value + 2\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert_eq!(result.resolved_path, "helper");
    assert_eq!(result.resolved_symbol_id, "helper");
    assert!(
        result
            .validation
            .syntax_errors
            .iter()
            .any(|issue| issue.kind == "decorator_guard")
    );
    assert!(result.updated_source.contains("return value + 2"));
}

#[test]
fn patches_c_symbols_at_position_exactly() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let patched_declaration = patch_ast_node_at_position(
        &header,
        &fs::read_to_string(&header).unwrap(),
        &Position { row: 0, column: 4 },
        "long helper(long value);",
        None,
    )
    .unwrap();
    assert!(patched_declaration.applied);
    assert_eq!(patched_declaration.resolved_path, "helper");
    assert_eq!(
        patched_declaration.updated_source,
        "long helper(long value);\n"
    );

    let patched_definition = patch_ast_node_at_position(
        &source,
        &fs::read_to_string(&source).unwrap(),
        &Position { row: 2, column: 4 },
        "int helper(int value) {\n    return value + 2;\n}\n",
        None,
    )
    .unwrap();
    assert!(patched_definition.applied);
    assert_eq!(patched_definition.resolved_path, "helper");
    assert!(
        patched_definition
            .resolved_symbol_id
            .ends_with("helper.h::helper")
    );
    assert!(
        patched_definition
            .updated_source
            .contains("return value + 2;")
    );
    assert!(
        patched_definition
            .updated_source
            .contains("#include \"helper.h\"")
    );
}

#[test]
fn validates_patch_with_discovery_context_at_position_in_one_call() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let entry = dir.join("entry.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
    fs::write(
            &entry,
            "from caller import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
        )
        .unwrap();

    let result = validate_patch_with_discovery_context_at_position(
        &dir,
        &helper,
        "def helper(value: int) -> int:\n    return value + 2\n",
        &Position { row: 0, column: 5 },
        "def helper(value: int) -> int:\n    return value + 2\n",
        None,
        TraceDirection::Callers,
        2,
        10,
    )
    .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.trace_target, "helper");
    assert_eq!(result.patch.resolved_path, "helper");
    assert_eq!(
        result.trace.as_ref().unwrap().callers[0].semantic_path,
        "orchestrate"
    );
    assert_eq!(result.read.as_ref().unwrap().symbol.semantic_path, "helper");
    assert_eq!(result.neighborhood_context.as_ref().unwrap().reads.len(), 3);
    assert_eq!(
        result.neighborhood_context.as_ref().unwrap().reads[1]
            .symbol
            .semantic_path,
        "orchestrate"
    );
}

#[test]
fn validates_patch_with_trace_context_at_position_in_one_call() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let result = validate_patch_with_trace_context_at_position(
            &dir,
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
            &Position { row: 3, column: 5 },
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
            None,
            TraceDirection::Both,
        )
        .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.trace_target, "orchestrate");
    assert_eq!(result.patch.resolved_path, "orchestrate");
    assert_eq!(
        result.trace.as_ref().unwrap().symbol.semantic_path,
        "orchestrate"
    );
    assert_eq!(
        result.trace.as_ref().unwrap().callees[0].semantic_path,
        "helper"
    );
    assert!(result.trace_validation.as_ref().unwrap().allowed);
}
