use std::fs;

use super::support::temporary_dir;
use super::{
    TraceDirection, execute_tree_query, execute_tree_query_from_path, get_semantic_skeleton,
    get_semantic_skeleton_from_path, patch_ast_node, patch_ast_node_from_path,
    rebuild_symbol_index, trace_symbol_graph_from_index,
};
#[test]
fn from_path_entrypoints_normalize_file_paths() {
    let dir = temporary_dir();
    let nested = dir.join("child");
    let python_file = dir.join("buffer.py");
    let c_file = dir.join("sample.c");

    fs::create_dir_all(&nested).unwrap();
    fs::write(&python_file, "def value() -> int:\n    return 1\n").unwrap();
    fs::write(
            &c_file,
            "int helper(int value) { return value + 1; }\nint orchestrate(int value) { return helper(value); }\n",
        )
        .unwrap();

    let python_alias = nested.join("..").join("buffer.py");
    let c_alias = nested.join("..").join("sample.c");

    let skeleton = get_semantic_skeleton_from_path(&python_alias, 1, &[]).unwrap();
    assert!(!skeleton.file.contains("/../"));

    let patch = patch_ast_node_from_path(
        &python_alias,
        "value",
        "def value() -> int:\n    return 2\n",
        None,
    )
    .unwrap();
    assert!(patch.applied);
    assert!(!patch.file.contains("/../"));
    assert!(
        fs::read_to_string(&python_file)
            .unwrap()
            .contains("return 2")
    );

    let captures =
        execute_tree_query_from_path(&c_alias, "(call_expression function: (identifier) @callee)")
            .unwrap();
    let owner_symbol_id = captures[0].owner_symbol_id.as_deref().unwrap();
    assert!(!owner_symbol_id.contains("/../"));
}

#[test]
fn source_entrypoints_normalize_file_paths() {
    let dir = temporary_dir();
    let nested = dir.join("child");
    let python_file = dir.join("buffer.py");
    let c_file = dir.join("sample.c");

    fs::create_dir_all(&nested).unwrap();
    let python_alias = nested.join("..").join("buffer.py");
    let c_alias = nested.join("..").join("sample.c");

    let skeleton =
        get_semantic_skeleton(&python_alias, "def value() -> int:\n    return 1\n", 1, &[])
            .unwrap();
    assert_eq!(
        skeleton.file,
        python_file.to_string_lossy().replace('\\', "/")
    );

    let patch = patch_ast_node(
        &python_alias,
        "def value() -> int:\n    return 1\n",
        "value",
        "def value() -> int:\n    return 2\n",
        None,
    )
    .unwrap();
    assert_eq!(patch.file, python_file.to_string_lossy().replace('\\', "/"));

    let captures = execute_tree_query(
        &c_alias,
        "int orchestrate(int value) { return value + 1; }\n",
        "(function_definition declarator: (function_declarator declarator: (identifier) @name))",
    )
    .unwrap();
    let owner_symbol_id = captures[0].owner_symbol_id.as_deref().unwrap();
    assert_eq!(
        owner_symbol_id,
        format!(
            "{}::orchestrate",
            c_file.to_string_lossy().replace('\\', "/")
        )
    );
}

#[test]
fn from_path_entrypoints_accept_case_insensitive_extensions() {
    let dir = temporary_dir();
    let python_file = dir.join("Buffer.PY");
    let c_file = dir.join("Sample.C");
    let header_file = dir.join("Header.HH");
    let db_path = dir.join("symbols.db");

    fs::write(&python_file, "def py_value() -> int:\n    return 1\n").unwrap();
    fs::write(&c_file, "int c_value(void) { return 2; }\n").unwrap();
    fs::write(&header_file, "int hh_value(void);\n").unwrap();

    let skeleton = get_semantic_skeleton_from_path(&python_file, 1, &[]).unwrap();
    assert_eq!(skeleton.available_paths, vec!["py_value"]);

    let header_skeleton = get_semantic_skeleton_from_path(&header_file, 1, &[]).unwrap();
    assert_eq!(header_skeleton.available_paths, vec!["hh_value"]);

    let captures = execute_tree_query_from_path(
        &c_file,
        "(function_definition declarator: (function_declarator declarator: (identifier) @name))",
    )
    .unwrap();
    assert_eq!(captures[0].text, "c_value");

    let header_captures = execute_tree_query_from_path(
        &header_file,
        "(declaration declarator: (function_declarator declarator: (identifier) @name))",
    )
    .unwrap();
    assert_eq!(header_captures[0].text, "hh_value");

    let stats = rebuild_symbol_index(&dir, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert!(trace_symbol_graph_from_index(&db_path, "py_value", TraceDirection::Both).is_ok());
    assert!(trace_symbol_graph_from_index(&db_path, "c_value", TraceDirection::Both).is_ok());
    assert!(trace_symbol_graph_from_index(&db_path, "hh_value", TraceDirection::Both).is_ok());
}
