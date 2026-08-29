use std::fs;
use std::path::Path;

use super::{
    Position, VirtualFileSystem, patch_ast_node, patch_ast_node_at_position,
    patch_ast_node_from_path, preview_patch_ast_node_from_path, temporary_dir,
};

const CSHARP_SOURCE: &str = r#"using System;

namespace Demo.Core {
    public class Counter {
        private int initial;

        public Counter(int initial) {
            this.initial = initial;
        }

        public int Increment(int amount) {
            return initial + amount;
        }
    }
}
"#;

#[test]
fn patches_csharp_methods_by_semantic_path_and_position() {
    let old_method =
        "public int Increment(int amount) {\n            return initial + amount;\n        }";
    let new_method =
        "public int Increment(int amount) {\n            return initial + amount + 1;\n        }";

    let semantic_target = patch_ast_node(
        Path::new("Counter.cs"),
        CSHARP_SOURCE,
        "Demo::Core::Counter::Increment",
        new_method,
        None,
    )
    .unwrap();
    assert!(semantic_target.applied, "{semantic_target:#?}");
    assert!(semantic_target.validation.syntax_errors.is_empty());
    assert_eq!(
        semantic_target.resolved_path,
        "Demo::Core::Counter::Increment"
    );
    assert_eq!(
        semantic_target.updated_source,
        CSHARP_SOURCE.replace(old_method, new_method)
    );

    let position_target = patch_ast_node_at_position(
        Path::new("Counter.cs"),
        CSHARP_SOURCE,
        &Position {
            row: 10,
            column: 22,
        },
        new_method,
        None,
    )
    .unwrap();
    assert!(position_target.applied, "{position_target:#?}");
    assert!(position_target.validation.syntax_errors.is_empty());
    assert_eq!(
        position_target.resolved_symbol_id,
        "Demo::Core::Counter::Increment"
    );
    assert_eq!(
        position_target.updated_source,
        CSHARP_SOURCE.replace(old_method, new_method)
    );
}

#[test]
fn patches_csharp_types_in_file_scoped_namespaces() {
    let source = r#"namespace Shapes;

public struct Point {
    public int X;
}

public interface IRenderer {
    string Render();
}

public enum Kind {
    Basic,
    Advanced
}

public record Entry(string Name);
"#;

    let struct_result = patch_ast_node(
        Path::new("Shapes.cs"),
        source,
        "Shapes::Point",
        "public struct Point {\n    public int X;\n    public int Y;\n}",
        None,
    )
    .unwrap();
    assert!(struct_result.applied, "{struct_result:#?}");
    assert_eq!(struct_result.resolved_symbol_id, "Shapes::Point");

    let interface_result = patch_ast_node(
        Path::new("Shapes.cs"),
        source,
        "Shapes::IRenderer",
        "public interface IRenderer {\n    string Render();\n    string Label();\n}",
        None,
    )
    .unwrap();
    assert!(interface_result.applied, "{interface_result:#?}");
    assert_eq!(interface_result.resolved_symbol_id, "Shapes::IRenderer");

    let enum_result = patch_ast_node(
        Path::new("Shapes.cs"),
        source,
        "Shapes::Kind",
        "public enum Kind {\n    Basic,\n    Advanced,\n    Premium\n}",
        None,
    )
    .unwrap();
    assert!(enum_result.applied, "{enum_result:#?}");
    assert_eq!(enum_result.resolved_symbol_id, "Shapes::Kind");

    let record_result = patch_ast_node(
        Path::new("Shapes.cs"),
        source,
        "Shapes::Entry",
        "public record Entry(string Name, string Label);",
        None,
    )
    .unwrap();
    assert!(record_result.applied, "{record_result:#?}");
    assert_eq!(record_result.resolved_symbol_id, "Shapes::Entry");
}

