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

#[test]
fn validates_go_function_patch_bindings_for_locals_items_and_imports() {
    let source = r#"package sample

import "fmt"

const LIMIT = 10

func helper() int {
	return 1
}

func compute(value int) int {
	return value + 1
}
"#;

    let replacement = r#"func compute(value int) int {
	bonus := value + 1
	total := helper() + LIMIT + bonus
	fmt.Sprintf("%d", total)
	return total
}"#;
    let result =
        patch_ast_node(Path::new("sample.go"), source, "compute", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    for name in ["value", "bonus", "total", "helper", "LIMIT", "fmt"] {
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == name && decision.status == "resolved"),
            "expected resolved decision for {name}: {result:#?}"
        );
    }
    let helper_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "helper")
        .unwrap();
    assert!(
        helper_decision
            .selected_symbol_id
            .as_deref()
            .is_some_and(|id| id.contains("::go::<module>::function_declaration::helper")),
        "{helper_decision:#?}"
    );
    let fmt_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "fmt")
        .unwrap();
    assert_eq!(
        fmt_decision.candidates.first().unwrap().origin_type,
        "imported_module"
    );
}

#[test]
fn rejects_go_patches_that_use_unresolved_local_package_imports() {
    let root = temporary_dir();
    let caller_path = root.join("cmd").join("main.go");
    let package_path = root.join("internal").join("service").join("service.go");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(package_path.parent().unwrap()).unwrap();
    fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &package_path,
        "package catalog\n\nfunc Value() int { return 1 }\n",
    )
    .unwrap();

    let source = r#"package main

import "example.com/project/internal/service"

func compute() int {
	return 1
}
"#;
    fs::write(&caller_path, source).unwrap();

    let result = patch_ast_node(
        &caller_path,
        source,
        "compute",
        "func compute() int {\n\treturn service.Value()\n}",
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, vec!["service"]);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| { decision.name == "service" && decision.status == "unresolved" })
    );
}

#[test]
fn rejects_go_patches_that_treat_dot_or_blank_imports_as_package_bindings() {
    let root = temporary_dir();
    let caller_path = root.join("cmd").join("main.go");
    let service_path = root.join("internal").join("service").join("service.go");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(service_path.parent().unwrap()).unwrap();
    fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &service_path,
        "package service\n\nfunc Value() int { return 1 }\n",
    )
    .unwrap();

    for (name, import) in [
        ("dot", ". \"example.com/project/internal/service\""),
        ("blank", "_ \"example.com/project/internal/service\""),
    ] {
        let source =
            format!("package main\n\nimport {import}\n\nfunc compute() int {{\n\treturn 1\n}}\n");
        fs::write(&caller_path, &source).unwrap();
        let result = patch_ast_node(
            &caller_path,
            &source,
            "compute",
            "func compute() int {\n\treturn service.Value()\n}",
            None,
        )
        .unwrap();

        assert!(!result.applied, "{name}: {result:#?}");
        assert_eq!(result.validation.unresolved_identifiers, vec!["service"]);
    }
}

#[test]
fn resolves_go_imported_package_names_when_path_basenames_collide() {
    let root = temporary_dir();
    let caller_path = root.join("cmd").join("main.go");
    let first_path = root
        .join("internal")
        .join("first")
        .join("foo")
        .join("source.go");
    let second_path = root
        .join("internal")
        .join("second")
        .join("foo")
        .join("source.go");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(first_path.parent().unwrap()).unwrap();
    fs::create_dir_all(second_path.parent().unwrap()).unwrap();
    fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &first_path,
        "package first\n\nfunc Value() int { return 1 }\n",
    )
    .unwrap();
    fs::write(
        &second_path,
        "package second\n\nfunc Value() int { return 2 }\n",
    )
    .unwrap();

    let source = r#"package main

import (
	"example.com/project/internal/first/foo"
	"example.com/project/internal/second/foo"
)

func compute() int {
	return 1
}
"#;
    fs::write(&caller_path, source).unwrap();

    let result = patch_ast_node(
        &caller_path,
        source,
        "compute",
        "func compute() int {\n\treturn first.Value() + second.Value()\n}",
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
    for name in ["first", "second"] {
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

#[test]
fn rejects_go_patches_with_ambiguous_local_package_import_bindings() {
    let root = temporary_dir();
    let caller_path = root.join("cmd").join("main.go");
    let first_path = root.join("internal").join("first").join("first.go");
    let second_path = root.join("internal").join("second").join("second.go");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(first_path.parent().unwrap()).unwrap();
    fs::create_dir_all(second_path.parent().unwrap()).unwrap();
    fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &first_path,
        "package first\n\nfunc Value() int { return 1 }\n",
    )
    .unwrap();
    fs::write(
        &second_path,
        "package second\n\nfunc Value() int { return 2 }\n",
    )
    .unwrap();

    let source = r#"package main

import (
	alias "example.com/project/internal/first"
	alias "example.com/project/internal/second"
)

func compute() int {
	return 1
}
"#;
    fs::write(&caller_path, source).unwrap();

    let result = patch_ast_node(
        &caller_path,
        source,
        "compute",
        "func compute() int {\n\treturn alias.Value()\n}",
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, vec!["alias"]);
}

#[test]
fn rejects_go_function_patch_with_unresolved_identifier() {
    let source = "package sample\n\nfunc compute(value int) int {\n\treturn value + 1\n}\n";
    let replacement = "func compute(value int) int {\n\treturn missing(value)\n}";
    let result =
        patch_ast_node(Path::new("sample.go"), source, "compute", replacement, None).unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, vec!["missing"]);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "value" && decision.status == "resolved")
    );
}

