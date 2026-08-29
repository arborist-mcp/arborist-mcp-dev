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

#[test]
fn validates_rust_function_patch_bindings_for_locals_items_and_imports() {
    let source = r#"use crate::api::helper as imported_helper;

pub const LIMIT: usize = 10;

fn helper(value: i32) -> i32 {
    value + 1
}

pub fn compute(value: i32) -> i32 {
    let bonus = 2;
    value + bonus
}
"#;

    let replacement = r#"pub fn compute(value: i32) -> i32 {
    let bonus = 2;
    helper(value) + bonus + LIMIT + imported_helper(value) + Some(value).unwrap_or(0) + crate::LIMIT
}"#;
    let result =
        patch_ast_node(Path::new("sample.rs"), source, "compute", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.syntax_errors.is_empty());
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    assert_eq!(result.validation.ambiguous_identifiers.len(), 0);

    for name in ["value", "bonus", "helper", "LIMIT", "imported_helper"] {
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == name && decision.status == "resolved"),
            "missing resolved decision for {name}: {:#?}",
            result.validation.binding_decisions
        );
    }
    let helper_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "helper")
        .unwrap();
    assert_eq!(helper_binding.symbol.scope_path, None);
    assert_eq!(helper_binding.symbol.node_kind, "function_item");
    assert_eq!(helper_binding.symbol.origin_type, "module_scope");
    let imported_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "imported_helper")
        .unwrap();
    assert_eq!(imported_binding.symbol.node_kind, "use_declaration");
    assert_eq!(imported_binding.symbol.origin_type, "imported_module");
    let local_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "bonus")
        .unwrap();
    assert_eq!(local_binding.symbol.node_kind, "let_declaration");
    assert_eq!(local_binding.symbol.origin_type, "local_scope");
    assert_eq!(local_binding.symbol.scope_path.as_deref(), Some("compute"));
}

#[test]
fn rejects_rust_function_patch_with_unresolved_identifier() {
    let source = "pub fn compute(value: i32) -> i32 {\n    value + 1\n}\n";

    let replacement = "pub fn compute(value: i32) -> i32 {\n    missing_helper(value) + 1\n}";
    let result =
        patch_ast_node(Path::new("sample.rs"), source, "compute", replacement, None).unwrap();

    assert!(!result.applied, "{result:#?}");
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
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "value" && decision.status == "resolved")
    );
}

#[test]
fn rejects_rust_function_patch_with_unresolved_receiver_and_let_initializer() {
    let source = "pub fn compute(value: i32) -> i32 {\n    value + 1\n}\n";

    let replacement = r#"pub fn compute(value: i32) -> i32 {
    let missing_local = value + 1;
    missing_local + other.increment()
}"#;
    let result =
        patch_ast_node(Path::new("sample.rs"), source, "compute", replacement, None).unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, vec!["other"]);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "missing_local" && decision.status == "resolved")
    );
}

#[test]
fn rust_patch_binding_validation_resolves_scoped_patterns_guards_and_closures() {
    let source = r#"pub fn compute(values: Vec<i32>, maybe: Option<i32>) -> i32 {
    let mut total = 0;
    for item in values {
        total += item;
    }
    let doubled = |x: i32| x * 2;
    total += doubled(total);
    match maybe {
        Some(v) if v > 0 => total += v,
        None => total += 0,
    }
    if let Some(w) = maybe {
        total += w;
    }
    total
}
"#;

    let replacement = r#"pub fn compute(values: Vec<i32>, maybe: Option<i32>) -> i32 {
    let mut total = 0;
    for item in values {
        total += item * 2;
    }
    let doubled = |x: i32| x * 2;
    total += doubled(total);
    match maybe {
        Some(v) if v > 0 => total += v,
        None => total += 0,
    }
    if let Some(w) = maybe {
        total += w;
    }
    total
}"#;
    let result =
        patch_ast_node(Path::new("sample.rs"), source, "compute", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    for name in ["values", "total", "item", "doubled", "x", "v", "w", "maybe"] {
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == name && decision.status == "resolved"),
            "missing resolved decision for {name}: {:#?}",
            result.validation.binding_decisions
        );
    }
}

