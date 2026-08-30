use std::fs;
use std::path::Path;

use super::{
    Position, VirtualFileSystem, patch_ast_node, patch_ast_node_at_position,
    patch_ast_node_from_path, preview_patch_ast_node_from_path, temporary_dir,
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
fn patches_exported_javascript_generator_variables_by_semantic_path() {
    let source = "export const sequence = function* (value) { yield value; };\n";
    let replacement = "export const sequence = function* (value) { yield value + 1; };";
    let result = patch_ast_node(
        Path::new("sample.js"),
        source,
        "sequence",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.resolved_symbol_id, "sequence");
    assert_eq!(result.updated_source, format!("{replacement}\n"));
}

#[test]
fn patches_typescript_namespace_functions_by_qualified_semantic_path() {
    let source = r#"namespace first {
    export function helper(value: number): number {
        return value + 1;
    }
}

namespace second {
    export function helper(value: number): number {
        return value + 2;
    }
}
"#;
    let replacement = r#"export function helper(value: number): number {
    return value + 3;
}"#;
    let result = patch_ast_node(
        Path::new("sample.ts"),
        source,
        "first::helper",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.resolved_symbol_id, "first::helper");
    assert!(result.updated_source.contains("return value + 3;"));
    assert!(result.updated_source.contains("return value + 2;"));
}

#[test]
fn javascript_patch_binding_validation_resolves_typescript_namespace_siblings() {
    let source = r#"namespace utilities {
    export function helper(value: number): number {
        return value + 1;
    }

    export function target(value: number): number {
        return helper(value);
    }
}
"#;
    let replacement = r#"export function target(value: number): number {
    return helper(value) + 1;
}"#;
    let result = patch_ast_node(
        Path::new("sample.ts"),
        source,
        "utilities::target",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    let helper = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "helper")
        .expect("namespace helper should resolve");
    assert_eq!(helper.symbol.semantic_path, "utilities::helper");
    assert_eq!(helper.symbol.scope_path.as_deref(), Some("utilities"));
}

#[test]
fn javascript_patch_binding_validation_resolves_parent_typescript_namespace_bindings() {
    let source = r#"namespace outer {
    export function helper(value: number): number {
        return value + 1;
    }

    export namespace inner {
        export function target(value: number): number {
            return helper(value);
        }
    }
}

namespace unrelated {
    export function helper(value: number): number {
        return value + 2;
    }
}
"#;
    let replacement = r#"export function target(value: number): number {
    return helper(value) + 1;
}"#;
    let result = patch_ast_node(
        Path::new("sample.ts"),
        source,
        "outer::inner::target",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    let helper = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "helper")
        .expect("outer namespace helper should resolve");
    assert_eq!(helper.symbol.semantic_path, "outer::helper");
    assert_eq!(helper.symbol.scope_path.as_deref(), Some("outer"));
}

#[test]
fn javascript_patch_binding_validation_resolves_exported_nested_namespace_siblings() {
    let source = r#"namespace outer {
    export namespace inner {
        export function helper(value: number): number {
            return value + 1;
        }

        export function target(value: number): number {
            return helper(value);
        }
    }
}
"#;
    let replacement = r#"export function target(value: number): number {
    return helper(value) + 1;
}"#;
    let result = patch_ast_node(
        Path::new("sample.ts"),
        source,
        "outer::inner::target",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    let helper = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "helper")
        .expect("nested namespace helper should resolve");
    assert_eq!(helper.symbol.semantic_path, "outer::inner::helper");
    assert_eq!(helper.symbol.scope_path.as_deref(), Some("outer::inner"));
}

