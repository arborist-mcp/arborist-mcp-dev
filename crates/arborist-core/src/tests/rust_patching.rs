use std::fs;
use std::path::Path;

use super::{
    Position, VirtualFileSystem, patch_ast_node, patch_ast_node_at_position,
    patch_ast_node_from_path, preview_patch_ast_node_from_path, temporary_dir,
};

const RUST_SOURCE: &str = "pub fn compute(value: i32) -> i32 {\n    value + 1\n}\n";

#[test]
fn patches_rust_functions_by_semantic_path_and_position() {
    let replacement = "pub fn compute(value: i32) -> i32 {\n    value + 2\n}";

    let semantic_target = patch_ast_node(
        Path::new("sample.rs"),
        RUST_SOURCE,
        "compute",
        replacement,
        None,
    )
    .unwrap();
    assert!(semantic_target.applied, "{semantic_target:#?}");
    assert!(semantic_target.validation.syntax_errors.is_empty());
    assert_eq!(semantic_target.resolved_path, "compute");
    assert_eq!(semantic_target.updated_source, format!("{replacement}\n"));

    let position_target = patch_ast_node_at_position(
        Path::new("sample.rs"),
        RUST_SOURCE,
        &Position { row: 0, column: 10 },
        replacement,
        None,
    )
    .unwrap();
    assert!(position_target.applied, "{position_target:#?}");
    assert!(position_target.validation.syntax_errors.is_empty());
    assert_eq!(position_target.resolved_symbol_id, "compute");
    assert_eq!(position_target.updated_source, format!("{replacement}\n"));
}

#[test]
fn patches_rust_items_and_methods_preserving_attributes_and_impl_wrappers() {
    let source = r#"#[derive(Debug)]
pub struct Point {
    x: i32,
}

impl Point {
    pub fn x_squared(&self) -> i32 {
        self.x * self.x
    }
}
"#;

    let struct_replacement = "#[derive(Debug)]\npub struct Point {\n    x: i32,\n    y: i32,\n}";
    let struct_result = patch_ast_node(
        Path::new("point.rs"),
        source,
        "Point",
        struct_replacement,
        None,
    )
    .unwrap();
    assert!(struct_result.applied, "{struct_result:#?}");
    assert!(struct_result.validation.syntax_errors.is_empty());
    assert_eq!(struct_result.resolved_symbol_id, "Point");
    assert!(
        struct_result
            .updated_source
            .starts_with("#[derive(Debug)]\n")
    );
    assert!(struct_result.updated_source.contains("\nimpl Point {\n"));

    let method_replacement = "pub fn x_squared(&self) -> i32 {\n        self.x * self.x * 2\n    }";
    let method_result = patch_ast_node(
        Path::new("point.rs"),
        source,
        "Point::x_squared",
        method_replacement,
        None,
    )
    .unwrap();
    assert!(method_result.applied, "{method_result:#?}");
    assert!(method_result.validation.syntax_errors.is_empty());
    assert_eq!(method_result.resolved_symbol_id, "Point::x_squared");
    assert!(method_result.updated_source.contains("impl Point {\n"));
    assert!(method_result.updated_source.contains("self.x * self.x * 2"));
}

