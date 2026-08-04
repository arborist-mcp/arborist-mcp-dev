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
fn traces_csharp_global_static_import_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("Helper.cs");
    let caller = dir.join("Caller.cs");
    let global_usings = dir.join("GlobalUsings.cs");
    fs::write(
        &helper,
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo.App; class Caller { int Call() => Utility(1); }\n",
    )
    .unwrap();
    fs::write(&global_usings, "// no global imports on disk\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &global_usings,
        Some("global using static Demo.Utility.Helper;\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(
            &dir,
            "Demo::Utility::Helper::Utility",
            TraceDirection::Callers,
        )
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::App::Caller::Call");
}
