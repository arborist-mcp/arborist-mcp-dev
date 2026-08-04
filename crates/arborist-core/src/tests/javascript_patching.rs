use std::fs;
use std::path::Path;

use super::{
    Position, patch_ast_node, patch_ast_node_at_position, patch_ast_node_from_path, temporary_dir,
};

const TYPESCRIPT_SOURCE: &str =
    "export function helper(value: number): number { return value + 1; }\n";

#[test]
fn patches_typescript_symbols_by_semantic_path_and_position() {
    let replacement = "export function helper(value: number): number { return value + 2; }";

    let semantic_target = patch_ast_node(
        Path::new("sample.ts"),
        TYPESCRIPT_SOURCE,
        "helper",
        replacement,
        None,
    )
    .unwrap();
    assert!(semantic_target.applied, "{semantic_target:#?}");
    assert!(semantic_target.validation.syntax_errors.is_empty());
    assert_eq!(semantic_target.resolved_path, "helper");
    assert_eq!(semantic_target.updated_source, format!("{replacement}\n"));

    let position_target = patch_ast_node_at_position(
        Path::new("sample.ts"),
        TYPESCRIPT_SOURCE,
        &Position { row: 0, column: 16 },
        replacement,
        None,
    )
    .unwrap();
    assert!(position_target.applied, "{position_target:#?}");
    assert!(position_target.validation.syntax_errors.is_empty());
    assert_eq!(position_target.resolved_symbol_id, "helper");
    assert_eq!(position_target.updated_source, format!("{replacement}\n"));
}

#[test]
fn patches_exported_javascript_callable_variables_and_tsx_functions() {
    let javascript_source = "export const helper = (value) => value + 1;\n";
    let javascript_replacement = "export const helper = (value) => value + 2;";
    let javascript_result = patch_ast_node(
        Path::new("sample.js"),
        javascript_source,
        "helper",
        javascript_replacement,
        None,
    )
    .unwrap();
    assert!(javascript_result.applied, "{javascript_result:#?}");
    assert_eq!(
        javascript_result.updated_source,
        format!("{javascript_replacement}\n")
    );

    let tsx_source = "export function App() { return <main>ready</main>; }\n";
    let tsx_replacement = "export function App() { return <main>updated</main>; }";
    let tsx_result = patch_ast_node(
        Path::new("sample.tsx"),
        tsx_source,
        "App",
        tsx_replacement,
        None,
    )
    .unwrap();
    assert!(tsx_result.applied, "{tsx_result:#?}");
    assert_eq!(tsx_result.resolved_symbol_id, "App");
    assert_eq!(tsx_result.updated_source, format!("{tsx_replacement}\n"));
}

#[test]
fn rejects_invalid_typescript_replacements_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("sample.ts");
    fs::write(&path, TYPESCRIPT_SOURCE).unwrap();

    let result = patch_ast_node_from_path(
        &path,
        "helper",
        "export function helper(value: number): number { return value + 2;\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(!result.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), TYPESCRIPT_SOURCE);
}
