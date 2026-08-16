use std::fs;
use std::path::Path;

use super::{
    Position, VirtualFileSystem, patch_ast_node, patch_ast_node_at_position,
    patch_ast_node_from_path, preview_patch_ast_node_from_path, temporary_dir,
};

const GO_SOURCE: &str =
    "package sample\n\nfunc Add(left int, right int) int {\n\treturn left + right\n}\n";

#[test]
fn patches_go_functions_by_semantic_path_and_position() {
    let replacement = "func Add(left int, right int) int {\n\treturn left + right + 1\n}";

    let semantic_target =
        patch_ast_node(Path::new("sample.go"), GO_SOURCE, "Add", replacement, None).unwrap();
    assert!(semantic_target.applied, "{semantic_target:#?}");
    assert!(semantic_target.validation.syntax_errors.is_empty());
    assert_eq!(semantic_target.resolved_path, "Add");
    assert_eq!(
        semantic_target.updated_source,
        format!("package sample\n\n{replacement}\n")
    );

    let position_target = patch_ast_node_at_position(
        Path::new("sample.go"),
        GO_SOURCE,
        &Position { row: 2, column: 6 },
        replacement,
        None,
    )
    .unwrap();
    assert!(position_target.applied, "{position_target:#?}");
    assert!(position_target.validation.syntax_errors.is_empty());
    assert_eq!(position_target.resolved_symbol_id, "Add");
    assert_eq!(
        position_target.updated_source,
        format!("package sample\n\n{replacement}\n")
    );
}

#[test]
fn patches_go_methods_preserving_doc_comments_and_impl_structure() {
    let source = r#"package sample

// Counter counts values.
type Counter struct {
	value int
}

func (counter *Counter) Value() int {
	return counter.value
}
"#;

    let method_replacement = "func (counter *Counter) Value() int {\n\treturn counter.value * 2\n}";
    let method_result = patch_ast_node(
        Path::new("counter.go"),
        source,
        "Counter::Value",
        method_replacement,
        None,
    )
    .unwrap();
    assert!(method_result.applied, "{method_result:#?}");
    assert!(method_result.validation.syntax_errors.is_empty());
    assert_eq!(method_result.resolved_symbol_id, "Counter::Value");
    assert!(
        method_result
            .updated_source
            .contains("// Counter counts values.")
    );
    assert!(
        method_result
            .updated_source
            .contains("type Counter struct {")
    );
    assert!(method_result.updated_source.contains("counter.value * 2"));
}

#[test]
fn patches_go_type_specs_and_aliases_with_and_without_the_type_keyword() {
    let source = r#"package sample

type Point struct {
	X int
}

type Alias = int
"#;

    let with_keyword = patch_ast_node(
        Path::new("types.go"),
        source,
        "Point",
        "type Point struct {\n\tX int\n\tY int\n}",
        None,
    )
    .unwrap();
    assert!(with_keyword.applied, "{with_keyword:#?}");
    assert!(with_keyword.validation.syntax_errors.is_empty());
    assert_eq!(with_keyword.resolved_symbol_id, "Point");
    assert!(
        with_keyword
            .updated_source
            .contains("type Point struct {\n\tX int\n\tY int\n}")
    );

    let without_keyword = patch_ast_node(
        Path::new("types.go"),
        source,
        "Point",
        "Point struct {\n\tX int\n\tZ int\n}",
        None,
    )
    .unwrap();
    assert!(without_keyword.applied, "{without_keyword:#?}");
    assert!(without_keyword.validation.syntax_errors.is_empty());
    assert!(
        without_keyword
            .updated_source
            .contains("type Point struct {\n\tX int\n\tZ int\n}")
    );

    let alias_result = patch_ast_node(
        Path::new("types.go"),
        source,
        "Alias",
        "type Alias = string",
        None,
    )
    .unwrap();
    assert!(alias_result.applied, "{alias_result:#?}");
    assert_eq!(alias_result.resolved_symbol_id, "Alias");
    assert!(alias_result.updated_source.contains("type Alias = string"));
}

#[test]
fn patches_one_spec_inside_a_grouped_go_type_declaration() {
    let source = "package sample\n\ntype (\n\tA int\n\tB string\n)\n";
    let result = patch_ast_node(Path::new("grouped.go"), source, "A", "A uint", None).unwrap();
    assert!(result.applied, "{result:#?}");
    assert!(result.validation.syntax_errors.is_empty());
    assert_eq!(result.resolved_symbol_id, "A");
    assert_eq!(
        result.updated_source,
        "package sample\n\ntype (\n\tA uint\n\tB string\n)\n"
    );
}

#[test]
fn previews_go_patch_success_and_rejection_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("sample.go");
    fs::write(&path, GO_SOURCE).unwrap();

    let replacement = "func Add(left int, right int) int {\n\treturn left + right + 1\n}";
    let preview = preview_patch_ast_node_from_path(&path, "Add", replacement, None).unwrap();
    assert!(preview.patch.applied, "{preview:#?}");
    assert!(preview.changed);
    assert!(preview.unified_diff.contains("-	return left + right"));
    assert!(preview.unified_diff.contains("+	return left + right + 1"));
    assert_eq!(fs::read_to_string(&path).unwrap(), GO_SOURCE);

    let rejected = preview_patch_ast_node_from_path(
        &path,
        "Add",
        "func Add(left int, right int) int {\n\treturn left + right\n",
        None,
    )
    .unwrap();
    assert!(!rejected.patch.applied);
    assert!(!rejected.patch.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), GO_SOURCE);
}

#[test]
fn patches_dirty_go_virtual_source_without_writing_disk() {
    let dir = temporary_dir();
    let path = dir.join("sample.go");
    fs::write(&path, GO_SOURCE).unwrap();

    let overlay_source =
        "package sample\n\nfunc Add(left int, right int) int {\n\treturn left + right + 2\n}\n";
    let replacement = "func Add(left int, right int) int {\n\treturn left + right + 3\n}";
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&path, Some(overlay_source)).unwrap();

    let result = vfs
        .patch_node_at_position(&path, &Position { row: 2, column: 6 }, replacement, None)
        .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(
        result.updated_source,
        format!("package sample\n\n{replacement}\n")
    );
    let snapshot = vfs.read_file(&path).unwrap();
    assert!(snapshot.dirty);
    assert_eq!(
        snapshot.source,
        format!("package sample\n\n{replacement}\n")
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), GO_SOURCE);
}

#[test]
fn rejects_invalid_go_replacements_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("sample.go");
    fs::write(&path, GO_SOURCE).unwrap();

    let result = patch_ast_node_from_path(
        &path,
        "Add",
        "func Add(left int, right int) int {\n\treturn left + right\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(!result.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), GO_SOURCE);
}
