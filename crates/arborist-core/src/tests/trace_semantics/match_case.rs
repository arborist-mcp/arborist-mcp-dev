use super::*;

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
