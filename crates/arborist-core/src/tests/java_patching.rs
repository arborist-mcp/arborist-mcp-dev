use std::fs;
use std::path::Path;

use super::{
    Position, VirtualFileSystem, patch_ast_node, patch_ast_node_at_position,
    patch_ast_node_from_path, preview_patch_ast_node_from_path, temporary_dir,
};

const JAVA_SOURCE: &str = "package com.example;\n\npublic class Main {\n    public int add(int left, int right) {\n        return left + right;\n    }\n}\n";

#[test]
fn patches_java_methods_by_semantic_path_and_position() {
    let replacement =
        "public int add(int left, int right) {\n        return left + right + 1;\n    }";

    let semantic_target = patch_ast_node(
        Path::new("Main.java"),
        JAVA_SOURCE,
        "com::example::Main::add",
        replacement,
        None,
    )
    .unwrap();
    assert!(semantic_target.applied, "{semantic_target:#?}");
    assert!(semantic_target.validation.syntax_errors.is_empty());
    assert_eq!(semantic_target.resolved_path, "com::example::Main::add");
    assert_eq!(
        semantic_target.updated_source,
        "package com.example;\n\npublic class Main {\n    public int add(int left, int right) {\n        return left + right + 1;\n    }\n}\n"
    );

    let position_target = patch_ast_node_at_position(
        Path::new("Main.java"),
        JAVA_SOURCE,
        &Position { row: 3, column: 16 },
        replacement,
        None,
    )
    .unwrap();
    assert!(position_target.applied, "{position_target:#?}");
    assert!(position_target.validation.syntax_errors.is_empty());
    assert_eq!(
        position_target.resolved_symbol_id,
        "com::example::Main::add"
    );
    assert_eq!(
        position_target.updated_source,
        "package com.example;\n\npublic class Main {\n    public int add(int left, int right) {\n        return left + right + 1;\n    }\n}\n"
    );
}

#[test]
fn patches_java_classes_constructors_and_nested_types() {
    let source = r#"package com.example;

public class Counter {
    private int value;

    public Counter(int initial) {
        this.value = initial;
    }
}

public class Outer {
    public static class Inner {
        public int helper() {
            return 1;
        }
    }
}
"#;

    let class_result = patch_ast_node(
        Path::new("Counter.java"),
        source,
        "com::example::Counter",
        "public class Counter {\n    private int value;\n    private int limit;\n\n    public Counter(int initial) {\n        this.value = initial;\n    }\n}",
        None,
    )
    .unwrap();
    assert!(class_result.applied, "{class_result:#?}");
    assert!(class_result.validation.syntax_errors.is_empty());
    assert_eq!(class_result.resolved_symbol_id, "com::example::Counter");
    assert!(class_result.updated_source.contains("private int limit;"));

    let constructor_result = patch_ast_node(
        Path::new("Counter.java"),
        source,
        "com::example::Counter::Counter",
        "public Counter(int initial, int limit) {\n        this.value = initial + limit;\n    }",
        None,
    )
    .unwrap();
    assert!(constructor_result.applied, "{constructor_result:#?}");
    assert_eq!(
        constructor_result.resolved_symbol_id,
        "com::example::Counter::Counter"
    );
    assert!(
        constructor_result
            .updated_source
            .contains("public Counter(int initial, int limit)")
    );

    let nested_result = patch_ast_node(
        Path::new("Counter.java"),
        source,
        "com::example::Outer::Inner",
        "public static class Inner {\n        public int helper() {\n            return 2;\n        }\n    }",
        None,
    )
    .unwrap();
    assert!(nested_result.applied, "{nested_result:#?}");
    assert_eq!(
        nested_result.resolved_symbol_id,
        "com::example::Outer::Inner"
    );
    assert!(
        nested_result
            .updated_source
            .contains("public class Outer {")
    );
    assert!(nested_result.updated_source.contains("return 2;"));
}

#[test]
fn patches_java_interfaces_enums_records_and_annotation_types() {
    let source = r#"package com.example;

public interface Renderer {
    String render();
}

public enum Kind {
    BASIC,
    ADVANCED
}

public record Point(int x, int y) {
}

public @interface Marker {
}
"#;

    let interface_result = patch_ast_node(
        Path::new("types.java"),
        source,
        "com::example::Renderer",
        "public interface Renderer {\n    String render();\n    String label();\n}",
        None,
    )
    .unwrap();
    assert!(interface_result.applied, "{interface_result:#?}");
    assert_eq!(
        interface_result.resolved_symbol_id,
        "com::example::Renderer"
    );

    let enum_result = patch_ast_node(
        Path::new("types.java"),
        source,
        "com::example::Kind",
        "public enum Kind {\n    BASIC,\n    ADVANCED,\n    PREMIUM\n}",
        None,
    )
    .unwrap();
    assert!(enum_result.applied, "{enum_result:#?}");
    assert_eq!(enum_result.resolved_symbol_id, "com::example::Kind");

    let record_result = patch_ast_node(
        Path::new("types.java"),
        source,
        "com::example::Point",
        "public record Point(int x, int y) {\n    public int sum() {\n        return x + y;\n    }\n}",
        None,
    )
    .unwrap();
    assert!(record_result.applied, "{record_result:#?}");
    assert_eq!(record_result.resolved_symbol_id, "com::example::Point");

    let annotation_result = patch_ast_node(
        Path::new("types.java"),
        source,
        "com::example::Marker",
        "public @interface Marker {\n    String value();\n}",
        None,
    )
    .unwrap();
    assert!(annotation_result.applied, "{annotation_result:#?}");
    assert_eq!(annotation_result.resolved_symbol_id, "com::example::Marker");
}

