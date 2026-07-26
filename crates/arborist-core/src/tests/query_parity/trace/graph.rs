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