#[test]
fn patches_rust_const_static_enum_type_and_trait_items() {
    let source = r#"pub const LIMIT: usize = 10;
pub static NAME: &str = "arborist";
pub enum Kind {
    Alpha,
    Beta,
}
pub type Alias = i64;
pub trait Render {
    fn render(&self) -> String;
}
"#;

    let const_result = patch_ast_node(
        Path::new("sample.rs"),
        source,
        "LIMIT",
        "pub const LIMIT: usize = 20;",
        None,
    )
    .unwrap();
    assert!(const_result.applied, "{const_result:#?}");
    assert_eq!(const_result.resolved_symbol_id, "LIMIT");

    let static_result = patch_ast_node(
        Path::new("sample.rs"),
        source,
        "NAME",
        r#"pub static NAME: &str = "updated";"#,
        None,
    )
    .unwrap();
    assert!(static_result.applied, "{static_result:#?}");
    assert_eq!(static_result.resolved_symbol_id, "NAME");

    let enum_result = patch_ast_node(
        Path::new("sample.rs"),
        source,
        "Kind",
        "pub enum Kind {\n    Alpha,\n    Beta,\n    Gamma,\n}",
        None,
    )
    .unwrap();
    assert!(enum_result.applied, "{enum_result:#?}");
    assert_eq!(enum_result.resolved_symbol_id, "Kind");

    let alias_result = patch_ast_node(
        Path::new("sample.rs"),
        source,
        "Alias",
        "pub type Alias = u32;",
        None,
    )
    .unwrap();
    assert!(alias_result.applied, "{alias_result:#?}");
    assert_eq!(alias_result.resolved_symbol_id, "Alias");

    let trait_result = patch_ast_node(
        Path::new("sample.rs"),
        source,
        "Render",
        "pub trait Render {\n    fn render(&self) -> String;\n    fn label(&self) -> String;\n}",
        None,
    )
    .unwrap();
    assert!(trait_result.applied, "{trait_result:#?}");
    assert_eq!(trait_result.resolved_symbol_id, "Render");

    let signature_result = patch_ast_node(
        Path::new("sample.rs"),
        source,
        "Render::render",
        "fn render(&self) -> String;",
        None,
    )
    .unwrap();
    assert!(signature_result.applied, "{signature_result:#?}");
    assert_eq!(signature_result.resolved_symbol_id, "Render::render");
}

#[test]
fn previews_rust_patch_success_and_rejection_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("sample.rs");
    fs::write(&path, RUST_SOURCE).unwrap();

    let replacement = "pub fn compute(value: i32) -> i32 {\n    value + 2\n}";
    let preview = preview_patch_ast_node_from_path(&path, "compute", replacement, None).unwrap();
    assert!(preview.patch.applied, "{preview:#?}");
    assert!(preview.changed);
    assert!(preview.unified_diff.contains("-    value + 1"));
    assert!(preview.unified_diff.contains("+    value + 2"));
    assert_eq!(fs::read_to_string(&path).unwrap(), RUST_SOURCE);

    let rejected = preview_patch_ast_node_from_path(
        &path,
        "compute",
        "pub fn compute(value: i32) -> i32 {\n    value + 2\n",
        None,
    )
    .unwrap();
    assert!(!rejected.patch.applied);
    assert!(!rejected.patch.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), RUST_SOURCE);
}

#[test]
fn patches_dirty_rust_virtual_source_without_writing_disk() {
    let dir = temporary_dir();
    let path = dir.join("sample.rs");
    fs::write(&path, RUST_SOURCE).unwrap();

    let overlay_source = "pub fn compute(value: i32) -> i32 {\n    value + 3\n}\n";
    let replacement = "pub fn compute(value: i32) -> i32 {\n    value + 4\n}";
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&path, Some(overlay_source)).unwrap();

    let result = vfs
        .patch_node_at_position(&path, &Position { row: 0, column: 10 }, replacement, None)
        .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.updated_source, format!("{replacement}\n"));
    let snapshot = vfs.read_file(&path).unwrap();
    assert!(snapshot.dirty);
    assert_eq!(snapshot.source, format!("{replacement}\n"));
    assert_eq!(fs::read_to_string(&path).unwrap(), RUST_SOURCE);
}

#[test]
fn rejects_invalid_rust_replacements_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("sample.rs");
    fs::write(&path, RUST_SOURCE).unwrap();

    let result = patch_ast_node_from_path(
        &path,
        "compute",
        "pub fn compute(value: i32) -> i32 {\n    value + 2\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(!result.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), RUST_SOURCE);
}