#[test]
fn previews_java_patch_success_and_rejection_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("Main.java");
    fs::write(&path, JAVA_SOURCE).unwrap();

    let replacement =
        "public int add(int left, int right) {\n        return left + right + 1;\n    }";
    let preview =
        preview_patch_ast_node_from_path(&path, "com::example::Main::add", replacement, None)
            .unwrap();
    assert!(preview.patch.applied, "{preview:#?}");
    assert!(preview.changed);
    assert!(
        preview
            .unified_diff
            .contains("-        return left + right;")
    );
    assert!(
        preview
            .unified_diff
            .contains("+        return left + right + 1;")
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), JAVA_SOURCE);

    let rejected = preview_patch_ast_node_from_path(
        &path,
        "com::example::Main::add",
        "public int add(int left, int right) {\n        return left + right\n",
        None,
    )
    .unwrap();
    assert!(!rejected.patch.applied);
    assert!(!rejected.patch.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), JAVA_SOURCE);
}

#[test]
fn patches_dirty_java_virtual_source_without_writing_disk() {
    let dir = temporary_dir();
    let path = dir.join("Main.java");
    fs::write(&path, JAVA_SOURCE).unwrap();

    let overlay_source = "package com.example;\n\npublic class Main {\n    public int add(int left, int right) {\n        return left + right + 2;\n    }\n}\n";
    let replacement =
        "public int add(int left, int right) {\n        return left + right + 3;\n    }";
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&path, Some(overlay_source)).unwrap();

    let result = vfs
        .patch_node_at_position(&path, &Position { row: 3, column: 16 }, replacement, None)
        .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(
        result.updated_source,
        "package com.example;\n\npublic class Main {\n    public int add(int left, int right) {\n        return left + right + 3;\n    }\n}\n"
    );
    let snapshot = vfs.read_file(&path).unwrap();
    assert!(snapshot.dirty);
    assert_eq!(
        snapshot.source,
        "package com.example;\n\npublic class Main {\n    public int add(int left, int right) {\n        return left + right + 3;\n    }\n}\n"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), JAVA_SOURCE);
}

#[test]
fn rejects_invalid_java_replacements_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("Main.java");
    fs::write(&path, JAVA_SOURCE).unwrap();

    let result = patch_ast_node_from_path(
        &path,
        "com::example::Main::add",
        "public int add(int left, int right) {\n        return left + right\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(!result.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), JAVA_SOURCE);
}

#[test]
fn validates_java_method_patch_bindings_for_locals_fields_and_imports() {
    let source = r#"package com.example;

import java.util.Formatter;
import static java.util.Objects.requireNonNull;

public class Main {
    private static final int LIMIT = 10;

    public static int helper() {
        return 1;
    }

    public int compute(int value) {
        return value + 1;
    }
}
"#;

    let replacement = r#"public int compute(int value) {
    int bonus = value + 1;
    int total = helper() + LIMIT + bonus;
    requireNonNull(total);
    Formatter fmt = new Formatter();
    fmt.format("%d", total);
    return total;
}"#;
    let result = patch_ast_node(
        Path::new("Main.java"),
        source,
        "com::example::Main::compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    for name in [
        "value",
        "bonus",
        "total",
        "helper",
        "LIMIT",
        "fmt",
        "requireNonNull",
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
        helper_decision
            .selected_symbol_id
            .as_deref()
            .is_some_and(|id| id.contains("::java::com::example::Main::method_declaration::helper")),
        "{helper_decision:#?}"
    );
    let limit_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "LIMIT")
        .unwrap();
    assert!(
        limit_decision
            .selected_symbol_id
            .as_deref()
            .is_some_and(|id| id.contains("::java::com::example::Main::field_declaration::LIMIT")),
        "{limit_decision:#?}"
    );
    let import_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "requireNonNull")
        .unwrap();
    assert_eq!(
        import_decision.candidates.first().unwrap().origin_type,
        "imported_module"
    );
}

