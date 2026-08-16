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
            public Inner(int value) {}
            public int Value() => value;
        }

        public Outer() {}
    }
}
"#;
    let nested_result = patch_ast_node(
        Path::new("Nested.cs"),
        source,
        "Demo::Core::Outer::Inner",
        "public class Inner {\n            public Inner(int value) {}\n            public int Value() => value;\n            public int Double() => value * 2;\n        }",
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
        "[Serializable]\n    public class Outer {\n        public class Inner {\n            public Inner(int value) {}\n            public int Value() => value;\n        }\n\n        public Outer() {}\n    }",
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