#[test]
fn rust_patch_binding_validation_rejects_references_outside_nested_block_scope() {
    let source = "pub fn compute(value: i32) -> i32 {
    value
}
";
    let result = patch_ast_node(
        Path::new("sample.rs"),
        source,
        "compute",
        "pub fn compute(value: i32) -> i32 {
    {
        let helper = value + 1;
        let _ = helper;
    }
    helper
}",
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn rust_patch_binding_validation_rejects_nested_items_referenced_outside_their_block() {
    let source = "pub fn compute(value: i32) -> i32 {
    value
}
";
    let result = patch_ast_node(
        Path::new("sample.rs"),
        source,
        "compute",
        "pub fn compute(value: i32) -> i32 {
    {
        fn helper(input: i32) -> i32 {
            input + 1
        }
        let _ = helper(value);
    }
    helper(value)
}",
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn rust_patch_binding_validation_rejects_ambiguous_hoisted_nested_items() {
    let source = "pub fn compute(value: i32) -> i32 {\n    value\n}\n";
    let result = patch_ast_node(
        Path::new("sample.rs"),
        source,
        "compute",
        "pub fn compute(value: i32) -> i32 {
    let result = helper(value);
    fn helper(input: i32) -> i32 {
        input + 1
    }
    fn helper(input: i32) -> i32 {
        input + 2
    }
    result
}",
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert_eq!(result.validation.ambiguous_identifiers.len(), 1);
    assert_eq!(result.validation.ambiguous_identifiers[0].name, "helper");
    assert_eq!(
        result.validation.ambiguous_identifiers[0].candidates.len(),
        2
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "helper" && decision.status == "ambiguous")
    );
}

#[test]
fn rust_patch_binding_validation_resolves_method_bodies_without_member_checks() {
    let source = r#"pub struct Counter {
    count: i32,
}

impl Counter {
    pub fn increment(&mut self) -> i32 {
        self.count += 1;
        self.count
    }
}
"#;

    let replacement = r#"pub fn increment(&mut self) -> i32 {
    self.count += 1;
    self.count * 2
}"#;
    let result = patch_ast_node(
        Path::new("point.rs"),
        source,
        "Counter::increment",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
}

#[test]
fn rust_patch_binding_validation_ignores_type_annotations() {
    let source = "pub fn compute(value: i32) -> i32 {\n    value + 1\n}\n";

    let replacement = "pub fn compute(value: MissingType) -> MissingReturn {\n    value\n}";
    let result =
        patch_ast_node(Path::new("sample.rs"), source, "compute", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .all(|decision| decision.name != "MissingType" && decision.name != "MissingReturn")
    );
}

#[test]
fn rust_patch_binding_validation_resolves_and_rejects_const_item_references() {
    let source = "pub const BASE: usize = 10;\npub const LIMIT: usize = BASE;\n";

    let allowed = patch_ast_node(
        Path::new("sample.rs"),
        source,
        "LIMIT",
        "pub const LIMIT: usize = BASE + 1;",
        None,
    )
    .unwrap();
    assert!(allowed.applied, "{allowed:#?}");
    assert!(
        allowed
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "BASE" && decision.status == "resolved")
    );

    let rejected = patch_ast_node(
        Path::new("sample.rs"),
        source,
        "LIMIT",
        "pub const LIMIT: usize = BASE + MISSING;",
        None,
    )
    .unwrap();
    assert!(!rejected.applied, "{rejected:#?}");
    assert_eq!(rejected.validation.unresolved_identifiers, vec!["MISSING"]);
}

#[test]
fn rust_patch_binding_validation_resolves_hoisted_nested_items() {
    let source = "pub fn compute(value: i32) -> i32 {\n    value + 1\n}\n";

    let replacement = r#"pub fn compute(value: i32) -> i32 {
    let result = nested_helper(value);
    fn nested_helper(x: i32) -> i32 {
        x
    }
    result
}"#;
    let result =
        patch_ast_node(Path::new("sample.rs"), source, "compute", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "nested_helper" && decision.status == "resolved")
    );
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "x" && decision.status == "resolved")
    );
}

#[test]
fn rust_patch_binding_validation_resolves_same_file_type_items_in_value_positions() {
    let source = "pub struct Unit;\npub struct Counter(i32);\npub fn compute(value: i32) -> i32 {\n    value + 1\n}\n";

    let replacement = r#"pub fn compute(value: i32) -> i32 {
    let u = Unit;
    let c = Counter(1);
    value + c.0
}"#;
    let result =
        patch_ast_node(Path::new("sample.rs"), source, "compute", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    for name in ["Unit", "Counter"] {
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == name && decision.status == "resolved"),
            "expected resolved decision for {name}: {result:#?}"
        );
    }
}
