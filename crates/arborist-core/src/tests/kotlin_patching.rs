use std::fs;
use std::path::Path;

use super::{
    Position, VirtualFileSystem, patch_ast_node, patch_ast_node_at_position,
    patch_ast_node_from_path, preview_patch_ast_node_from_path, temporary_dir,
};

const KOTLIN_SOURCE: &str = r#"package com.example

class Counter(private val initial: Int) {
    fun increment(amount: Int): Int {
        return initial + amount
    }
}

object Registry {
    fun register(value: String): String {
        return value
    }
}

class Config {
    fun instance(value: Int): Int = value

    companion object {
        fun helper(value: Int): Int = value
        val label = "x"
    }
}

typealias Helper = Counter
"#;

#[test]
fn patches_kotlin_functions_by_semantic_path_and_position() {
    let old_method = "fun increment(amount: Int): Int {\n        return initial + amount\n    }";
    let new_method =
        "fun increment(amount: Int): Int {\n        return initial + amount + 1\n    }";

    let semantic_target = patch_ast_node(
        Path::new("Counter.kt"),
        KOTLIN_SOURCE,
        "com::example::Counter::increment",
        new_method,
        None,
    )
    .unwrap();
    assert!(semantic_target.applied, "{semantic_target:#?}");
    assert!(semantic_target.validation.syntax_errors.is_empty());
    assert_eq!(
        semantic_target.resolved_path,
        "com::example::Counter::increment"
    );
    assert_eq!(
        semantic_target.updated_source,
        KOTLIN_SOURCE.replace(old_method, new_method)
    );

    let position_target = patch_ast_node_at_position(
        Path::new("Counter.kt"),
        KOTLIN_SOURCE,
        &Position { row: 3, column: 8 },
        new_method,
        None,
    )
    .unwrap();
    assert!(position_target.applied, "{position_target:#?}");
    assert!(position_target.validation.syntax_errors.is_empty());
    assert_eq!(
        position_target.resolved_symbol_id,
        "com::example::Counter::increment"
    );
    assert_eq!(
        position_target.updated_source,
        KOTLIN_SOURCE.replace(old_method, new_method)
    );
}

#[test]
fn patches_kotlin_classes_objects_and_type_aliases() {
    let class_result = patch_ast_node(
        Path::new("Types.kt"),
        KOTLIN_SOURCE,
        "com::example::Counter",
        r#"class Counter(private val initial: Int) {
    fun increment(amount: Int): Int {
        return initial + amount
    }

    fun decrement(amount: Int): Int {
        return initial - amount
    }
}"#,
        None,
    )
    .unwrap();
    assert!(class_result.applied, "{class_result:#?}");
    assert!(class_result.validation.syntax_errors.is_empty());
    assert_eq!(class_result.resolved_symbol_id, "com::example::Counter");
    assert!(class_result.updated_source.contains("fun decrement"));

    let object_result = patch_ast_node(
        Path::new("Types.kt"),
        KOTLIN_SOURCE,
        "com::example::Registry",
        r#"object Registry {
    fun register(value: String): String {
        return value
    }

    fun unregister(value: String): String {
        return value
    }
}"#,
        None,
    )
    .unwrap();
    assert!(object_result.applied, "{object_result:#?}");
    assert!(object_result.validation.syntax_errors.is_empty());
    assert_eq!(object_result.resolved_symbol_id, "com::example::Registry");
    assert!(object_result.updated_source.contains("fun unregister"));

    let alias_result = patch_ast_node(
        Path::new("Types.kt"),
        KOTLIN_SOURCE,
        "com::example::Helper",
        "typealias Helper = Map<String, Counter>",
        None,
    )
    .unwrap();
    assert!(alias_result.applied, "{alias_result:#?}");
    assert!(alias_result.validation.syntax_errors.is_empty());
    assert_eq!(alias_result.resolved_symbol_id, "com::example::Helper");
    assert!(
        alias_result
            .updated_source
            .contains("typealias Helper = Map<String, Counter>")
    );
}

