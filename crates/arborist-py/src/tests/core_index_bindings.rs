use super::*;

#[test]
fn semantic_skeleton_binding_forwards_zero_timeout_before_path_work() {
    prepare_python();

    let core = ArboristCore::new();
    let errors = [
        core.get_semantic_skeleton_json_impl("", None, 2, None, Some(0))
            .expect_err("path skeleton should reject zero timeout"),
        core.get_semantic_skeleton_json_impl("", Some("value = 1\n".to_string()), 2, None, Some(0))
            .expect_err("inline skeleton should reject zero timeout"),
    ];

    for error in errors {
        assert!(
            error
                .to_string()
                .contains("invalid semantic skeleton timeout_ms: value must be greater than zero")
        );
    }
}
#[test]
fn index_migration_binding_forwards_zero_timeout_before_path_work() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
        .migrate_symbol_index_json_impl("", Some(0))
        .expect_err("zero timeout should reach index migration before path validation");

    assert!(
        error
            .to_string()
            .contains("invalid workspace scan timeout_ms: value must be greater than zero")
    );
}
#[test]
fn index_registry_bindings_forward_timeout_bounds() {
    prepare_python();

    let core = ArboristCore::new();
    let unregister_error = core
        .unregister_symbol_index_json_impl("", Some(0))
        .expect_err("zero timeout should reach registry validation before path work");
    let list_error = core
        .list_symbol_indexes_json_impl(Some(MAX_SYMBOL_INDEX_REGISTRY_TIMEOUT_MS + 1))
        .expect_err("excessive timeout should reach registry validation");

    assert!(
        unregister_error
            .to_string()
            .contains("invalid symbol index registry timeout_ms")
    );
    assert!(list_error.to_string().contains("must not exceed"));
}