#[test]
fn previews_typescript_patch_success_and_rejection_without_writing_the_source_file() {
    let dir = temporary_dir();
    let path = dir.join("sample.ts");
    fs::write(&path, TYPESCRIPT_SOURCE).unwrap();

    let replacement = "export function helper(value: number): number { return value + 2; }";
    let preview = preview_patch_ast_node_from_path(&path, "helper", replacement, None).unwrap();
    assert!(preview.patch.applied, "{preview:#?}");
    assert!(preview.changed);
    assert!(preview.unified_diff.contains("-export function helper"));
    assert!(preview.unified_diff.contains("+export function helper"));
    assert_eq!(fs::read_to_string(&path).unwrap(), TYPESCRIPT_SOURCE);

    let rejected = preview_patch_ast_node_from_path(
        &path,
        "helper",
        "export function helper(value: number): number { return value + 2;\n",
        None,
    )
    .unwrap();
    assert!(!rejected.patch.applied);
    assert!(!rejected.patch.validation.syntax_errors.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), TYPESCRIPT_SOURCE);
}

#[test]
fn patches_dirty_typescript_virtual_source_without_writing_disk() {
    let dir = temporary_dir();
    let path = dir.join("sample.ts");
    fs::write(&path, TYPESCRIPT_SOURCE).unwrap();

    let overlay_source = "export function helper(value: number): number { return value + 3; }\n";
    let replacement = "export function helper(value: number): number { return value + 4; }";
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&path, Some(overlay_source)).unwrap();

    let result = vfs
        .patch_node_at_position(&path, &Position { row: 0, column: 16 }, replacement, None)
        .unwrap();

    assert!(result.applied, "{result:#?}");
    assert_eq!(result.updated_source, format!("{replacement}\n"));
    let snapshot = vfs.read_file(&path).unwrap();
    assert!(snapshot.dirty);
    assert_eq!(snapshot.source, format!("{replacement}\n"));
    assert_eq!(fs::read_to_string(&path).unwrap(), TYPESCRIPT_SOURCE);
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

#[test]
fn validates_javascript_patch_bindings_for_params_locals_destructuring_and_imports() {
    let source = r#"import { helper as h } from "./util";
import other from "./other";
const limit = 10;

function pair() {
    return { x: 1, y: 2 };
}

function compute(value, { a, b = 1 } = {}) {
    const list = [1, 2, 3];
    const total = h(value) + other(a) + limit + b;
    return total;
}
"#;
    let replacement = r#"function compute(value, { a, b = 1 } = {}) {
    const list = [1, 2, 3];
    const { x, y: renamed } = pair();
    const mapped = list.map((item) => item * 2);
    const total = h(value) + other(a) + limit + b + x + renamed + mapped.length;
    return total;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    for name in [
        "value", "a", "b", "list", "x", "renamed", "item", "mapped", "pair", "h", "other", "limit",
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
    let h_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "h")
        .unwrap();
    assert_eq!(
        h_decision.candidates.first().unwrap().origin_type,
        "imported_module"
    );
    let limit_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "limit")
        .unwrap();
    assert_eq!(
        limit_decision.candidates.first().unwrap().origin_type,
        "module_scope"
    );
    assert!(
        result.validation.unresolved_identifiers.is_empty(),
        "{result:#?}"
    );
}

#[test]
fn rejects_javascript_patch_with_unresolved_identifier() {
    let source = "function compute(value) {\n    return value + 1;\n}\n";
    let replacement = "function compute(value) {\n    return missing(value);\n}";
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
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
fn validates_javascript_scoped_loop_catch_and_arrow_bindings() {
    let source = r#"function risky() {
    return 0;
}

function log(value) {
    return value;
}

function compute(list) {
    let total = 0;
    for (const item of list) {
        total += item;
    }
    for (let i = 0; i < 3; i++) {
        total += i;
    }
    try {
        risky();
    } catch (err) {
        total += log(err);
    }
    list.forEach((entry, index) => {
        total += entry + index;
    });
    const pairs = [[1, 2]];
    for (const [head, ...tail] of pairs) {
        total += head + tail.length;
    }
    return total;
}
"#;
    let replacement = r#"function compute(list) {
    let total = 0;
    for (const item of list) {
        total += item;
    }
    for (let i = 0; i < 3; i++) {
        total += i;
    }
    try {
        risky();
    } catch (err) {
        total += log(err);
    }
    list.forEach((entry, index) => {
        total += entry + index;
    });
    const pairs = [[1, 2]];
    for (const [head, ...tail] of pairs) {
        total += head + tail.length;
    }
    return total;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    for name in [
        "list", "total", "item", "i", "err", "entry", "index", "pairs", "head", "tail", "risky",
        "log",
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
    assert!(
        result.validation.unresolved_identifiers.is_empty(),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_ignores_member_names_keys_labels_and_types() {
    let source = r#"interface Config {
    label: string;
}

function outer() {
    const config = { name: "x", value: 1 };
    let total = 0;
    loop: for (const key of ["a", "b"]) {
        total += config.value + key.length;
    }
    return total;
}
"#;
    let replacement = r#"function outer(): number {
    const config = { name: "x", value: 1 };
    const value: number = config.value;
    let total = 0;
    loop: for (const key of ["a", "b"]) {
        total += value + key.length;
    }
    return total;
}"#;
    let result = patch_ast_node(Path::new("outer.ts"), source, "outer", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    for name in ["config", "value", "key", "total"] {
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == name && decision.status == "resolved"),
            "expected resolved decision for {name}: {result:#?}"
        );
    }
    assert!(
        result.validation.unresolved_identifiers.is_empty(),
        "{result:#?}"
    );
}