#[test]
fn patches_kotlin_companion_and_instance_members_by_semantic_path() {
    let companion_helper = patch_ast_node(
        Path::new("Config.kt"),
        KOTLIN_SOURCE,
        "com::example::Config::Companion::helper",
        "fun helper(value: Int): Int = value + 1",
        None,
    )
    .unwrap();
    assert!(companion_helper.applied, "{companion_helper:#?}");
    assert!(companion_helper.validation.syntax_errors.is_empty());
    assert_eq!(
        companion_helper.resolved_symbol_id,
        "com::example::Config::Companion::helper"
    );
    assert!(
        companion_helper
            .updated_source
            .contains("fun helper(value: Int): Int = value + 1")
    );

    let companion_label = patch_ast_node(
        Path::new("Config.kt"),
        KOTLIN_SOURCE,
        "com::example::Config::Companion::label",
        r#"val label = "y""#,
        None,
    )
    .unwrap();
    assert!(companion_label.applied, "{companion_label:#?}");
    assert!(companion_label.validation.syntax_errors.is_empty());
    assert_eq!(
        companion_label.resolved_symbol_id,
        "com::example::Config::Companion::label"
    );
    assert!(
        companion_label
            .updated_source
            .contains(r#"val label = "y""#)
    );

    let instance_member = patch_ast_node(
        Path::new("Config.kt"),
        KOTLIN_SOURCE,
        "com::example::Config::instance",
        "fun instance(value: Int): Int = value * 2",
        None,
    )
    .unwrap();
    assert!(instance_member.applied, "{instance_member:#?}");
    assert!(instance_member.validation.syntax_errors.is_empty());
    assert_eq!(
        instance_member.resolved_symbol_id,
        "com::example::Config::instance"
    );
    assert!(
        instance_member
            .updated_source
            .contains("fun instance(value: Int): Int = value * 2")
    );
}

#[test]
fn previews_kotlin_patch_success_and_rejection_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("Counter.kt");
    fs::write(&path, KOTLIN_SOURCE).unwrap();

    let new_method =
        "fun increment(amount: Int): Int {\n        return initial + amount + 1\n    }";
    let preview = preview_patch_ast_node_from_path(
        &path,
        "com::example::Counter::increment",
        new_method,
        None,
    )
    .unwrap();
    assert!(preview.patch.applied, "{preview:#?}");
    assert!(preview.changed);
    assert!(
        preview
            .unified_diff
            .contains("-        return initial + amount")
    );
    assert!(
        preview
            .unified_diff
            .contains("+        return initial + amount + 1")
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), KOTLIN_SOURCE);

    let rejected = preview_patch_ast_node_from_path(
        &path,
        "com::example::Counter::increment",
        "fun increment(amount: Int): Int {\n        return initial + amount\n",
        None,
    )
    .unwrap();
    assert!(!rejected.patch.applied);
    assert!(!rejected.patch.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), KOTLIN_SOURCE);
}

#[test]
fn patches_dirty_kotlin_virtual_source_without_writing_disk() {
    let dir = temporary_dir();
    let path = dir.join("Counter.kt");
    fs::write(&path, KOTLIN_SOURCE).unwrap();

    let overlay_source = KOTLIN_SOURCE.replace(
        "fun increment(amount: Int): Int {\n        return initial + amount\n    }",
        "fun increment(amount: Int): Int {\n        return initial + amount + 2\n    }",
    );
    let new_method =
        "fun increment(amount: Int): Int {\n        return initial + amount + 3\n    }";
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&path, Some(overlay_source.as_str())).unwrap();

    let result = vfs
        .patch_node_at_position(&path, &Position { row: 3, column: 8 }, new_method, None)
        .unwrap();

    assert!(result.applied, "{result:#?}");
    let expected = overlay_source.replace(
        "fun increment(amount: Int): Int {\n        return initial + amount + 2\n    }",
        new_method,
    );
    assert_eq!(result.updated_source, expected);
    let snapshot = vfs.read_file(&path).unwrap();
    assert!(snapshot.dirty);
    assert_eq!(snapshot.source, expected);
    assert_eq!(fs::read_to_string(&path).unwrap(), KOTLIN_SOURCE);
}

#[test]
fn rejects_invalid_kotlin_replacements_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("Counter.kt");
    fs::write(&path, KOTLIN_SOURCE).unwrap();

    let result = patch_ast_node_from_path(
        &path,
        "com::example::Counter::increment",
        "fun increment(amount: Int): Int {\n        return initial + amount\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(!result.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), KOTLIN_SOURCE);
}
