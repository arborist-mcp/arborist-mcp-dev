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

#[test]
fn validates_kotlin_method_patch_bindings_for_locals_fields_and_imports() {
    let source = r#"package com.example

import com.example.util.Helper
import com.example.util.other

class Counter(private val initial: Int) {
    val label = "x"
    var count: Int = 0

    fun helper(): Int {
        return 1
    }

    fun compute(amount: Int): Int {
        val bonus = amount + 1
        return initial + bonus
    }
}
"#;

    let replacement = r#"fun compute(amount: Int): Int {
    val bonus = amount + 1
    var total = helper() + initial + label.length + bonus + count
    other(total)
    Helper.help(total)
    return total
}"#;
    let result = patch_ast_node(
        Path::new("Counter.kt"),
        source,
        "com::example::Counter::compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    for name in [
        "amount", "bonus", "total", "helper", "initial", "label", "count", "other", "Helper",
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
    let helper_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "helper")
        .unwrap();
    assert!(
        helper_decision.selected_symbol_id.as_deref().is_some_and(
            |id| id.contains("::kotlin::com::example::Counter::function_declaration::helper")
        ),
        "{helper_decision:#?}"
    );
    let initial_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "initial")
        .unwrap();
    assert!(
        initial_decision.selected_symbol_id.as_deref().is_some_and(
            |id| id.contains("::kotlin::com::example::Counter::class_parameter::initial")
        ),
        "{initial_decision:#?}"
    );
    let label_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "label")
        .unwrap();
    assert!(
        label_decision.selected_symbol_id.as_deref().is_some_and(
            |id| id.contains("::kotlin::com::example::Counter::property_declaration::label")
        ),
        "{label_decision:#?}"
    );
    let import_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "other")
        .unwrap();
    assert_eq!(
        import_decision.candidates.first().unwrap().origin_type,
        "imported_module"
    );
}

#[test]
fn rejects_kotlin_method_patch_with_unresolved_identifier() {
    let source = "package com.example\n\nfun compute(value: Int): Int {\n    return value + 1\n}\n";
    let replacement = "fun compute(value: Int): Int {\n    return missing(value)\n}";
    let result = patch_ast_node(
        Path::new("Compute.kt"),
        source,
        "com::example::compute",
        replacement,
        None,
    )
    .unwrap();

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
fn rejects_kotlin_method_patch_with_unresolved_local_initializer() {
    let source = "package com.example\n\nfun compute(value: Int): Int {\n    return value + 1\n}\n";
    let replacement =
        "fun compute(value: Int): Int {\n    val bonus = unknown + 1\n    return value + bonus\n}";
    let result = patch_ast_node(
        Path::new("Compute.kt"),
        source,
        "com::example::compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, vec!["unknown"]);
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "bonus" && decision.status == "resolved")
    );
}

#[test]
fn kotlin_patch_binding_validation_resolves_scoped_lambdas_loops_and_catches() {
    let source = r#"package com.example

fun pair(): Pair<Int, Int> = Pair(1, 2)

fun risky(): Int = 0

fun log(value: String) {
    println(value)
}

fun topLevel(value: Int): Int {
    val list = listOf(1, 2, 3)
    val mapped = list.map { it * 2 }
    list.forEach { item ->
        print(item)
    }
    val (a, b) = pair()
    for (i in 0 until value) {
        println(i)
    }
    try {
        risky()
    } catch (e: Exception) {
        log(e.message)
    }
    return mapped.size + a + b
}
"#;

    let replacement = r#"fun topLevel(value: Int): Int {
    val list = listOf(1, 2, 3)
    val mapped = list.map { it * 2 }
    list.forEach { item ->
        print(item)
    }
    val (a, b) = pair()
    for (i in 0 until value) {
        println(i + a)
    }
    try {
        risky()
    } catch (e: Exception) {
        log(e.message)
    }
    return mapped.size + b
}"#;
    let result = patch_ast_node(
        Path::new("TopLevel.kt"),
        source,
        "com::example::topLevel",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    for name in ["it", "item", "a", "b", "i", "e", "value", "list", "mapped"] {
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == name && decision.status == "resolved"),
            "expected resolved decision for {name}: {result:#?}"
        );
    }
    for name in ["pair", "risky", "log"] {
        let decision = result
            .validation
            .binding_decisions
            .iter()
            .find(|decision| decision.name == name)
            .unwrap();
        assert!(
            decision
                .selected_symbol_id
                .as_deref()
                .is_some_and(|id| id.contains("::kotlin::com::example::function_declaration::")),
            "{decision:#?}"
        );
    }
}

#[test]
fn kotlin_patch_binding_validation_ignores_type_annotations_and_member_names() {
    let source = r#"package com.example

import com.example.util.Helper

class Counter(private val initial: Int) {
    fun compute(amount: Int): String {
        return Helper.help(initial.toString())
    }
}
"#;

    let replacement = r#"fun compute(amount: Int): String {
    val text = amount.toString() + initial.toString()
    val cast = amount as Int
    val checked = amount is Int
    val result = Helper.help(text)
    return result
}"#;
    let result = patch_ast_node(
        Path::new("Counter.kt"),
        source,
        "com::example::Counter::compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(
        result.validation.unresolved_identifiers.is_empty(),
        "{result:#?}"
    );
    for name in ["amount", "initial", "text", "result", "Helper"] {
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
fn kotlin_patch_binding_validation_resolves_class_level_field_references() {
    let source = r#"package com.example

class Counter(private val initial: Int) {
    val label = "x"
    var count: Int = 0

    fun compute(amount: Int): Int {
        return initial + count + label.length + amount
    }
}
"#;

    let replacement = r#"class Counter(private val initial: Int) {
    val label = "x"
    var count: Int = 0

    fun compute(amount: Int): Int {
        val total = initial + count + label.length + amount
        return total
    }
}"#;
    let result = patch_ast_node(
        Path::new("Counter.kt"),
        source,
        "com::example::Counter",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(
        result.validation.unresolved_identifiers.is_empty(),
        "{result:#?}"
    );
    for name in ["initial", "label", "count", "amount", "total"] {
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
fn kotlin_patch_binding_validation_resolves_same_file_items_in_bare_calls() {
    let source = r#"package com.example

object Registry {
    fun register(value: String): String {
        return value
    }
}

fun topLevel(value: Int): Int {
    return value * 2
}
"#;

    let replacement = r#"fun register(value: String): String {
    val doubled = topLevel(value.length)
    return value + doubled.toString()
}"#;
    let result = patch_ast_node(
        Path::new("Registry.kt"),
        source,
        "com::example::Registry::register",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(
        result.validation.unresolved_identifiers.is_empty(),
        "{result:#?}"
    );
    let top_level_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "topLevel")
        .unwrap();
    assert!(
        top_level_decision
            .selected_symbol_id
            .as_deref()
            .is_some_and(|id| id.contains("::kotlin::com::example::function_declaration::topLevel")),
        "{top_level_decision:#?}"
    );
}

#[test]
fn kotlin_patch_binding_validation_rejects_unknown_type_in_bare_call() {
    let source = "package com.example\n\nfun compute(value: Int): Int {\n    return value + 1\n}\n";
    let replacement = "fun compute(value: Int): Int {\n    return MissingFactory(value)\n}";
    let result = patch_ast_node(
        Path::new("Compute.kt"),
        source,
        "com::example::compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(
        result.validation.unresolved_identifiers,
        vec!["MissingFactory"]
    );
}
