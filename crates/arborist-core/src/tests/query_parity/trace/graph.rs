use super::*;

#[test]
fn traces_unqualified_cpp_using_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let definitions = dir.join("definitions.cpp");
    let caller = dir.join("caller.cpp");
    fs::write(
        &definitions,
        "namespace api { namespace base { int convert(int value) { return value + 1; } } }\n",
    )
    .unwrap();
    fs::write(&caller, "namespace api { int caller() { return 0; } }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some("namespace api { using base::convert; int caller() { return convert(1); } }\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "api::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::base::convert(int)"]
    );
}

#[test]
fn traces_cpp_module_interface_symbols_in_live_and_persisted_indexes() {
    let dir = temporary_dir();
    let module = dir.join("api.cppm");
    fs::write(
        &module,
        "export module api;\nexport int helper(int value) { return value + 1; }\nexport int caller() { return helper(1); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(
        live.callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["helper(int)"]
    );

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &module,
        Some(
            "export module api;\nexport int helper(int value) { return value + 2; }\nexport int caller() { return helper(1); }\n",
        ),
    )
    .unwrap();
    let virtual_trace = vfs
        .trace_symbol_graph(&dir, "caller", TraceDirection::Callees)
        .unwrap();
    assert_eq!(
        virtual_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["helper(int)"]
    );

    let db_path = dir.join("symbols.sqlite");
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(
        persisted
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["helper(int)"]
    );
}
