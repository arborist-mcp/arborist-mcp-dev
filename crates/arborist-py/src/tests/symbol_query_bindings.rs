use super::*;

#[test]
fn direct_read_bindings_forward_zero_timeout_before_query_work() {
    prepare_python();

    let core = ArboristCore::new();
    let workspace_root = std::env::current_dir().expect("current directory should be available");
    let file_path = workspace_root.join("missing.py");
    let workspace_root = workspace_root.to_string_lossy();
    let file_path = file_path.to_string_lossy();
    let errors = [
        core.read_symbol_json_impl(&workspace_root, "missing", None, None, None, Some(0))
            .expect_err("zero timeout should reach the core read query"),
        core.read_symbol_at_position_json_impl(
            &workspace_root,
            &file_path,
            0,
            0,
            None,
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach the core position read query"),
        core.read_symbol_context_json_impl(
            &workspace_root,
            "missing",
            "both",
            None,
            None,
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach the core context query"),
        core.read_symbol_context_at_position_json_impl(
            &workspace_root,
            &file_path,
            0,
            0,
            "both",
            None,
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach the core position context query"),
        core.read_symbol_discovery_context_json_impl(
            &workspace_root,
            "missing",
            "both",
            NeighborhoodBounds::new(2, 64),
            None,
            None,
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach the core discovery query"),
        core.read_symbol_discovery_context_at_position_json_impl(
            &workspace_root,
            &file_path,
            0,
            0,
            "both",
            NeighborhoodBounds::new(2, 64),
            None,
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach the core position discovery query"),
    ];

    for error in errors {
        assert!(
            error
                .to_string()
                .contains("invalid trace timeout_ms: value must be greater than zero")
        );
    }
}
#[test]
fn neighborhood_read_bindings_forward_zero_timeout_before_query_work() {
    prepare_python();

    let core = ArboristCore::new();
    let workspace_root = std::env::current_dir().expect("current directory should be available");
    let file_path = workspace_root.join("missing.py");
    let workspace_root = workspace_root.to_string_lossy();
    let error = core
        .read_symbol_neighborhood_context_json_impl(
            &workspace_root,
            "missing",
            "both",
            NeighborhoodBounds::new(2, 64),
            None,
            None,
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach the core read query");
    assert!(
        error
            .to_string()
            .contains("invalid trace timeout_ms: value must be greater than zero")
    );

    let file_path = file_path.to_string_lossy();
    let error = core
        .read_symbol_neighborhood_context_at_position_json_impl(
            &workspace_root,
            &file_path,
            0,
            0,
            "both",
            NeighborhoodBounds::new(2, 64),
            None,
            None,
            Some(0),
        )
        .expect_err("zero timeout should reach the core position query");
    assert!(
        error
            .to_string()
            .contains("invalid trace timeout_ms: value must be greater than zero")
    );
}