#[test]
fn typescript_patch_binding_validation_resolves_class_fields_and_imports() {
    let source = r#"import { helper } from "./util";
const defaultCount = 5;

class Counter {
    private count: number = defaultCount;

    constructor(private initial: number) {
        this.count = initial;
    }

    increment(amount: number): number {
        const bonus: number = amount + 1;
        return this.count + bonus + helper(this.initial);
    }
}
"#;
    let replacement = r#"class Counter {
    private count: number = defaultCount;

    constructor(private initial: number) {
        this.count = initial;
    }

    increment(amount: number): number {
        const bonus: number = amount + 1;
        return this.count + bonus + helper(this.initial);
    }

    doubled(): number {
        const base = this.count + 1;
        return helper(base) * 2;
    }
}"#;
    let result = patch_ast_node(
        Path::new("counter.ts"),
        source,
        "Counter",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    for name in ["amount", "bonus", "base", "helper", "defaultCount"] {
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
    assert_eq!(
        helper_decision.candidates.first().unwrap().origin_type,
        "imported_module"
    );
    assert!(
        result.validation.unresolved_identifiers.is_empty(),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_resolves_nested_function_declarations() {
    let source = r#"function outer(value) {
    function inner(input) {
        return input + 1;
    }
    return inner(value);
}
"#;
    let replacement = r#"function outer(value) {
    function inner(input) {
        return input + 1;
    }
    return inner(value) + outer(value - 1);
}"#;
    let result = patch_ast_node(Path::new("outer.js"), source, "outer", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    for name in ["value", "inner", "input", "outer"] {
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == name && decision.status == "resolved"),
            "expected resolved decision for {name}: {result:#?}"
        );
    }
    assert!(
        result.validation.unresolved_identifiers.is_empty(),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_resolves_hoisted_nested_function_declarations() {
    let source = r#"function outer(value) {
    return value;
}
"#;
    let replacement = r#"function outer(value) {
    const result = inner(value);
    function inner(input) {
        return input + 1;
    }
    return result;
}"#;
    let result = patch_ast_node(Path::new("outer.js"), source, "outer", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn javascript_patch_binding_validation_resolves_hoisted_var_bindings() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const previous = helper;
    var helper = value + 1;
    return previous;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn javascript_patch_binding_validation_rejects_nested_function_var_scope_escapes() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    function nested() {
        var hidden = value + 1;
        return hidden;
    }
    nested();
    return hidden;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["hidden"]);
    assert!(
        result
            .validation
            .resolved_identifiers
            .iter()
            .all(|binding| binding.name != "hidden"),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_rejects_lexical_tdz_references() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = helper(value);
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
    assert!(
        result
            .validation
            .resolved_identifiers
            .iter()
            .all(|binding| binding.name != "helper"),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_rejects_outer_bindings_shadowed_by_block_tdz() {
    let source = r#"function compute(helper) { return helper; }"#;
    let replacement = r#"function compute(helper) {
    {
        console.log(helper);
        let helper = 1;
    }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_resolves_lexical_bindings_captured_by_initializers() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = () => helper;
    return helper();
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn javascript_patch_binding_validation_rejects_type_assertion_iife_tdz_captures() {
    let source = r#"function compute() {
    return 0;
}
"#;
    let replacement = r#"function compute() {
    const helper = (<() => number>function () {
        return helper;
    })();
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.ts"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_non_null_iife_tdz_captures() {
    let source = r#"function compute() {
    return 0;
}
"#;
    let replacement = r#"function compute() {
    const helper = (function () {
        return helper;
    })!();
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.ts"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_immediately_invoked_lexical_captures() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = (() => helper)();
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_lexical_captures_in_function_constructors() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = new (function () {
        return helper;
    })();
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_lexical_captures_in_inline_class_constructors() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = new class {
        constructor() {
            return helper;
        }
    }();
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_lexical_references_in_computed_method_keys() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = class {
        [helper]() {
            return value;
        }
    };
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_lexical_references_in_computed_object_keys() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = { [helper]: value };
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_lexical_references_in_computed_field_keys() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = class {
        [helper] = value;
    };
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_resolves_initialized_computed_property_keys() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const key = "result";
    const helper = { [key]: value };
    return helper[key];
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "key" && decision.status == "resolved"),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_allows_deferred_instance_field_initializers() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = class {
        result = helper;
    };
    return new helper().result;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn javascript_patch_binding_validation_rejects_eager_static_field_initializers() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = class {
        static result = helper;
    };
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_inline_constructed_instance_field_initializers() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = new class {
        result = helper;
    }();
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_uses_outer_bindings_for_loop_assignment_targets() {
    let source = r#"function compute(entry) {
    return entry;
}
"#;
    let replacement = r#"function compute(entry) {
    for (entry of entry) {
        continue;
    }
    return entry;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    let decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "entry")
        .expect("entry should resolve to the function parameter");
    assert_eq!(decision.candidates[0].node_kind, "parameter", "{result:#?}");
}
#[test]
fn javascript_patch_binding_validation_rejects_unresolved_loop_assignment_targets() {
    let source = r#"function compute(items) {
    return 0;
}
"#;
    let replacement = r#"function compute(items) {
    for (missing of items) {
        continue;
    }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["missing"]);
}
#[test]
fn javascript_patch_binding_validation_rejects_unresolved_computed_loop_assignment_keys() {
    let source = r#"function compute(items, target) {
    return target;
}
"#;
    let replacement = r#"function compute(items, target) {
    for ({ [missing]: target.value } of items) {
        continue;
    }
    return target;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["missing"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_unresolved_destructuring_assignment_targets() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    ({ missing } = value);
    return value;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["missing"]);
}

