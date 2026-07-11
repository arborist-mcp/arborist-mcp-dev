use std::fs;
use std::path::Path;

use super::support::temporary_dir;
use super::{
    TraceDirection, rebuild_symbol_index, trace_symbol_graph, trace_symbol_graph_from_index,
};
#[test]
fn traces_symbol_graph_across_python_files() {
    let workspace_root = Path::new("../../tests/fixtures");
    let trace = trace_symbol_graph(workspace_root, "orchestrate", TraceDirection::Both).unwrap();

    assert_eq!(trace.symbol.semantic_path, "orchestrate");
    assert_eq!(trace.symbol.scope_path, None);
    assert_eq!(trace.symbol.parameters, vec!["value: int".to_string()]);
    assert_eq!(trace.symbol.return_type.as_deref(), Some("int"));
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.parameters == vec!["value: int".to_string()])
    );
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.return_type.as_deref() == Some("int"))
    );

    let leaf_trace = trace_symbol_graph(workspace_root, "leaf", TraceDirection::Callers).unwrap();
    assert!(
        leaf_trace
            .callers
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn ignores_python_local_variable_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("shadow.py");

    fs::write(
        &source,
        "def helper():\n    return 1\n\n\
def orchestrate():\n    helper = 2\n    return helper\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn traces_python_global_declared_references() {
    let dir = temporary_dir();
    let source = dir.join("global_decl.py");

    fs::write(
        &source,
        "def helper():\n    return 1\n\n\
def orchestrate():\n    global helper\n    helper = helper\n    return helper()\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn traces_python_default_parameter_references_despite_local_shadowing() {
    let dir = temporary_dir();
    let source = dir.join("default_param_shadow.py");

    fs::write(
        &source,
        "def helper():\n    return 1\n\n\
def orchestrate(value=helper()):\n    helper = 2\n    return value\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn ignores_python_nonlocal_outer_variable_references_in_nested_traces() {
    let dir = temporary_dir();
    let source = dir.join("nonlocal_outer_variable.py");

    fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 2\n\n    def inner():\n        nonlocal helper\n        return helper\n\n    return inner()\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn traces_python_nonlocal_outer_function_references_in_nested_traces() {
    let dir = temporary_dir();
    let source = dir.join("nonlocal_outer_function.py");

    fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    def helper():\n        return 2\n\n    def inner():\n        nonlocal helper\n        return helper()\n\n    return inner()\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "top_level.helper")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn traces_python_global_references_inside_nested_functions_despite_outer_shadowing() {
    let dir = temporary_dir();
    let source = dir.join("nested_global_shadow.py");

    fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 2\n\n    def inner():\n        global helper\n        return helper()\n\n    return inner()\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn ignores_python_post_except_target_global_fallback_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("post_except_target.py");

    fs::write(
            &source,
            "def exc():\n    return 1\n\n\
def risky():\n    raise ValueError()\n\n\
def orchestrate():\n    try:\n        risky()\n    except ValueError as exc:\n        return 0\n    return exc()\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "risky")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "exc")
    );
}

#[test]
fn ignores_python_pre_except_target_global_fallback_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("pre_except_target.py");

    fs::write(
            &source,
            "def exc():\n    return 1\n\n\
def risky():\n    raise ValueError()\n\n\
def orchestrate():\n    before = exc\n    try:\n        risky()\n    except ValueError as exc:\n        return before\n    return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "risky")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "exc")
    );
}

#[test]
fn ignores_python_named_expression_global_fallback_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("named_expression_shadow.py");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(flag):\n    before = target\n    if flag and (target := 3):\n        return before\n    return before\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_lambda_parameter_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("lambda_param_shadow.py");

    fs::write(
        &source,
        "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda target: target)(3)\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn traces_python_lambda_default_parameter_references() {
    let dir = temporary_dir();
    let source = dir.join("lambda_default.py");

    fs::write(
        &source,
        "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda x=target(): x)()\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "target")
    );
}

#[test]
fn traces_python_async_function_references() {
    let dir = temporary_dir();
    let source = dir.join("async_orchestrate.py");

    fs::write(
        &source,
        "def helper(value: int) -> int:\n    return value + 1\n\n\
async def orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn ignores_python_nested_default_parameter_closure_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("nested_default_param_shadow.py");

    fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 3\n\n    def inner(value=helper):\n        return value\n\n    return inner()\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_nested_lambda_parameter_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("nested_lambda_param_shadow.py");

    fs::write(
        &source,
        "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda target: (lambda: target)())(3)\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_class_lambda_local_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("class_lambda_shadow.py");

    fs::write(
        &source,
        "class Container:\n    helper = 2\n    value = (lambda: helper)()\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_class_method_default_parameter_local_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("class_method_default_shadow.py");

    fs::write(
        &source,
        "def helper():\n    return 1\n\n\
class Container:\n    helper = 2\n\n    def method(value=helper):\n        return value\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "Container.method", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_nested_decorator_closure_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("nested_decorator_shadow.py");

    fs::write(
            &source,
            "def helper(func):\n    return func\n\n\
def top_level():\n    helper = lambda func: func\n\n    @helper\n    def inner():\n        return 1\n\n    return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_class_method_decorator_local_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("class_method_decorator_shadow.py");

    fs::write(
        &source,
        "def helper(func):\n    return func\n\n\
class Container:\n    helper = helper\n\n    @helper\n    def method(self):\n        return 0\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "Container.method", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_nested_class_base_local_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("nested_class_base_shadow.py");

    fs::write(
        &source,
        "class GlobalBase:\n    pass\n\n\
def top_level():\n    Base = GlobalBase\n\n    class Inner(Base):\n        pass\n\n    return 0\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "top_level.Inner", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn traces_python_class_base_global_fallbacks() {
    let dir = temporary_dir();
    let source = dir.join("class_base_global.py");

    fs::write(
        &source,
        "class Base:\n    pass\n\n\
class Container(Base):\n    Base = 1\n    value = 0\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "Base")
    );
}

#[test]
fn ignores_python_nested_class_metaclass_local_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("nested_class_metaclass_shadow.py");

    fs::write(
            &source,
            "class GlobalMeta(type):\n    pass\n\n\
def top_level():\n    Meta = GlobalMeta\n\n    class Inner(metaclass=Meta):\n        pass\n\n    return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "top_level.Inner", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn traces_python_class_metaclass_global_fallbacks() {
    let dir = temporary_dir();
    let source = dir.join("class_metaclass_global.py");

    fs::write(
        &source,
        "class Meta(type):\n    pass\n\n\
class Container(metaclass=Meta):\n    Meta = 1\n    value = 0\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "Meta")
    );
}

#[test]
fn ignores_python_class_body_local_references_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("class_local_reference.py");

    fs::write(
        &source,
        "class Container:\n    helper = 2\n    value = helper\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn traces_python_class_comprehension_global_fallbacks() {
    let dir = temporary_dir();
    let source = dir.join("class_comprehension_global.py");

    fs::write(
        &source,
        "def helper():\n    return 1\n\n\
class Container:\n    helper = 2\n    value = [helper for item in range(1)]\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn collects_python_class_comprehension_global_reference_names() {
    let source = "def helper():\n    return 1\n\n\
class Container:\n    helper = 2\n    value = [helper for item in range(1)]\n";
    let document = crate::language::parse_document(Path::new("sample.py"), source).unwrap();
    let mut class_range = None;
    let mut callback = |node: tree_sitter::Node<'_>| {
        if node.kind() == "class_definition"
            && crate::semantic::semantic_path(node, source)
                .ok()
                .is_some_and(|path| path == "Container")
        {
            class_range = Some((node.start_byte(), node.end_byte()));
        }
    };
    crate::language::visit_tree(document.tree.root_node(), &mut callback);
    let (start, end) = class_range.unwrap();
    let class_node = document
        .tree
        .root_node()
        .descendant_for_byte_range(start, end)
        .unwrap();
    assert_eq!(class_node.kind(), "class_definition");

    let mut references = std::collections::BTreeSet::new();
    crate::patching::collect_python_references(
        Path::new("sample.py"),
        class_node,
        source,
        &mut references,
    )
    .unwrap();

    assert!(
        references.contains("helper"),
        "references: {:?}",
        references
    );
}

#[test]
fn traces_python_comprehension_call_references() {
    let dir = temporary_dir();
    let source = dir.join("comprehension.py");

    fs::write(
        &source,
        "def helper(value: int) -> int:\n    return value + 1\n\n\
def orchestrate(items: list[int]) -> list[int]:\n    return [helper(item) for item in items]\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn ignores_python_comprehension_target_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("comprehension_shadow.py");

    fs::write(
        &source,
        "def item():\n    return 1\n\n\
def orchestrate(values):\n    return [item for item in values]\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_match_capture_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_capture.py");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case {\"target\": target}:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_pre_match_capture_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_pre_capture.py");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    before = target\n    match value:\n        case {\"target\": target}:\n            return before\n        case _:\n            return before\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_match_alias_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_alias.py");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case int() as target:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_match_keyword_capture_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_keyword_capture.py");

    fs::write(
            &source,
            "class Point:\n    __match_args__ = ()\n\ndef target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case Point(value=target):\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "target")
    );
}

#[test]
fn ignores_python_match_splat_capture_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_splat_capture.py");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case [*target]:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_match_list_capture_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_list_capture.py");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case [target]:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_match_mapping_splat_capture_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_mapping_splat_capture.py");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case {\"x\": _, **target}:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn ignores_python_match_class_positional_capture_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_class_positional_capture.py");

    fs::write(
            &source,
            "class Point:\n    __match_args__ = (\"value\",)\n\n\
def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case Point(target):\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "Point")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "target")
    );
}

#[test]
fn ignores_python_match_union_capture_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_union_capture.py");

    fs::write(
            &source,
            "class Point:\n    __match_args__ = (\"value\",)\n\n\
class Value:\n    __match_args__ = (\"value\",)\n\n\
def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case Point(target) | Value(target):\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "Point")
    );
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "Value")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "target")
    );
}

#[test]
fn ignores_python_match_guard_global_fallback_after_prior_capture_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_guard_reference.py");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case [target]:\n            return 0\n        case _ if target():\n            return 1\n        case _:\n            return 2\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "target")
    );
}

#[test]
fn ignores_python_match_capture_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_capture.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case {\"target\": target}:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn ignores_python_pre_match_capture_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_pre_capture.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    before = target\n    match value:\n        case {\"target\": target}:\n            return before\n        case _:\n            return before\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn traces_python_default_parameter_references_despite_local_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("default_param_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "def helper():\n    return 1\n\n\
def orchestrate(value=helper()):\n    helper = 2\n    return value\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn traces_python_global_references_inside_nested_functions_despite_outer_shadowing_in_persisted_traces()
 {
    let dir = temporary_dir();
    let source = dir.join("nested_global_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 2\n\n    def inner():\n        global helper\n        return helper()\n\n    return inner()\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "top_level.inner", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn ignores_python_post_except_target_global_fallback_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("post_except_target.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "def exc():\n    return 1\n\n\
def risky():\n    raise ValueError()\n\n\
def orchestrate():\n    try:\n        risky()\n    except ValueError as exc:\n        return 0\n    return exc()\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "risky")
    );
    assert!(
        !live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "exc")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "risky")
    );
    assert!(
        !persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "exc")
    );
}