#[test]
fn rejects_go_function_patch_with_unresolved_receiver_and_short_var_initializer() {
    let source = "package sample\n\nfunc compute(value int) int {\n\treturn value + 1\n}\n";
    let replacement = r#"func compute(value int) int {
	missing := value + 1
	return missing + other.Count
}"#;
    let result =
        patch_ast_node(Path::new("sample.go"), source, "compute", replacement, None).unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, vec!["other"]);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "missing" && decision.status == "resolved")
    );
}

#[test]
fn go_patch_binding_validation_resolves_scoped_patterns_guards_and_closures() {
    let source = "package sample\n\nfunc compute(values []int) int {\n\treturn len(values)\n}\n";
    let replacement = r#"func compute(values []int) int {
	total := 0
	for i, value := range values {
		_ = i
		if doubled := value * 2; doubled > total {
			total = doubled
		}
	}
	apply := func(extra int) int {
		return total + extra
	}
	switch tag := total; tag {
	case 0:
		_ = tag
		return apply(1)
	}
	return apply(2)
}"#;
    let result =
        patch_ast_node(Path::new("sample.go"), source, "compute", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    for name in [
        "values", "total", "i", "value", "doubled", "extra", "tag", "apply",
    ] {
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

#[test]
fn go_patch_binding_validation_resolves_method_bodies_without_member_checks() {
    let source = r#"package sample

type Counter struct {
	count int
}

func (c *Counter) Increment() int {
	c.count++
	return c.count
}
"#;

    let replacement = r#"func (c *Counter) Increment() int {
	c.count += 2
	return c.count
}"#;
    let result = patch_ast_node(
        Path::new("counter.go"),
        source,
        "Counter::Increment",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "c" && decision.status == "resolved")
    );
}

#[test]
fn go_patch_binding_validation_ignores_type_annotations() {
    let source = "package sample\n\nfunc compute(value int) int {\n\treturn value\n}\n";
    let replacement = "func compute(value MissingType) MissingReturn {\n\treturn value\n}";
    let result =
        patch_ast_node(Path::new("sample.go"), source, "compute", replacement, None).unwrap();

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
fn go_patch_binding_validation_resolves_and_rejects_const_and_var_item_references() {
    let source = "package sample\n\nconst BASE = 10\n\nvar count = BASE\n\nfunc compute(value int) int {\n\treturn value + count\n}\n";
    let allowed = patch_ast_node(
        Path::new("sample.go"),
        source,
        "compute",
        "func compute(value int) int {\n\treturn value + BASE + count\n}",
        None,
    )
    .unwrap();
    assert!(allowed.applied, "{allowed:#?}");
    for name in ["BASE", "count"] {
        assert!(
            allowed
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == name && decision.status == "resolved"),
            "expected resolved decision for {name}: {allowed:#?}"
        );
    }

    let rejected = patch_ast_node(
        Path::new("sample.go"),
        source,
        "compute",
        "func compute(value int) int {\n\treturn value + BASE + MISSING\n}",
        None,
    )
    .unwrap();
    assert!(!rejected.applied, "{rejected:#?}");
    assert_eq!(rejected.validation.unresolved_identifiers, vec!["MISSING"]);
}

#[test]
fn go_patch_binding_validation_resolves_same_file_type_items_in_static_calls() {
    let source = r#"package sample

type Counter struct {
	count int
}

func (c *Counter) Increment() int {
	c.count++
	return c.count
}

func compute(value int) int {
	return value + 1
}
"#;

    let replacement = "func compute(value int) int {\n\treturn Counter.Increment()\n}";
    let result =
        patch_ast_node(Path::new("sample.go"), source, "compute", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    assert!(
        result.validation.binding_decisions.iter().any(|decision| {
            decision.name == "Counter"
                && decision.status == "resolved"
                && decision
                    .selected_symbol_id
                    .as_deref()
                    .is_some_and(|id| id.contains("::go::<module>::type_spec::Counter"))
        }),
        "{result:#?}"
    );
}
