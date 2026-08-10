use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use arborist_core::{TraceDirection, VirtualFileSystem};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temporary_dir() -> PathBuf {
    let suffix = format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let dir = std::env::temp_dir().join(format!("arborist-mcp-{suffix}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

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
fn traces_csharp_nested_type_static_calls_through_namespace_imports_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let outer = dir.join("Outer.cs");
    let caller = dir.join("Caller.cs");
    fs::write(
        &outer,
        "namespace Demo.Utility; class Outer { class Helper { public static int Utility(int value) => value; } }\n",
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
            "using Demo.Utility;\nnamespace Demo.App; class Caller { int Call(int value) => Outer.Helper.Utility(value); }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(
            &dir,
            "Demo::Utility::Outer::Helper::Utility",
            TraceDirection::Callers,
        )
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::App::Caller::Call");
}

#[test]
fn traces_csharp_nested_type_static_calls_through_aliases_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let outer = dir.join("Outer.cs");
    let caller = dir.join("Caller.cs");
    fs::write(
        &outer,
        "namespace Demo; class Outer { class Helper { public static int Utility(int value) => value; } }\n",
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
            "using AliasOuter = Demo.Outer;\nnamespace Demo.App; class Caller { int Call(int value) => AliasOuter.Helper.Utility(value); }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(
            &dir,
            "Demo::Outer::Helper::Utility",
            TraceDirection::Callers,
        )
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::App::Caller::Call");
}

#[test]
fn traces_csharp_generic_nested_static_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let outer = dir.join("Outer.cs");
    let caller = dir.join("Caller.cs");
    fs::write(
        &outer,
        "namespace Demo; class Outer<T> { class Helper<U> { public static int Utility(int value) => value; } }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo; class Caller { int Call(int value) => value; }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some(
            "namespace Demo; class Caller { int Call(int value) => Outer<int>.Helper<string>.Utility(value); }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(
            &dir,
            "Demo::Outer::Helper::Utility",
            TraceDirection::Callers,
        )
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn traces_csharp_generic_import_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let targets = dir.join("Targets.cs");
    let caller = dir.join("Caller.cs");
    let global_usings = dir.join("GlobalUsings.cs");
    fs::write(
        &targets,
        "namespace Demo.Utility;
class LocalAliasTarget<T> { public static int FromLocalAlias(int value) => value; }
class LocalStaticTarget<T> { public static int FromLocalStatic(int value) => value; }
class GlobalAliasTarget<T> { public static int FromGlobalAlias(int value) => value; }
class GlobalStaticTarget<T> { public static int FromGlobalStatic(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo.App; class Caller { int Call() => 0; }
",
    )
    .unwrap();
    fs::write(
        &global_usings,
        "// no global imports
",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some(
            "using LocalAlias = Demo.Utility.LocalAliasTarget<int>;
using static Demo.Utility.LocalStaticTarget<int>;
namespace Demo.App;
class Caller {
    int LocalAliasCall() => LocalAlias.FromLocalAlias(1);
    int LocalStaticCall() => FromLocalStatic(1);
    int GlobalAliasCall() => GlobalAlias.FromGlobalAlias(1);
    int GlobalStaticCall() => FromGlobalStatic(1);
}
",
        ),
    )
    .unwrap();
    vfs.open_file(
        &global_usings,
        Some(
            "global using GlobalAlias = Demo.Utility.GlobalAliasTarget<int>;
global using static Demo.Utility.GlobalStaticTarget<int>;
",
        ),
    )
    .unwrap();

    for (target, expected_caller) in [
        (
            "Demo::Utility::LocalAliasTarget::FromLocalAlias",
            "Demo::App::Caller::LocalAliasCall",
        ),
        (
            "Demo::Utility::LocalStaticTarget::FromLocalStatic",
            "Demo::App::Caller::LocalStaticCall",
        ),
        (
            "Demo::Utility::GlobalAliasTarget::FromGlobalAlias",
            "Demo::App::Caller::GlobalAliasCall",
        ),
        (
            "Demo::Utility::GlobalStaticTarget::FromGlobalStatic",
            "Demo::App::Caller::GlobalStaticCall",
        ),
    ] {
        let trace = vfs
            .trace_symbol_graph(&dir, target, TraceDirection::Callers)
            .unwrap();
        assert_eq!(trace.callers.len(), 1, "{target}");
        assert_eq!(trace.callers[0].symbol_id, expected_caller, "{target}");
    }
}

#[test]
fn traces_csharp_nested_type_static_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let outer = dir.join("Outer.cs");
    let caller = dir.join("Caller.cs");
    fs::write(
        &outer,
        "namespace Demo; class Outer { class Helper { public static int Utility(int value) => value; } }\n",
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
            "namespace Demo.App; class Caller { int Call(int value) => Outer.Helper.Utility(value); }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(
            &dir,
            "Demo::Outer::Helper::Utility",
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
fn traces_csharp_inherited_bare_calls_from_dirty_vfs_overrides() {
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
        Some("namespace Demo; class Derived : Base { int Call(int value) => Ping(value); }\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "Demo::Base::Ping", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "Demo::Derived::Call");
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

#[test]
fn traces_kotlin_same_package_top_level_function_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("Helper.kt");
    let caller = dir.join("Caller.kt");
    fs::write(
        &helper,
        "package com.example\n\nfun helper(value: Int): Int = value\n",
    )
    .unwrap();
    fs::write(&caller, "package com.example\n\nfun caller(): Int = 0\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some("package com.example\n\nfun caller(): Int = helper(1)\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "com::example::helper", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "com::example::caller");
}
#[test]
fn traces_kotlin_imported_top_level_function_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("Helper.kt");
    let caller = dir.join("Caller.kt");
    fs::write(
        &helper,
        "package org.util\n\nfun helper(value: Int): Int = value\n",
    )
    .unwrap();
    fs::write(&caller, "package com.example\n\nfun caller(): Int = 0\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some("package com.example\n\nimport org.util.helper\n\nfun caller(): Int = helper(1)\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "org::util::helper", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "com::example::caller");
}
#[test]
fn traces_kotlin_qualified_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("Helper.kt");
    let caller = dir.join("Caller.kt");
    fs::write(
        &helper,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "package com.example\n\nclass Caller {\n    fun run(): Int = 0\n}\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some("package com.example\n\nclass Caller {\n    fun run(): Int {\n        val other = Other()\n        return other.helper(1)\n    }\n}\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "com::example::Other::helper", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "com::example::Caller::run");
}
#[test]
fn traces_kotlin_extension_function_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let extension = dir.join("Extensions.kt");
    let caller = dir.join("Caller.kt");
    fs::write(
        &extension,
        "package com.example\n\nfun Other.helper(value: Int): Int = value\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "package com.example\n\nclass Other\n\nclass Caller {\n    fun run(): Int = 0\n}\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some("package com.example\n\nclass Other\n\nclass Caller {\n    fun run(): Int {\n        val other = Other()\n        return other.helper(1)\n    }\n}\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "com::example::helper", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_kotlin_imported_extension_function_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let extension = dir.join("Extensions.kt");
    let caller = dir.join("Caller.kt");
    fs::write(
        &extension,
        "package org.util\n\nclass Other\n\nfun Other.helper(value: Int): Int = value\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "package com.example\n\nclass Caller {\n    fun run(): Int = 0\n}\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some("package com.example\n\nimport org.util.Other\nimport org.util.helper\n\nclass Caller {\n    fun run(other: Other): Int = other.helper(1)\n}\n"),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&dir, "org::util::helper", TraceDirection::Callers)
        .unwrap();
    assert_eq!(trace.callers.len(), 1);
    assert_eq!(trace.callers[0].symbol_id, "com::example::Caller::run");
}