#[test]
fn ignores_python_pre_except_target_global_fallback_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("pre_except_target.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "def exc():\n    return 1\n\n\
def risky():\n    raise ValueError()\n\n\
def orchestrate():\n    before = exc\n    try:\n        risky()\n    except ValueError as exc:\n        return before\n    return 0\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "risky")
    );
    assert!(
        !live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "exc")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "risky")
    );
    assert!(
        !persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "exc")
    );
}

#[test]
fn ignores_python_named_expression_global_fallback_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("named_expression_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(flag):\n    before = target\n    if flag and (target := 3):\n        return before\n    return before\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn ignores_python_lambda_parameter_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("lambda_param_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda target: target)(3)\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn traces_python_lambda_default_parameter_references_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("lambda_default.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda x=target(): x)()\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "target")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "target")
    );
}

#[test]
fn traces_python_async_function_references_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("async_orchestrate.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "def helper(value: int) -> int:\n    return value + 1\n\n\
async def orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn ignores_python_nested_default_parameter_closure_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("nested_default_param_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 3\n\n    def inner(value=helper):\n        return value\n\n    return inner()\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "top_level.inner", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn ignores_python_nested_lambda_parameter_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("nested_lambda_param_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda target: (lambda: target)())(3)\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn ignores_python_class_method_default_parameter_local_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("class_method_default_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "def helper():\n    return 1\n\n\
class Container:\n    helper = 2\n\n    def method(value=helper):\n        return value\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "Container.method", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "Container.method", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn ignores_python_nested_decorator_closure_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("nested_decorator_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "def helper(func):\n    return func\n\n\
def top_level():\n    helper = lambda func: func\n\n    @helper\n    def inner():\n        return 1\n\n    return 0\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "top_level.inner", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn ignores_python_class_method_decorator_local_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("class_method_decorator_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "def helper(func):\n    return func\n\n\
class Container:\n    helper = helper\n\n    @helper\n    def method(self):\n        return 0\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "Container.method", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "Container.method", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn ignores_python_nested_class_base_local_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("nested_class_base_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "class GlobalBase:\n    pass\n\n\
def top_level():\n    Base = GlobalBase\n\n    class Inner(Base):\n        pass\n\n    return 0\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "top_level.Inner", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "top_level.Inner", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn traces_python_class_base_global_fallbacks_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("class_base_global.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "class Base:\n    pass\n\n\
class Container(Base):\n    Base = 1\n    value = 0\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "Base")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "Container", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "Base")
    );
}

#[test]
fn ignores_python_nested_class_metaclass_local_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("nested_class_metaclass_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "class GlobalMeta(type):\n    pass\n\n\
def top_level():\n    Meta = GlobalMeta\n\n    class Inner(metaclass=Meta):\n        pass\n\n    return 0\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "top_level.Inner", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "top_level.Inner", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn traces_python_class_metaclass_global_fallbacks_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("class_metaclass_global.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "class Meta(type):\n    pass\n\n\
class Container(metaclass=Meta):\n    Meta = 1\n    value = 0\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "Meta")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "Container", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "Meta")
    );
}