#[test]
fn patches_csharp_classes_constructors_nested_types_and_attributes() {
    let source = r#"namespace Demo.Core {
    [Serializable]
    public class Outer {
        public class Inner {
            private int value;

            public Inner(int value) {
                this.value = value;
            }

            public int Value() => value;
        }

        public void Initialize() {}

        public Outer() {}
    }
}
"#;
    let nested_result = patch_ast_node(
        Path::new("Nested.cs"),
        source,
        "Demo::Core::Outer::Inner",
        "public class Inner {\n            private int value;\n\n            public Inner(int value) {\n                this.value = value;\n            }\n\n            public int Value() => value;\n            public int Double() => value * 2;\n        }",
        None,
    )
    .unwrap();
    assert!(nested_result.applied, "{nested_result:#?}");
    assert!(nested_result.validation.syntax_errors.is_empty());
    assert_eq!(nested_result.resolved_symbol_id, "Demo::Core::Outer::Inner");
    assert!(
        nested_result
            .updated_source
            .contains("public class Outer {")
    );
    assert!(
        nested_result
            .updated_source
            .contains("public int Double() => value * 2;")
    );

    let constructor_result = patch_ast_node(
        Path::new("Nested.cs"),
        source,
        "Demo::Core::Outer::Outer",
        "public Outer() {\n            Initialize();\n        }",
        None,
    )
    .unwrap();
    assert!(constructor_result.applied, "{constructor_result:#?}");
    assert_eq!(
        constructor_result.resolved_symbol_id,
        "Demo::Core::Outer::Outer"
    );
    assert!(constructor_result.updated_source.contains("Initialize();"));

    let class_result = patch_ast_node(
        Path::new("Nested.cs"),
        source,
        "Demo::Core::Outer",
        "[Serializable]\n    public class Outer {\n        public class Inner {\n            private int value;\n\n            public Inner(int value) {\n                this.value = value;\n            }\n\n            public int Value() => value;\n        }\n\n        public void Initialize() {}\n\n        public Outer() {}\n    }",
        None,
    )
    .unwrap();
    assert!(class_result.applied, "{class_result:#?}");
    assert!(class_result.validation.syntax_errors.is_empty());
    assert_eq!(class_result.resolved_symbol_id, "Demo::Core::Outer");
    assert!(class_result.updated_source.contains("[Serializable]"));
}

#[test]
fn previews_csharp_patch_success_and_rejection_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("Counter.cs");
    fs::write(&path, CSHARP_SOURCE).unwrap();

    let new_method =
        "public int Increment(int amount) {\n            return initial + amount + 1;\n        }";
    let preview =
        preview_patch_ast_node_from_path(&path, "Demo::Core::Counter::Increment", new_method, None)
            .unwrap();
    assert!(preview.patch.applied, "{preview:#?}");
    assert!(preview.changed);
    assert!(
        preview
            .unified_diff
            .contains("-            return initial + amount;")
    );
    assert!(
        preview
            .unified_diff
            .contains("+            return initial + amount + 1;")
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), CSHARP_SOURCE);

    let rejected = preview_patch_ast_node_from_path(
        &path,
        "Demo::Core::Counter::Increment",
        "public int Increment(int amount) {\n            return initial + amount\n",
        None,
    )
    .unwrap();
    assert!(!rejected.patch.applied);
    assert!(!rejected.patch.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), CSHARP_SOURCE);
}

#[test]
fn patches_dirty_csharp_virtual_source_without_writing_disk() {
    let dir = temporary_dir();
    let path = dir.join("Counter.cs");
    fs::write(&path, CSHARP_SOURCE).unwrap();

    let overlay_source = r#"using System;

namespace Demo.Core {
    public class Counter {
        private int initial;

        public Counter(int initial) {
            this.initial = initial;
        }

        public int Increment(int amount) {
            return initial + amount + 2;
        }
    }
}
"#;
    let new_method =
        "public int Increment(int amount) {\n            return initial + amount + 3;\n        }";
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&path, Some(overlay_source)).unwrap();

    let result = vfs
        .patch_node_at_position(
            &path,
            &Position {
                row: 10,
                column: 22,
            },
            new_method,
            None,
        )
        .unwrap();

    assert!(result.applied, "{result:#?}");
    let expected = overlay_source.replace(
        "public int Increment(int amount) {\n            return initial + amount + 2;\n        }",
        new_method,
    );
    assert_eq!(result.updated_source, expected);
    let snapshot = vfs.read_file(&path).unwrap();
    assert!(snapshot.dirty);
    assert_eq!(snapshot.source, expected);
    assert_eq!(fs::read_to_string(&path).unwrap(), CSHARP_SOURCE);
}

#[test]
fn rejects_invalid_csharp_replacements_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("Counter.cs");
    fs::write(&path, CSHARP_SOURCE).unwrap();

    let result = patch_ast_node_from_path(
        &path,
        "Demo::Core::Counter::Increment",
        "public int Increment(int amount) {\n            return initial + amount\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(!result.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), CSHARP_SOURCE);
}

