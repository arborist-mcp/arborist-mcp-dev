use super::*;

fn assert_zero_timeout<T>(result: anyhow::Result<T>) {
    let error = match result {
        Ok(_) => panic!("zero timeout should be rejected before VFS query setup"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("invalid trace timeout_ms: value must be greater than zero")
    );
}

#[test]
fn symbol_query_timeout_variants_validate_before_virtual_setup() {
    let workspace_root = Path::new("");
    let file_path = Path::new("");
    let position = Position { row: 0, column: 0 };
    let mut vfs = VirtualFileSystem::new();

    assert_zero_timeout(vfs.list_symbols_filtered_with_timeout(
        workspace_root,
        10,
        None,
        None,
        Some(0),
    ));
    assert_zero_timeout(vfs.list_symbols_context_filtered_with_timeout(
        workspace_root,
        10,
        None,
        None,
        Some(0),
    ));
    assert_zero_timeout(vfs.list_symbols_neighborhood_context_filtered_with_timeout(
        workspace_root,
        10,
        TraceDirection::Both,
        1,
        32,
        None,
        None,
        Some(0),
    ));
    assert_zero_timeout(vfs.list_symbols_discovery_context_filtered_with_timeout(
        workspace_root,
        10,
        TraceDirection::Both,
        1,
        32,
        None,
        None,
        Some(0),
    ));

    assert_zero_timeout(vfs.search_symbols_filtered_with_timeout(
        workspace_root,
        "query",
        10,
        None,
        None,
        Some(0),
    ));
    assert_zero_timeout(vfs.search_symbols_context_filtered_with_timeout(
        workspace_root,
        "query",
        10,
        None,
        None,
        Some(0),
    ));
    assert_zero_timeout(
        vfs.search_symbols_neighborhood_context_filtered_with_timeout(
            workspace_root,
            "query",
            10,
            TraceDirection::Both,
            1,
            32,
            None,
            None,
            Some(0),
        ),
    );
    assert_zero_timeout(vfs.search_symbols_discovery_context_filtered_with_timeout(
        workspace_root,
        "query",
        10,
        TraceDirection::Both,
        1,
        32,
        None,
        None,
        Some(0),
    ));

    assert_zero_timeout(vfs.read_symbol_with_timeout(workspace_root, "helper", Some(0)));
    assert_zero_timeout(vfs.read_symbol_at_position_with_timeout(
        workspace_root,
        file_path,
        &position,
        Some(0),
    ));
    assert_zero_timeout(vfs.read_symbol_context_with_timeout(
        workspace_root,
        "helper",
        TraceDirection::Both,
        Some(0),
    ));
    assert_zero_timeout(vfs.read_symbol_context_at_position_with_timeout(
        workspace_root,
        file_path,
        &position,
        TraceDirection::Both,
        Some(0),
    ));
    assert_zero_timeout(vfs.read_symbol_neighborhood_context_with_timeout(
        workspace_root,
        "helper",
        TraceDirection::Both,
        1,
        32,
        Some(0),
    ));
    assert_zero_timeout(
        vfs.read_symbol_neighborhood_context_at_position_with_timeout(
            workspace_root,
            file_path,
            &position,
            TraceDirection::Both,
            1,
            32,
            Some(0),
        ),
    );
    assert_zero_timeout(vfs.read_symbol_discovery_context_with_timeout(
        workspace_root,
        "helper",
        TraceDirection::Both,
        1,
        32,
        Some(0),
    ));
    assert_zero_timeout(vfs.read_symbol_discovery_context_at_position_with_timeout(
        workspace_root,
        file_path,
        &position,
        TraceDirection::Both,
        1,
        32,
        Some(0),
    ));

    assert_zero_timeout(vfs.trace_symbol_graph_with_timeout(
        workspace_root,
        "helper",
        TraceDirection::Both,
        Some(0),
    ));
    assert_zero_timeout(vfs.trace_symbol_neighborhood_with_timeout(
        workspace_root,
        "helper",
        TraceDirection::Both,
        1,
        32,
        Some(0),
    ));
    assert_zero_timeout(vfs.trace_symbol_graph_at_position_with_timeout(
        workspace_root,
        file_path,
        &position,
        TraceDirection::Both,
        Some(0),
    ));
    assert_zero_timeout(vfs.trace_symbol_neighborhood_at_position_with_timeout(
        workspace_root,
        file_path,
        &position,
        TraceDirection::Both,
        1,
        32,
        Some(0),
    ));
}