#[test]
fn ignores_python_class_lambda_local_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("class_lambda_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "class Container:\n    helper = 2\n    value = (lambda: helper)()\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "Container", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn ignores_python_class_body_local_references_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("class_local_reference.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "class Container:\n    helper = 2\n    value = helper\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "Container", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn traces_python_class_comprehension_global_fallbacks_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("class_comprehension_global.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "def helper():\n    return 1\n\n\
class Container:\n    helper = 2\n    value = [helper for item in range(1)]\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "Container", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn ignores_python_comprehension_target_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("comprehension_shadow.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source,
        "def item():\n    return 1\n\n\
def orchestrate(values):\n    return [item for item in values]\n",
    )
    .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn ignores_python_nonlocal_outer_variable_references_in_persisted_nested_traces() {
    let dir = temporary_dir();
    let source = dir.join("nonlocal_outer_variable.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 2\n\n    def inner():\n        nonlocal helper\n        return helper\n\n    return inner()\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();
    assert!(live_trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "top_level.inner", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn traces_python_nonlocal_outer_function_references_in_persisted_nested_traces() {
    let dir = temporary_dir();
    let source = dir.join("nonlocal_outer_function.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    def helper():\n        return 2\n\n    def inner():\n        nonlocal helper\n        return helper()\n\n    return inner()\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "top_level.helper")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "top_level.inner", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "top_level.helper")
    );
}

#[test]
fn ignores_python_match_class_positional_capture_shadowing_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_class_positional_capture.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "class Point:\n    __match_args__ = (\"value\",)\n\n\
def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case Point(target):\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "Point")
    );
    assert!(
        !live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "target")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "Point")
    );
    assert!(
        !persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "target")
    );
}

#[test]
fn ignores_python_match_guard_global_fallback_after_prior_capture_in_persisted_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_guard_reference.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case [target]:\n            return 0\n        case _ if target():\n            return 1\n        case _:\n            return 2\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        !live_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "target")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        !persisted_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "target")
    );
}