#[test]
fn validates_csharp_method_patch_bindings_for_locals_fields_and_imports() {
    let source = r#"using System;
using Alias = Demo.Tools.Toolbox;

namespace Demo.Core {
    public class Counter {
        private const int LIMIT = 10;

        public static int Helper() {
            return 1;
        }

        public int Compute(int value) {
            return value + 1;
        }
    }
}

namespace Demo.Tools {
    public class Toolbox {
    }
}
"#;

    let replacement = r#"public int Compute(int value) {
    int bonus = value + 1;
    int total = Helper() + LIMIT + bonus;
    Helper().ToString();
    Alias.Go();
    return total;
}"#;
    let result = patch_ast_node(
        Path::new("Main.cs"),
        source,
        "Demo::Core::Counter::Compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    for name in ["value", "bonus", "total", "Helper", "LIMIT"] {
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
        .find(|decision| decision.name == "Helper")
        .unwrap();
    assert!(
        helper_decision.selected_symbol_id.as_deref().is_some_and(
            |id| id.contains("::csharp::Demo::Core::Counter::method_declaration::Helper")
        ),
        "{helper_decision:#?}"
    );
    let limit_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "LIMIT")
        .unwrap();
    assert!(
        limit_decision.selected_symbol_id.as_deref().is_some_and(
            |id| id.contains("::csharp::Demo::Core::Counter::field_declaration::LIMIT")
        ),
        "{limit_decision:#?}"
    );
    let alias_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "Alias")
        .unwrap();
    assert_eq!(alias_decision.status, "resolved");
    assert_eq!(
        alias_decision.candidates.first().unwrap().origin_type,
        "imported_module"
    );
}

#[test]
fn csharp_patch_binding_validation_respects_namespace_alias_scope() {
    let source = r#"namespace Demo.Tools {
    public class Toolbox {}
}

namespace Demo {
    using OuterAlias = Demo.Tools.Toolbox;

    namespace Core {
        public class Counter {
            public int Compute(int value) {
                return value;
            }
        }
    }
}

namespace Demo.Other {
    using SiblingAlias = Demo.Tools.Toolbox;

    public class Other {}
}
"#;
    let outer_alias = patch_ast_node(
        Path::new("Main.cs"),
        source,
        "Demo::Core::Counter::Compute",
        "public int Compute(int value) {
    return OuterAlias.Run(value);
}",
        None,
    )
    .unwrap();

    assert!(outer_alias.applied, "{outer_alias:#?}");
    assert!(
        outer_alias
            .validation
            .binding_decisions
            .iter()
            .any(|decision| {
                decision.name == "OuterAlias"
                    && decision.status == "resolved"
                    && decision
                        .selected_symbol_id
                        .as_deref()
                        .is_some_and(|symbol_id| {
                            symbol_id.contains("::Demo::using_directive::OuterAlias")
                        })
            })
    );

    let sibling_alias = patch_ast_node(
        Path::new("Main.cs"),
        source,
        "Demo::Core::Counter::Compute",
        "public int Compute(int value) {
    return SiblingAlias.Run(value);
}",
        None,
    )
    .unwrap();

    assert!(!sibling_alias.applied, "{sibling_alias:#?}");
    assert_eq!(
        sibling_alias.validation.unresolved_identifiers,
        ["SiblingAlias"]
    );
}