#[test]
fn javascript_patch_binding_validation_resolves_destructuring_assignment_targets() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    let target;
    ({ target } = value);
    return target;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(
        result.validation.unresolved_identifiers.is_empty(),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_rejects_references_outside_switch_scope() {
    let source = r#"function compute(value) { return value; }"#;
    let replacement = r#"function compute(value) {
    switch (value) {
        case 1:
            let helper = 1;
            break;
        default:
            break;
    }
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_switch_case_label_tdz_references() {
    let source = r#"function compute(helper) { return helper; }"#;
    let replacement = r#"function compute(helper) {
    switch (1) {
        case helper:
            let helper = 2;
            return helper;
        default:
            return 0;
    }
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_hoists_switch_case_function_declarations() {
    let source = r#"function compute(value) { return value; }"#;
    let replacement = r#"function compute(value) {
    switch (value) {
        case 0:
            return helper();
        case 1:
            function helper() {
                return 1;
            }
    }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    let decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "helper")
        .expect("helper should resolve to the switch-scoped function declaration");
    assert_eq!(decision.candidates[0].node_kind, "function_declaration");
}

#[test]
fn javascript_patch_binding_validation_evaluates_switch_discriminants_outside_case_scope() {
    let source = r#"function compute(helper) { return helper; }"#;
    let replacement = r#"function compute(helper) {
    switch (helper) {
        case 1:
            let helper = 2;
            return helper;
        default:
            return 0;
    }
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    let decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "helper")
        .expect("helper should resolve to the function parameter in the discriminant");
    assert_eq!(decision.candidates[0].node_kind, "parameter");
}

#[test]
fn javascript_patch_binding_validation_rejects_switch_case_tdz_references() {
    let source = r#"function compute(value) { return value; }"#;
    let replacement = r#"function compute(value) {
    switch (value) {
        case 1:
            let helper = helper;
            return helper;
        default:
            return value;
    }
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();
    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_later_parameter_default_references() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute(first = second, second = 1) {
    return first + second;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["second"]);
}

#[test]
fn typescript_patch_binding_validation_rejects_later_parameter_default_references() {
    let source = r#"function compute(): number { return 0; }"#;
    let replacement = r#"function compute(first: number = second, second: number = 1): number {
    return first + second;
}"#;
    let result = patch_ast_node(
        Path::new("compute.ts"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["second"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_using_initializer_tdz_references() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute() {
    using value = value;
    return value;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["value"]);
    assert!(
        result
            .validation
            .resolved_identifiers
            .iter()
            .all(|binding| binding.name != "value"),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_rejects_await_using_initializer_tdz_references() {
    let source = r#"async function compute() { return 0; }"#;
    let replacement = r#"async function compute() {
    await using value = value;
    return value;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["value"]);
    assert!(
        result
            .validation
            .resolved_identifiers
            .iter()
            .all(|binding| binding.name != "value"),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_resolves_initialized_using_bindings() {
    let source = r#"function compute(resource) { return resource; }"#;
    let replacement = r#"function compute(resource) {
    using value = resource;
    return value;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn javascript_patch_binding_validation_rejects_later_catch_destructuring_bindings() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute() {
    try {
        throw {};
    } catch ({ first = second, second = 1 }) {
        return first + second;
    }
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["second"]);
    assert!(
        result
            .validation
            .resolved_identifiers
            .iter()
            .all(|binding| binding.name != "second"),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_resolves_prior_catch_destructuring_bindings() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute() {
    try {
        throw {};
    } catch ({ first = 1, second = first }) {
        return first + second;
    }
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn javascript_patch_binding_validation_rejects_body_var_in_parameter_defaults() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute(value = hidden) {
    var hidden = 1;
    return value;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["hidden"]);
}

#[test]
fn javascript_patch_binding_validation_resolves_prior_parameter_default_references() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute(first = 1, second = first) {
    return first + second;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn javascript_patch_binding_validation_rejects_unresolved_destructured_loop_assignment_targets() {
    let source = r#"function compute(items) { return 0; }"#;
    let replacement = r#"function compute(items) {
    for ({ missing } of items) { continue; }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();
    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["missing"]);
}
#[test]
fn javascript_patch_binding_validation_resolves_var_loop_bindings_after_the_loop() {
    let source = r#"function compute(items) {
    return 0;
}
"#;
    let replacement = r#"function compute(items) {
    for (var item of items) {
        continue;
    }
    return item;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
}
#[test]
fn javascript_patch_binding_validation_rejects_lexical_references_in_loop_iterables() {
    let source = r#"function compute() {
    return [];
}
"#;
    let replacement = r#"function compute() {
    for (const entry of entry) {
        return entry;
    }
    return [];
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["entry"]);
    assert!(
        result
            .validation
            .resolved_identifiers
            .iter()
            .all(|binding| binding.name != "entry"),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_rejects_lexical_references_in_async_loop_iterables() {
    let source = r#"async function compute() {
    return [];
}
"#;
    let replacement = r#"async function compute() {
    for await (const entry of entry) {
        return entry;
    }
    return [];
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["entry"]);
}

#[test]
fn javascript_patch_binding_validation_resolves_prior_loop_destructuring_bindings() {
    let source = r#"function compute(pairs) {
    return 0;
}
"#;
    let replacement = r#"function compute(pairs) {
    for (const [head, tail = head] of pairs) {
        return tail;
    }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn javascript_patch_binding_validation_rejects_later_loop_destructuring_bindings() {
    let source = r#"function compute(pairs) {
    return 0;
}
"#;
    let replacement = r#"function compute(pairs) {
    for (const [head = tail, tail] of pairs) {
        return head;
    }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["tail"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_lexical_references_in_class_heritage() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const helper = class extends helper {};
    return helper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_resolves_named_function_expression_bindings() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute() {
    const factory = function Hidden(value = Hidden) {
        return value;
    };
    return factory();
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    let decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "Hidden")
        .expect("Hidden should resolve to the named function expression binding");
    assert_eq!(decision.candidates[0].node_kind, "function_expression");
}

#[test]
fn javascript_patch_binding_validation_keeps_named_function_expressions_scoped() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute() {
    const factory = function Hidden() {
        return 1;
    };
    return Hidden;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["Hidden"]);
}

#[test]
fn javascript_patch_binding_validation_resolves_named_generator_function_bindings() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute() {
    const factory = function* Hidden() {
        yield Hidden;
    };
    return factory;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    let decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "Hidden")
        .expect("Hidden should resolve to the named generator function binding");
    assert_eq!(decision.candidates[0].node_kind, "generator_function");
}

#[test]
fn javascript_patch_binding_validation_rejects_static_block_var_scope_leaks() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute() {
    class Container {
        static {
            var hidden = 1;
        }
    }
    return hidden;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["hidden"]);
}

#[test]
fn javascript_patch_binding_validation_hoists_static_block_var_bindings() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute() {
    class Container {
        static {
            console.log(hidden);
            var hidden = 1;
        }
    }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    let decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "hidden")
        .expect("hidden should resolve to the static-block var binding");
    assert_eq!(decision.candidates[0].node_kind, "variable_declarator");
}

#[test]
fn javascript_patch_binding_validation_rejects_static_block_lexical_tdz_references() {
    let source = r#"function compute(hidden) { return hidden; }"#;
    let replacement = r#"function compute(hidden) {
    class Container {
        static {
            console.log(hidden);
            let hidden = 1;
        }
    }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["hidden"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_class_declaration_heritage_tdz_references() {
    let source = r#"function compute(helper) { return helper; }"#;
    let replacement = r#"function compute(helper) {
    {
        class helper extends helper {}
    }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_rejects_named_class_expression_heritage_tdz_references() {
    let source = r#"function compute(helper) { return helper; }"#;
    let replacement = r#"function compute(helper) {
    const Wrapper = class helper extends helper {};
    return Wrapper;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn javascript_patch_binding_validation_keeps_named_class_expressions_in_class_scope() {
    let source = r#"function compute(value) { return value; }"#;
    let replacement = r#"function compute(value) {
    const Wrapper = class Hidden {
        static create() {
            return new Hidden();
        }
    };
    return Hidden;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["Hidden"]);
}

#[test]
fn javascript_patch_binding_validation_resolves_named_class_expression_body_references() {
    let source = r#"function compute() { return 0; }"#;
    let replacement = r#"function compute() {
    const Wrapper = class Hidden {
        static create() {
            return new Hidden();
        }
    };
    return Wrapper.create();
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn javascript_patch_binding_validation_resolves_initialized_class_heritage() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const Base = class {};
    const helper = class extends Base {};
    return new helper();
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "Base" && decision.status == "resolved"),
        "{result:#?}"
    );
}

#[test]
fn javascript_patch_binding_validation_resolves_prior_lexical_declarators() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    const first = value + 1,
        second = first + 1;
    return second;
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(result.validation.unresolved_identifiers.is_empty());
}

#[test]
fn javascript_patch_binding_validation_rejects_references_outside_nested_block_scope() {
    let source = r#"function compute(value) {
    return value;
}
"#;
    let replacement = r#"function compute(value) {
    {
        const helper = (input) => input + 1;
        helper(value);
    }
    return helper(value);
}"#;
    let result = patch_ast_node(
        Path::new("compute.js"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["helper"]);
}

