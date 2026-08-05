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
fn traces_csharp_outer_namespace_static_import_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("Helper.cs");
    let caller = dir.join("Caller.cs");
    fs::write(
        &helper,
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo.App; class Caller { int Call(int value) => value; }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some(
            "namespace Demo {\n    using static Demo.Utility.Helper;\n    namespace App { class Caller { int Call(int value) => Utility(value); } }\n}\n",
        ),
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

#[test]
fn traces_csharp_enclosing_namespace_static_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("Helper.cs");
    let caller = dir.join("Caller.cs");
    fs::write(
        &helper,
        "namespace Demo; class Helper { public static int Utility(int value) => value; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo.App.Tools; class Caller { int Call(int value) => value; }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some(
            "namespace Demo.App.Tools; class Caller { int Call(int value) => Helper.Utility(value); }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "Demo::Helper::Utility", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::App::Tools::Caller::Call");
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

#[test]
fn traces_csharp_global_namespace_import_calls_from_dirty_vfs_overrides() {
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
        "namespace Demo.App; class Caller { int Call() => Helper.Utility(1); }\n",
    )
    .unwrap();
    fs::write(&global_usings, "// no global imports on disk\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&global_usings, Some("global using Demo.Utility;\n"))
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

#[test]
fn traces_csharp_global_type_alias_calls_from_dirty_vfs_overrides() {
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
        "namespace Demo.App; class Caller { int Call() => HelperAlias.Utility(1); }\n",
    )
    .unwrap();
    fs::write(&global_usings, "// no global imports on disk\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &global_usings,
        Some("global using HelperAlias = Demo.Utility.Helper;\n"),
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

#[test]
fn traces_csharp_global_base_namespace_import_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base = dir.join("Base.cs");
    let derived = dir.join("Derived.cs");
    let global_usings = dir.join("GlobalUsings.cs");
    fs::write(
        &base,
        "namespace Demo.Utility; class Base { public int Ping(int value) => value; }\n",
    )
    .unwrap();
    fs::write(
        &derived,
        "namespace Demo.App; class Derived { int Call(int value) => value; }\n",
    )
    .unwrap();
    fs::write(&global_usings, "// no global imports on disk\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&global_usings, Some("global using Demo.Utility;\n"))
        .unwrap();
    vfs.open_file(
        &derived,
        Some("namespace Demo.App; class Derived : Base { int Call(int value) => base.Ping(value); }\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "Demo::Utility::Base::Ping", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::App::Derived::Call");
}

#[test]
fn traces_csharp_base_namespace_import_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base = dir.join("Base.cs");
    let derived = dir.join("Derived.cs");
    fs::write(
        &base,
        "namespace Demo.Utility; class Base { public int Ping(int value) => value; }\n",
    )
    .unwrap();
    fs::write(
        &derived,
        "namespace Demo.App; class Derived { int Call(int value) => value; }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &derived,
        Some(
            "using Demo.Utility;\nnamespace Demo.App; class Derived : Base { int Call(int value) => base.Ping(value); }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "Demo::Utility::Base::Ping", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::App::Derived::Call");
}

#[test]
fn traces_csharp_base_alias_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base = dir.join("Base.cs");
    let derived = dir.join("Derived.cs");
    fs::write(
        &base,
        "namespace Demo; class Base { public int Ping(int value) => value; }\n",
    )
    .unwrap();
    fs::write(
        &derived,
        "namespace Demo.App; class Derived { int Call(int value) => value; }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &derived,
        Some(
            "using BaseAlias = Demo.Base;\nnamespace Demo.App; class Derived : BaseAlias { int Call(int value) => base.Ping(value); }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "Demo::Base::Ping", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::App::Derived::Call");
}

#[test]
fn traces_csharp_generic_base_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base = dir.join("Base.cs");
    let derived = dir.join("Derived.cs");
    fs::write(
        &base,
        "namespace Demo; class Base<T> { public int Ping(int value) => value; }\n",
    )
    .unwrap();
    fs::write(
        &derived,
        "namespace Demo; class Derived { int Call(int value) => value; }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &derived,
        Some(
            "namespace Demo; class Derived : Base<int> { int Call(int value) => base.Ping(value); }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "Demo::Base::Ping", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::Derived::Call");
}

#[test]
fn traces_csharp_ancestor_base_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let grand = dir.join("Grand.cs");
    let parent = dir.join("Parent.cs");
    let derived = dir.join("Derived.cs");
    fs::write(
        &grand,
        "namespace Demo.Utility; class Grand { public int Ping(int value) => value; }\n",
    )
    .unwrap();
    fs::write(&parent, "namespace Demo.Middle; class Parent {}\n").unwrap();
    fs::write(
        &derived,
        "using Demo.Middle; namespace Demo.App; class Derived : Parent { int Call(int value) => base.Ping(value); }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &parent,
        Some(
            "using GrandAlias = Demo.Utility.Grand;\nnamespace Demo.Middle; class Parent : GrandAlias {}\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "Demo::Utility::Grand::Ping", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::App::Derived::Call");
}

#[test]
fn traces_csharp_base_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base = dir.join("Base.cs");
    let derived = dir.join("Derived.cs");
    fs::write(
        &base,
        "namespace Demo; class Base { public int Ping(int value) => value; }\n",
    )
    .unwrap();
    fs::write(
        &derived,
        "namespace Demo; class Derived : Base { int Call(int value) => value; }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &derived,
        Some("namespace Demo; class Derived : Base { int Call(int value) => base.Ping(value); }\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "Demo::Base::Ping", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::Derived::Call");
}

#[test]
fn traces_csharp_qualified_base_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base = dir.join("Base.cs");
    let derived = dir.join("Derived.cs");
    fs::write(
        &base,
        "namespace Demo; class Base { public int Ping(int value) => value; }\n",
    )
    .unwrap();
    fs::write(
        &derived,
        "namespace Other; class Derived { int Call(int value) => value; }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &derived,
        Some("namespace Other; class Derived : Demo.Base { int Call(int value) => base.Ping(value); }\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "Demo::Base::Ping", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Other::Derived::Call");
}

#[test]
fn traces_csharp_base_constructor_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base = dir.join("Base.cs");
    let derived = dir.join("Derived.cs");
    fs::write(
        &base,
        "namespace Demo; class Base { public Base(int value) {} }\n",
    )
    .unwrap();
    fs::write(
        &derived,
        "namespace Demo; class Derived : Base { Derived(int value) {} }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &derived,
        Some("namespace Demo; class Derived : Base { Derived(int value) : base(value) {} }\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "Demo::Base::Base", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::Derived::Derived");
}