#[test]
fn ignores_python_match_mixed_mapping_capture_shadowing_in_traces() {
    let dir = temporary_dir();
    let source = dir.join("match_mixed_mapping_capture.py");

    fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case {\"x\": x, **target}:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(trace.callees.is_empty());
}

#[test]
fn traces_python_decorator_references() {
    let dir = temporary_dir();
    let source = dir.join("decorated.py");

    fs::write(
        &source,
        "def decorator(func):\n    return func\n\n\
@decorator\n\
def orchestrate(value: int) -> int:\n    return value\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "decorator")
    );
}

#[test]
fn traces_decorated_python_symbol_metadata_through_index() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &helper,
            "def decorator(func):\n    return func\n\n@decorator\ndef helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\ndef decorator(func):\n    return func\n\n@decorator\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let helper_source = fs::read_to_string(&helper).unwrap();
    let helper_text = "@decorator\ndef helper(value: int) -> int:\n    return value + 1";
    let helper_start = helper_source.find(helper_text).unwrap();
    let helper_end = helper_start + helper_text.len();

    let caller_source = fs::read_to_string(&caller).unwrap();
    let orchestrate_text =
        "@decorator\ndef orchestrate(value: int) -> int:\n    return helper(value)";
    let orchestrate_start = caller_source.find(orchestrate_text).unwrap();
    let orchestrate_end = orchestrate_start + orchestrate_text.len();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(live_trace.symbol.origin_type, "trace_root");
    assert_eq!(
        live_trace.symbol.evidence_key,
        live_trace.evidence_keys.symbol
    );
    assert_eq!(
        live_trace.symbol.signature.as_deref(),
        Some("@decorator\ndef orchestrate(value: int) -> int:")
    );
    assert_eq!(
        live_trace.symbol.byte_range,
        (orchestrate_start, orchestrate_end)
    );
    let live_helper = live_trace
        .callees
        .iter()
        .find(|symbol| symbol.semantic_path == "helper")
        .unwrap();
    assert_eq!(
        live_helper.signature.as_deref(),
        Some("@decorator\ndef helper(value: int) -> int:")
    );
    assert_eq!(live_helper.byte_range, (helper_start, helper_end));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.origin_type, "trace_root");
    assert_eq!(
        persisted_trace.symbol.evidence_key,
        persisted_trace.evidence_keys.symbol
    );
    assert_eq!(
        persisted_trace.symbol.signature.as_deref(),
        Some("@decorator\ndef orchestrate(value: int) -> int:")
    );
    assert_eq!(
        persisted_trace.symbol.byte_range,
        (orchestrate_start, orchestrate_end)
    );
    let persisted_helper = persisted_trace
        .callees
        .iter()
        .find(|symbol| symbol.semantic_path == "helper")
        .unwrap();
    assert_eq!(
        persisted_helper.signature.as_deref(),
        Some("@decorator\ndef helper(value: int) -> int:")
    );
    assert_eq!(persisted_helper.byte_range, (helper_start, helper_end));
}

