use super::*;

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
