use super::*;

#[test]
fn applies_position_edits_in_sequence() {
    let file = temp_file("def value() -> int:\n    return 10\n");
    let mut vfs = VirtualFileSystem::new();

    let result = vfs
        .apply_position_edits(
            &file,
            &[
                PositionEdit {
                    start: Position { row: 1, column: 11 },
                    end: Position { row: 1, column: 13 },
                    new_text: "20".to_string(),
                },
                PositionEdit {
                    start: Position { row: 1, column: 0 },
                    end: Position { row: 1, column: 0 },
                    new_text: "# staged\n".to_string(),
                },
            ],
        )
        .unwrap();

    assert!(result.source.contains("return 20"));
    assert!(result.source.contains("# staged"));
    assert!(result.dirty);
}

#[test]
fn traces_symbol_graph_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return leaf(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.patch_node(
        &helper_path,
        "helper",
        "def helper(value: int) -> int:\n    return branch(value)\n",
        None,
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
        .unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "branch")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "leaf")
    );
    assert!(
        fs::read_to_string(&helper_path)
            .unwrap()
            .contains("return leaf")
    );
}