#[test]
fn typescript_patch_binding_validation_resolves_local_enum_values() {
    let source = r#"function compute(): number { return 0; }"#;
    let replacement = r#"function compute(): number {
    enum Mode {
        Active = 0,
        Next = Mode.Active + 1,
    }
    return Mode.Next;
}"#;
    let result = patch_ast_node(
        Path::new("compute.ts"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(result.applied, "{result:#?}");
    let decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "Mode")
        .expect("Mode should resolve to the local enum declaration");
    assert_eq!(decision.candidates[0].node_kind, "enum_declaration");
}

#[test]
fn typescript_patch_binding_validation_rejects_unresolved_enum_member_initializers() {
    let source = r#"function compute(): number { return 0; }"#;
    let replacement = r#"function compute(): number {
    enum Mode {
        Active = missing,
    }
    return Mode.Active;
}"#;
    let result = patch_ast_node(
        Path::new("compute.ts"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["missing"]);
}

#[test]
fn typescript_patch_binding_validation_rejects_unresolved_class_decorators() {
    let source = r#"function compute(): number { return 0; }"#;
    let replacement = r#"function compute(): number {
    @missing
    class Thing {}
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.ts"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["missing"]);
}

#[test]
fn typescript_patch_binding_validation_rejects_unresolved_member_decorators() {
    let source = r#"function compute(): number { return 0; }"#;
    let replacement = r#"function compute(): number {
    class Thing {
        @missingMethod
        method() {}
        @missingField
        field = 1;
    }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.ts"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(
        result.validation.unresolved_identifiers,
        ["missingField", "missingMethod"]
    );
}

