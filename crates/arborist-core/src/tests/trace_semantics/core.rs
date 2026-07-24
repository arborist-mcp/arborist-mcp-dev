use super::*;

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
