use super::*;

#[test]
fn expands_selected_c_function_definitions() {
    let source = r#"
typedef struct item {
    int value;
} item;

int helper(int value) {
    return value + 1;
}
"#;

    let skeleton =
        get_semantic_skeleton(Path::new("sample.c"), source, 1, &["helper".to_string()]).unwrap();

    assert!(skeleton.skeleton.contains("typedef struct item"));
    assert!(
        skeleton
            .skeleton
            .contains("int helper(int value) {\n    return value + 1;\n}")
    );
    assert_eq!(skeleton.available_symbols.len(), 2);
    assert_eq!(skeleton.available_symbols[1].semantic_path, "helper");
    assert_eq!(skeleton.available_symbols[1].scope_path, None);
    assert_eq!(
        skeleton.available_symbols[1].node_kind,
        "function_definition"
    );
    assert_eq!(
        skeleton.available_symbols[1].signature.as_deref(),
        Some("int helper(int value);")
    );
    assert_eq!(
        skeleton.available_symbols[1].parameters,
        vec!["int value".to_string()]
    );
    assert_eq!(
        skeleton.available_symbols[1].return_type.as_deref(),
        Some("int")
    );
    assert_eq!(skeleton.available_symbols[1].docstring, None);
}

#[test]
fn expands_c_function_definition_by_precise_symbol_id() {
    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source = dir.join("helper.c");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let precise_symbol_id = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "helper")
        .map(|symbol| symbol.symbol_id.clone())
        .unwrap();

    let expanded = get_semantic_skeleton(&source, &source_text, 1, &[precise_symbol_id]).unwrap();

    assert!(
        expanded
            .skeleton
            .contains("int helper(int value) {\n    return value + 1;\n}")
    );
}

#[test]
fn anchors_c_source_symbol_ids_to_uppercase_sibling_header() {
    let dir = temporary_dir();
    let header = dir.join("helper.H");
    let source = dir.join("helper.C");

    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source,
        "int helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let symbol = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "helper")
        .unwrap();

    assert_eq!(
        symbol.symbol_id,
        format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
    );
}