#[test]
fn typescript_patch_binding_validation_rejects_unresolved_parameter_decorators() {
    let source = r#"function compute(): number { return 0; }"#;
    let replacement = r#"function compute(): number {
    class Thing {
        method(@missingParameter value: number) {}
    }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.ts"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(
        result.validation.unresolved_identifiers,
        ["missingParameter"]
    );
}

#[test]
fn tsx_patch_binding_validation_ignores_jsx_namespace_names() {
    let source = r#"function App() { return <main />; }"#;
    let replacement = r#"function App() { return <svg:rect />; }"#;
    let result = patch_ast_node(Path::new("app.tsx"), source, "App", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    assert!(
        result.validation.unresolved_identifiers.is_empty(),
        "{result:#?}"
    );
}

#[test]
fn typescript_patch_binding_validation_rejects_enum_tdz_shadowing() {
    let source = r#"function compute(Mode: number): number { return Mode; }"#;
    let replacement = r#"function compute(Mode: number): number {
    {
        console.log(Mode);
        enum Mode { Active }
    }
    return 0;
}"#;
    let result = patch_ast_node(
        Path::new("compute.ts"),
        source,
        "compute",
        replacement,
        None,
    )
    .unwrap();

    assert!(!result.applied, "{result:#?}");
    assert_eq!(result.validation.unresolved_identifiers, ["Mode"]);
}

#[test]
fn tsx_patch_binding_validation_ignores_jsx_tag_and_attribute_names() {
    let source = r#"function format(value: number): string {
    return String(value);
}

export function App(props: { name: string }) {
    const items = [1, 2];
    return <main className="x">{items.map((item) => <span key={item}>{item}</span>)}</main>;
}
"#;
    let replacement = r#"export function App(props: { name: string }) {
    const items = [1, 2];
    return <main className="x">{items.map((item) => <span key={item}>{format(item)}</span>)}</main>;
}"#;
    let result = patch_ast_node(Path::new("app.tsx"), source, "App", replacement, None).unwrap();

    assert!(result.applied, "{result:#?}");
    for name in ["items", "item", "format"] {
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == name && decision.status == "resolved"),
            "expected resolved decision for {name}: {result:#?}"
        );
    }
    assert!(
        result.validation.unresolved_identifiers.is_empty(),
        "{result:#?}"
    );
}
