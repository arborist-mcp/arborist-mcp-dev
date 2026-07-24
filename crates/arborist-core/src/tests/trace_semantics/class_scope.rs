use super::*;

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