#[test]
fn rejects_csharp_method_patch_with_unresolved_identifier() {
    let source = r#"namespace Demo.Core {
    public class Counter {
        public int Compute(int value) {
            return value + 1;
        }
    }
}
"#;
    let replacement = r#"public int Compute(int value) {
    return missing(value);
}"#;
    let result = patch_ast_node(
        Path::new("Main.cs"),
        source,
        "Demo::Core::Counter::Compute",
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
fn rejects_csharp_method_patch_with_unresolved_receiver() {
    let source = r#"namespace Demo.Core {
    public class Counter {
        public int Compute(int value) {
            return value + 1;
        }
    }
}
"#;
    let replacement = r#"public int Compute(int value) {
    int missing = value + 1;
    return missing + other.count;
}"#;
    let result = patch_ast_node(
        Path::new("Main.cs"),
        source,
        "Demo::Core::Counter::Compute",
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
fn csharp_patch_binding_validation_resolves_scoped_patterns_and_closures() {
    let source = r#"using System;
using System.Collections.Generic;

namespace Demo.Core {
    public class Counter {
        public int Compute(List<int> values) {
            return values.Count;
        }

        private System.IO.Stream Open() {
            return null;
        }
    }
}
"#;

    let replacement = r#"public int Compute(List<int> values) {
    int total = 0;
    for (int i = 0; i < values.Count; i++) {
        total += values[i];
    }
    foreach (int value in values) {
        total += value;
    }
    if (values is List<int> list) {
        total += list.Count;
    }
    using (var resource = Open()) {
        total += resource.Length;
    }
    try {
        total += values[0];
    } catch (InvalidOperationException ex) {
        total += ex.Message.Length;
    }
    Func<int, int> apply = (extra) => extra + total;
    switch (total) {
        case int n when n > 0:
            total += n;
            break;
        default:
            total += 1;
            break;
    }
    int parsed = int.Parse("1");
    int.TryParse("2", out int parsed2);
    total += parsed + parsed2;
    int Doubled(int amount) => amount * 2;
    total += Doubled(total);
    total += apply(1);
    return total;
}"#;
    let result = patch_ast_node(
        Path::new("Main.cs"),
        source,
        "Demo::Core::Counter::Compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    for name in [
        "values", "total", "i", "value", "list", "resource", "Open", "ex", "apply", "extra", "n",
        "parsed", "parsed2", "Doubled", "amount",
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
fn csharp_patch_binding_validation_ignores_types_members_and_constructors() {
    let source = r#"namespace Demo.Core {
    public class Counter {
        public int Compute(int value) {
            return value;
        }
    }

    public class Widget {
        public Widget(int value) {}

        public static int Total() {
            return 1;
        }
    }
}
"#;

    let replacement = r#"public int Compute(int value) {
    Widget item = new Widget(value);
    Counter.Total();
    Widget.Total();
    MissingType.Go();
    int total = (int)value;
    object box = value as object;
    if (box is Point p) {
        total += p.X;
    }
    string name = nameof(Widget);
    System.Type type = typeof(Widget);
    int size = sizeof(int);
    return total + name.Length;
}"#;
    let result = patch_ast_node(
        Path::new("Main.cs"),
        source,
        "Demo::Core::Counter::Compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(
        result.validation.unresolved_identifiers,
        vec!["MissingType"]
    );
    // Member names, labels, type spellings, and `nameof`/`typeof`/`sizeof`
    // arguments are never reported as value references.
    for skipped in ["Total", "Length", "X", "int", "object", "Point"] {
        assert!(
            !result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == skipped),
            "expected no decision for `{skipped}`: {result:#?}"
        );
    }
    // Receiver types and local values still resolve normally.
    for name in ["value", "box", "name", "p", "Widget", "Counter"] {
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
fn csharp_patch_binding_validation_resolves_class_level_field_references() {
    let source = r#"namespace Demo.Core {
    public class Counter {
        private const int LIMIT = 10;
        private int total = LIMIT + 1;

        public int Compute(int value) {
            return value + total;
        }
    }
}
"#;

    let replacement = r#"public class Counter {
    private const int LIMIT = 10;
    private int total = LIMIT + 2;
    private int doubled = total * 2;

    public int Compute(int value) {
        return value + doubled;
    }
}"#;
    let result = patch_ast_node(
        Path::new("Main.cs"),
        source,
        "Demo::Core::Counter",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    for name in ["LIMIT", "total", "doubled"] {
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
fn csharp_patch_binding_validation_resolves_same_file_type_items_in_static_calls() {
    let source = r#"namespace Demo.Core {
    public class Counter {
        public static int Next(int value) {
            return value + 1;
        }

        public int Compute(int value) {
            return value;
        }
    }
}
"#;

    let replacement = r#"public int Compute(int value) {
    Counter.Next(value);
    return value;
}"#;
    let result = patch_ast_node(
        Path::new("Main.cs"),
        source,
        "Demo::Core::Counter::Compute",
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
            .is_some_and(|id| id.contains("::csharp::Demo::Core::class_declaration::Counter")),
        "{counter_decision:#?}"
    );
}

#[test]
fn csharp_patch_binding_validation_rejects_unknown_type_in_static_call() {
    let source = r#"namespace Demo.Core {
    public class Counter {
        public int Compute(int value) {
            return value;
        }
    }
}
"#;
    let replacement = r#"public int Compute(int value) {
    MissingType.Run();
    return value;
}"#;
    let result = patch_ast_node(
        Path::new("Main.cs"),
        source,
        "Demo::Core::Counter::Compute",
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
fn csharp_patch_binding_validation_resolves_record_components() {
    let source = r#"namespace Demo.Core {
    public record Entry(string Name) {
        public int Sum(int other) {
            return other + Name.Length;
        }
    }
}
"#;

    let replacement = r#"public int Sum(int other) {
    return Name.Length + other;
}"#;
    let result = patch_ast_node(
        Path::new("Entry.cs"),
        source,
        "Demo::Core::Entry::Sum",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers.len(), 0);
    for name in ["Name", "other"] {
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