#[test]
fn rejects_java_method_patch_with_unresolved_identifier() {
    let source = "package com.example;\n\npublic class Main {\n    public int compute(int value) {\n        return value + 1;\n    }\n}\n";
    let replacement = "public int compute(int value) {\n        return missing(value);\n    }";
    let result = patch_ast_node(
        Path::new("Main.java"),
        source,
        "com::example::Main::compute",
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
fn rejects_java_method_patch_with_unresolved_receiver_and_local_initializer() {
    let source = "package com.example;\n\npublic class Main {\n    public int compute(int value) {\n        return value + 1;\n    }\n}\n";
    let replacement = r#"public int compute(int value) {
    int missing = value + 1;
    return missing + other.count;
}"#;
    let result = patch_ast_node(
        Path::new("Main.java"),
        source,
        "com::example::Main::compute",
        replacement,
        None,
    )
    .unwrap();

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
fn java_patch_binding_validation_resolves_scoped_patterns_and_closures() {
    let source = r#"package com.example;

import java.util.List;

public class Main {
    private java.io.InputStream open() {
        return null;
    }

    public int compute(List<Integer> values) {
        return values.size();
    }
}
"#;

    let replacement = r#"public int compute(List<Integer> values) {
    int total = 0;
    for (int i = 0; i < values.size(); i++) {
        total += values.get(i);
    }
    for (Integer value : values) {
        total += value;
    }
    if (values instanceof java.util.ArrayList list) {
        total += list.size();
    }
    try (var resource = open()) {
        total += resource.hashCode();
    } catch (java.io.IOException ex) {
        total += ex.getMessage().length();
    }
    java.util.function.Function<Integer, Integer> apply = (extra) -> extra + total;
    switch (total) {
        case 0 -> total = values.size();
        default -> total = total + 1;
    }
    total += apply.apply(1);
    return total;
}"#;
    let result = patch_ast_node(
        Path::new("Main.java"),
        source,
        "com::example::Main::compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    for name in [
        "values", "total", "i", "value", "list", "resource", "ex", "apply", "extra", "open",
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
fn java_patch_binding_validation_ignores_type_annotations_and_member_names() {
    let source = "package com.example;\n\npublic class Main {\n    public int add(int left, int right) {\n        return left + right;\n    }\n}\n";
    let replacement = r#"public MissingReturn compute(MissingType value, Helper obj) {
    obj.missingMethod();
    return (MissingReturn) value;
}"#;
    let result = patch_ast_node(
        Path::new("Main.java"),
        source,
        "com::example::Main::add",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(result.validation.binding_decisions.iter().all(|decision| {
        !["MissingType", "MissingReturn", "Helper", "missingMethod"]
            .contains(&decision.name.as_str())
    }));
}

#[test]
fn java_patch_binding_validation_resolves_class_level_field_references() {
    let source = r#"package com.example;

public class Main {
    private static final int BASE = 10;
    private int total = BASE + 1;

    public int compute(int value) {
        return value + total;
    }
}
"#;

    let replacement = r#"public class Main {
    private static final int BASE = 10;
    private int total = BASE + 2;
    private int doubled = total * 2;

    public int compute(int value) {
        return value + doubled;
    }
}"#;
    let result = patch_ast_node(
        Path::new("Main.java"),
        source,
        "com::example::Main",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    for name in ["BASE", "total", "doubled"] {
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
fn java_patch_binding_validation_resolves_same_file_type_items_in_static_calls() {
    let source = r#"package com.example;

class Counter {
    public static int increment() {
        return 1;
    }
}

public class Main {
    public int compute(int value) {
        return value;
    }
}
"#;

    let replacement = r#"public int compute(int value) {
    Counter.increment();
    return value;
}"#;
    let result = patch_ast_node(
        Path::new("Main.java"),
        source,
        "com::example::Main::compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    let counter_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "Counter")
        .expect("expected a decision for Counter: {result:#?}");
    assert_eq!(counter_decision.status, "resolved");
    assert!(
        counter_decision
            .selected_symbol_id
            .as_deref()
            .is_some_and(|id| id.contains("::java::com::example::class_declaration::Counter")),
        "{counter_decision:#?}"
    );
}

#[test]
fn java_patch_binding_validation_rejects_unknown_type_in_static_call() {
    let source = "package com.example;\n\npublic class Main {\n    public int compute(int value) {\n        return value;\n    }\n}\n";
    let replacement = r#"public int compute(int value) {
    MissingType.staticMethod();
    return value;
}"#;
    let result = patch_ast_node(
        Path::new("Main.java"),
        source,
        "com::example::Main::compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(
        result.validation.unresolved_identifiers,
        vec!["MissingType"]
    );
}

#[test]
fn java_patch_binding_validation_resolves_record_components() {
    let source = r#"package com.example;

public record Point(int x, int y) {
    public int sum() {
        return x + y;
    }
}
"#;

    let replacement = r#"public int sum() {
    return x + y + x;
}"#;
    let result = patch_ast_node(
        Path::new("Point.java"),
        source,
        "com::example::Point::sum",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    for name in ["x", "y"] {
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