#[test]
fn traces_python_alias_import_calls_across_files() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "import graph_b as gb\nfrom graph_b import helper as h\n\n\ndef orchestrate(value: int) -> int:\n    return gb.helper(value) + h(value)\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(live_trace.callees.len(), 1);
    assert_eq!(live_trace.callees[0].semantic_path, "helper");
    assert_eq!(
        live_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(persisted_trace.callees[0].semantic_path, "helper");
    assert_eq!(
        persisted_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_python_absolute_package_alias_import_calls_across_files() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let subpackage = package.join("sub");
    let helper = package.join("graph_c.py");
    let caller = subpackage.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&subpackage).unwrap();
    fs::write(package.join("__init__.py"), "").unwrap();
    fs::write(subpackage.join("__init__.py"), "").unwrap();
    fs::write(
        &helper,
        "def worker(value: int) -> int:\n    return value + 3\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "import pkg.graph_c as gc\nfrom pkg.graph_c import worker as w\n\n\ndef orchestrate(value: int) -> int:\n    return gc.worker(value) + w(value)\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(live_trace.callees.len(), 1);
    assert_eq!(live_trace.callees[0].semantic_path, "worker");
    assert_eq!(
        live_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(persisted_trace.callees[0].semantic_path, "worker");
    assert_eq!(
        persisted_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_python_import_from_module_alias_calls_across_files() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let subpackage = package.join("sub");
    let helper = package.join("graph_c.py");
    let local_helper = subpackage.join("local_mod.py");
    let caller = subpackage.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&subpackage).unwrap();
    fs::write(package.join("__init__.py"), "").unwrap();
    fs::write(subpackage.join("__init__.py"), "").unwrap();
    fs::write(
        &helper,
        "def worker(value: int) -> int:\n    return value + 3\n",
    )
    .unwrap();
    fs::write(
        &local_helper,
        "def helper2(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from pkg import graph_c as gc\nfrom . import local_mod as lm\n\n\ndef orchestrate(value: int) -> int:\n    return gc.worker(value) + lm.helper2(value)\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(live_trace.callees.len(), 2);
    assert!(live_trace.callees.iter().any(|symbol| {
        symbol.semantic_path == "worker"
            && symbol.file_path == helper.to_string_lossy().replace('\\', "/")
    }));
    assert!(live_trace.callees.iter().any(|symbol| {
        symbol.semantic_path == "helper2"
            && symbol.file_path == local_helper.to_string_lossy().replace('\\', "/")
    }));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 2);
    assert!(persisted_trace.callees.iter().any(|symbol| {
        symbol.semantic_path == "worker"
            && symbol.file_path == helper.to_string_lossy().replace('\\', "/")
    }));
    assert!(persisted_trace.callees.iter().any(|symbol| {
        symbol.semantic_path == "helper2"
            && symbol.file_path == local_helper.to_string_lossy().replace('\\', "/")
    }));
}

#[test]
fn traces_python_package_reexport_calls_across_files() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let helper = package.join("graph_c.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("__init__.py"),
        "from .graph_c import worker as worker\n",
    )
    .unwrap();
    fs::write(
        &helper,
        "def worker(value: int) -> int:\n    return value + 4\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from pkg import worker\n\n\ndef orchestrate(value: int) -> int:\n    return worker(value)\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(live_trace.callees.len(), 1);
    assert_eq!(live_trace.callees[0].semantic_path, "worker");
    assert_eq!(
        live_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(persisted_trace.callees[0].semantic_path, "worker");
    assert_eq!(
        persisted_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
}
