use super::*;

#[test]
fn trace_symbol_graph_at_position_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_caller = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&caller, Some(renamed_caller)).unwrap();

    let result = vfs
        .trace_symbol_graph_at_position(
            &dir,
            &helper,
            &Position { row: 0, column: 5 },
            TraceDirection::Callers,
        )
        .unwrap();
    assert_eq!(result.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.callers.len(), 1);
    assert_eq!(result.callers[0].semantic_path, "orchestrate");
}

#[test]
fn trace_symbol_graph_at_position_with_source_normalizes_path_without_writing_disk() {
    let dir = temporary_dir();
    let nested = dir.join("child");
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let caller_alias = nested.join("..").join("caller.py");

    fs::create_dir_all(&nested).unwrap();
    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let result = trace_symbol_graph_at_position_with_source(
            &dir,
            &caller_alias,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
            &Position { row: 3, column: 5 },
            TraceDirection::Both,
        )
        .unwrap();

    assert!(!caller.exists());
    assert_eq!(result.symbol.semantic_path, "orchestrate");
    assert_eq!(result.symbol.file_path, normalize_path(&caller));
    assert!(
        result
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn traces_symbol_graph_at_position_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let position = Position { row: 0, column: 5 };
    let live =
        trace_symbol_graph_at_position(&dir, &helper, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "orchestrate");
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &helper,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "orchestrate");
}

#[test]
fn traces_javascript_symbol_graph_at_position_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./helper\";\nexport function caller(value: number): number { return helper(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 16 };
    let live =
        trace_symbol_graph_at_position(&dir, &helper, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &helper,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_rust_unshadowed_local_direct_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "mod api {\n    pub fn caller() { helper(); }\n    pub fn helper() {}\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, "api::helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "api::caller");

    let position = Position { row: 2, column: 11 };
    let live_at_position =
        trace_symbol_graph_at_position(&dir, &source_path, &position, TraceDirection::Callers)
            .unwrap();
    assert_eq!(live_at_position.symbol.symbol_id, "api::helper");
    assert_eq!(live_at_position.callers[0].symbol_id, "api::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, "api::helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "api::caller");

    let persisted_at_position = trace_symbol_graph_at_position_from_index(
        &db_path,
        &source_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_at_position.symbol.symbol_id, "api::helper");
    assert_eq!(persisted_at_position.callers[0].symbol_id, "api::caller");
}

#[test]
fn traces_rust_qualified_inline_module_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "fn caller() { api::helper(); }\n\nmod api {\n    pub fn helper() {}\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, "api::helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, "api::helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_java_unqualified_same_type_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;\nclass Counter {\n    int helper() { return 1; }\n    int caller() { return helper(); }\n    int first(int value) { return value; }\n    long first(long value) { return value; }\n    long ambiguous() { return first(1L); }\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Counter::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Counter::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_path);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Counter::caller"
    );

    let overloaded_id = format!(
        "{}::com::example::Counter::first#overload[2]",
        normalize_path(&source_path)
    );
    let overloaded =
        trace_symbol_graph_from_index(&db_path, &overloaded_id, TraceDirection::Callers).unwrap();
    assert!(overloaded.callers.is_empty());
}

#[test]
fn traces_csharp_conservative_direct_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "class GlobalHelper {\n    public static int Utility(int value) => value;\n    public static int Flexible(params int[] values) => values.Length;\n    public int Instance(int value) => value;\n}\nclass Counter {\n    Counter() {}\n    Counter(int value) : this() {}\n    Counter(string value) : base() {}\n    Counter(params int[] values) {}\n    Counter(bool first, bool second) : this(1, 2) {}\n    int Helper() => 1;\n    int Caller() => Helper();\n    int ExplicitThis() => this.Helper();\n    int ExplicitThisParameterShadow(System.Func<int> Helper) => this.Helper();\n    int First(int value) => value;\n    long First(long value) => value;\n    long Ambiguous() => First(1L);\n    int Flexible(params int[] values) => values.Length;\n    int ParamsCaller() => Flexible(1);\n    int GlobalStaticCaller() => global::GlobalHelper.Utility(1);\n    int GlobalInstanceCaller() => global::GlobalHelper.Instance(1);\n    int GlobalParamsCaller() => global::GlobalHelper.Flexible(1);\n}\nclass SimpleCaller {\n    int LocalStaticCaller() => GlobalHelper.Utility(1);\n    int LocalInstanceCaller() => GlobalHelper.Instance(1);\n    int LocalParamsCaller() => GlobalHelper.Flexible(1);\n}\nclass Outer {\n    class Nested {\n        int NestedStaticCaller() => GlobalHelper.Utility(1);\n    }\n}\nclass MemberShadowCaller {\n    GlobalHelper GlobalHelper { get; } = new GlobalHelper();\n    int MemberShadow() => GlobalHelper.Instance(1);\n}\n",
    )
    .unwrap();

    let helper_path = "Counter::Helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 3);
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Counter::Caller",
            "Counter::ExplicitThis",
            "Counter::ExplicitThisParameterShadow"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_path);
    assert_eq!(persisted.callers.len(), 3);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Counter::Caller",
            "Counter::ExplicitThis",
            "Counter::ExplicitThisParameterShadow"
        ]
    );

    let overloaded_id = format!(
        "{}::Counter::First#overload[2]",
        normalize_path(&source_path)
    );
    let overloaded =
        trace_symbol_graph_from_index(&db_path, &overloaded_id, TraceDirection::Callers).unwrap();
    assert!(overloaded.callers.is_empty());

    let params_target =
        trace_symbol_graph_from_index(&db_path, "Counter::Flexible", TraceDirection::Callers)
            .unwrap();
    assert!(params_target.callers.is_empty());

    let constructor_target = "Counter::Counter";
    let delegated_constructor_id = format!(
        "{}::Counter::Counter#overload[2]",
        normalize_path(&source_path)
    );
    let constructor_live =
        trace_symbol_graph(&dir, constructor_target, TraceDirection::Callers).unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        delegated_constructor_id
    );
    let constructor_persisted =
        trace_symbol_graph_from_index(&db_path, constructor_target, TraceDirection::Callers)
            .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        delegated_constructor_id
    );

    let params_constructor_id = format!(
        "{}::Counter::Counter#overload[4]",
        normalize_path(&source_path)
    );
    let params_constructor_live =
        trace_symbol_graph(&dir, &params_constructor_id, TraceDirection::Callers).unwrap();
    assert!(params_constructor_live.callers.is_empty());
    let params_constructor_persisted =
        trace_symbol_graph_from_index(&db_path, &params_constructor_id, TraceDirection::Callers)
            .unwrap();
    assert!(params_constructor_persisted.callers.is_empty());

    let static_target = "GlobalHelper::Utility";
    let static_live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(static_live.callers.len(), 2);
    assert_eq!(
        static_live
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Counter::GlobalStaticCaller",
            "SimpleCaller::LocalStaticCaller"
        ]
    );
    let static_persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(static_persisted.callers.len(), 2);
    assert_eq!(
        static_persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Counter::GlobalStaticCaller",
            "SimpleCaller::LocalStaticCaller"
        ]
    );

    for target in ["GlobalHelper::Instance", "GlobalHelper::Flexible"] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn traces_csharp_same_namespace_static_calls_across_files_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo;
class Helper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo;
class Caller {
    int Call() => Helper.Utility(1);
    int InstanceCall() => Helper.Instance(1);
    int ParamsCall() => Helper.Flexible(1);
    int Shadowed(int Helper) => Helper.Utility(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");

    for target in ["Demo::Helper::Instance", "Demo::Helper::Flexible"] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_ambiguous_csharp_same_namespace_static_calls_across_files() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo; class Caller { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Demo::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_global_static_calls_across_files_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("GlobalHelper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo;
class GlobalHelper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo;
class Caller {
    int Call() => global::Demo.GlobalHelper.Utility(1);
    int InstanceCall() => global::Demo.GlobalHelper.Instance(1);
    int ParamsCall() => global::Demo.GlobalHelper.Flexible(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::GlobalHelper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");

    for target in [
        "Demo::GlobalHelper::Instance",
        "Demo::GlobalHelper::Flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_ambiguous_csharp_global_static_calls_across_files() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo; class GlobalHelper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo; class GlobalHelper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo; class Caller { int Call() => global::Demo.GlobalHelper.Utility(1); }
",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Demo::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_file_type_alias_static_calls_across_files_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo.Utility;
class Helper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "using HelperAlias = Demo.Utility.Helper;
namespace Demo.App;
class Caller {
    int Call() => HelperAlias.Utility(1);
    int InstanceCall() => HelperAlias.Instance(1);
    int ParamsCall() => HelperAlias.Flexible(1);
    int Shadowed(int HelperAlias) => HelperAlias.Utility(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Utility::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::App::Caller::Call");

    for target in [
        "Demo::Utility::Helper::Instance",
        "Demo::Utility::Helper::Flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_ambiguous_or_colliding_csharp_file_type_alias_static_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("AmbiguousAlias.cs"),
        "using HelperAlias = Demo.Utility.Helper;
using HelperAlias = Demo.Utility.Other;
namespace Demo.App; class AmbiguousAlias { int Call() => HelperAlias.Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("CollidingAlias.cs"),
        "using HelperAlias = Demo.Utility.Helper;
namespace Demo.App;
class HelperAlias { public static int Utility(int value) => value; }
class CollidingAlias { int Call() => HelperAlias.Utility(1); }
",
    )
    .unwrap();

    for caller in [
        "Demo::App::AmbiguousAlias::Call",
        "Demo::App::CollidingAlias::Call",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}: {:?}", live.callees);
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "Demo::App::AmbiguousAlias::Call",
        "Demo::App::CollidingAlias::Call",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller}: {:?}",
            persisted.callees
        );
    }
}

#[test]
fn traces_csharp_namespace_scoped_type_alias_static_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let block_caller_path = dir.join("BlockCaller.cs");
    let file_caller_path = dir.join("FileCaller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo.Utility { class Helper { public static int Utility(int value) => value; } }
namespace Demo.Other { class Helper { public static int Utility(int value) => value; } }
",
    )
    .unwrap();
    fs::write(
        &block_caller_path,
        "using HelperAlias = Demo.Utility.Helper;
namespace Demo.App {
    using HelperAlias = Demo.Other.Helper;
    class BlockCaller { int Call() => HelperAlias.Utility(1); }
}
",
    )
    .unwrap();
    fs::write(
        &file_caller_path,
        "namespace Demo.File;
using HelperAlias = Demo.Utility.Helper;
class FileCaller { int Call() => HelperAlias.Utility(1); }
",
    )
    .unwrap();

    let other_target = "Demo::Other::Helper::Utility";
    let live = trace_symbol_graph(&dir, other_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::BlockCaller::Call");

    let utility_target = "Demo::Utility::Helper::Utility";
    let live = trace_symbol_graph(&dir, utility_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::File::FileCaller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, other_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::App::BlockCaller::Call"
    );

    let persisted =
        trace_symbol_graph_from_index(&db_path, utility_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::File::FileCaller::Call"
    );
}

#[test]
fn does_not_trace_ambiguous_csharp_namespace_scoped_type_alias_static_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Helper.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App {
    using HelperAlias = Demo.Utility.Helper;
    using HelperAlias = Demo.Other.Helper;
    class Caller { int Call() => HelperAlias.Utility(1); }
}
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "Demo::App::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_namespace_scoped_static_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let block_caller_path = dir.join("BlockCaller.cs");
    let file_caller_path = dir.join("FileCaller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo.Utility {
    class BlockHelpers { public static int Utility(int value) => value; }
    class FileHelpers { public static int Utility(int value) => value; }
}
",
    )
    .unwrap();
    fs::write(
        &block_caller_path,
        "namespace Demo.App {
    using static Demo.Utility.BlockHelpers;
    class BlockCaller { int Call() => Utility(1); }
}
namespace Demo.Other {
    class OutOfScopeCaller { int Call() => Utility(1); }
}
",
    )
    .unwrap();
    fs::write(
        &file_caller_path,
        "namespace Demo.File;
using static Demo.Utility.FileHelpers;
class FileCaller { int Call() => Utility(1); }
",
    )
    .unwrap();

    let block_target = "Demo::Utility::BlockHelpers::Utility";
    let live = trace_symbol_graph(&dir, block_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::BlockCaller::Call");

    let file_target = "Demo::Utility::FileHelpers::Utility";
    let live = trace_symbol_graph(&dir, file_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::File::FileCaller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, block_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::App::BlockCaller::Call"
    );

    let persisted =
        trace_symbol_graph_from_index(&db_path, file_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::File::FileCaller::Call"
    );
}

#[test]
fn does_not_trace_ambiguous_csharp_namespace_scoped_static_import_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Helpers.cs"),
        "namespace Demo.Utility {
    class First { public static int Utility(int value) => value; }
    class Second { public static int Utility(int value) => value; }
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App {
    using static Demo.Utility.First;
    using static Demo.Utility.Second;
    class Caller { int Call() => Utility(1); }
}
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "Demo::App::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_file_static_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo.Utility;
class Helper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
class Unrelated { public static int Other(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "using static Demo.Utility.Helper;
using static Demo.Utility.Unrelated;
namespace Demo.App;
class Caller {
    int Call() => Utility(1);
    int InstanceCall() => Instance(1);
    int ParamsCall() => Flexible(1);
}
class LocalNameBlocksImport {
    int Utility() => 1;
    int Call() => Utility(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Utility::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::App::Caller::Call");

    for target in [
        "Demo::Utility::Helper::Instance",
        "Demo::Utility::Helper::Flexible",
        "Demo::App::LocalNameBlocksImport::Utility",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_ambiguous_csharp_file_static_import_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
class Other { public static int Utility(int value) => value; }
class UniqueHelper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("AmbiguousType.cs"),
        "using static Demo.Utility.Helper;
namespace Demo.App; class AmbiguousType { int Call() => Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("MultipleImports.cs"),
        "using static Demo.Utility.Other;
using static Demo.Utility.UniqueHelper;
namespace Demo.App; class MultipleImports { int Call() => Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("DuplicateImport.cs"),
        "using static Demo.Utility.UniqueHelper;
using static Demo.Utility.UniqueHelper;
namespace Demo.App; class DuplicateImport { int Call() => Utility(1); }
",
    )
    .unwrap();

    for caller in [
        "Demo::App::AmbiguousType::Call",
        "Demo::App::MultipleImports::Call",
        "Demo::App::DuplicateImport::Call",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty());
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "Demo::App::AmbiguousType::Call",
        "Demo::App::MultipleImports::Call",
        "Demo::App::DuplicateImport::Call",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty());
    }
}

#[test]
fn traces_csharp_file_namespace_import_static_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo.Utility;
class Helper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "using Demo.Utility;
namespace Demo.App;
class Caller {
    int Call() => Helper.Utility(1);
    int InstanceCall() => Helper.Instance(1);
    int ParamsCall() => Helper.Flexible(1);
    int Shadowed(int Helper) => Helper.Utility(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Utility::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::App::Caller::Call");

    for target in [
        "Demo::Utility::Helper::Instance",
        "Demo::Utility::Helper::Flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn handles_ambiguous_and_same_namespace_csharp_file_namespace_import_static_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Utility.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Shared.cs"),
        "namespace Demo.Shared; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Local.cs"),
        "namespace Demo.App; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("FirstDuplicate.cs"),
        "namespace Demo.Duplicate; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("SecondDuplicate.cs"),
        "namespace Demo.Duplicate; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("MultipleImports.cs"),
        "using Demo.Utility;
using Demo.Shared;
namespace Demo.Client; class MultipleImports { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("DuplicateImport.cs"),
        "using Demo.Utility;
using Demo.Utility;
namespace Demo.Client; class DuplicateImport { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("SameNamespace.cs"),
        "using Demo.Utility;
namespace Demo.App; class SameNamespace { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("DuplicateType.cs"),
        "using Demo.Duplicate;
namespace Demo.Client; class DuplicateType { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();

    for caller in [
        "Demo::Client::MultipleImports::Call",
        "Demo::Client::DuplicateImport::Call",
        "Demo::Client::DuplicateType::Call",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty());
    }

    let same_namespace_target = "Demo::App::Helper::Utility";
    let live = trace_symbol_graph(&dir, same_namespace_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::SameNamespace::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "Demo::Client::MultipleImports::Call",
        "Demo::Client::DuplicateImport::Call",
        "Demo::Client::DuplicateType::Call",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty());
    }

    let persisted =
        trace_symbol_graph_from_index(&db_path, same_namespace_target, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::App::SameNamespace::Call"
    );
}

#[test]
fn traces_java_explicit_this_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;\nclass Counter {\n    int helper() { return 1; }\n    int caller() { return this.helper(); }\n}\n",
    )
    .unwrap();

    let helper_symbol = "com::example::Counter::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Counter::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Counter::caller"
    );
}

#[test]
fn traces_java_explicit_local_static_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let helper_path = source_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;\nimport static com.example.Helper.utility;\nimport static com.example.Helper.instance;\nclass Main {\n    int caller() { return utility(1); }\n    int nonStatic() { return instance(1); }\n}\nclass Competing {\n    int utility(long value) { return (int) value; }\n    int caller() { return utility(1); }\n}\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package com.example;\nclass Helper {\n    static int utility(int value) { return value; }\n    int instance(int value) { return value; }\n}\n",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");

    let instance_symbol = "com::example::Helper::instance";
    let live_instance = trace_symbol_graph(&dir, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(live_instance.callers.is_empty());
    let persisted_instance =
        trace_symbol_graph_from_index(&db_path, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted_instance.callers.is_empty());
}

#[test]
fn ignores_ambiguous_java_explicit_static_import_calls_in_live_and_persisted_indexes() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let first_path = source_dir.join("First.java");
    let second_path = source_dir.join("Second.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;\nimport static com.example.First.utility;\nimport static com.example.Second.utility;\nclass Main { int caller() { return utility(1); } }\n",
    )
    .unwrap();
    fs::write(
        &first_path,
        "package com.example; class First { static int utility(int value) { return value; } }\n",
    )
    .unwrap();
    fs::write(
        &second_path,
        "package com.example; class Second { static int utility(int value) { return value; } }\n",
    )
    .unwrap();

    let first_symbol = "com::example::First::utility";
    let live = trace_symbol_graph(&dir, first_symbol, TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, first_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_explicit_local_import_static_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let helper_path = source_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;\nimport com.example.Helper;\nclass Main {\n    int caller() { return Helper.utility(1); }\n    int shadowed(Helper Helper) { return Helper.utility(1); }\n    int nonStatic() { return Helper.instance(1); }\n}\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package com.example;\nclass Helper {\n    static int utility(int value) { return value; }\n    int instance(int value) { return value; }\n}\n",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");

    let instance_symbol = "com::example::Helper::instance";
    let live_instance = trace_symbol_graph(&dir, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(live_instance.callers.is_empty());
    let persisted_instance =
        trace_symbol_graph_from_index(&db_path, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted_instance.callers.is_empty());
}

#[test]
fn traces_go_unshadowed_same_file_direct_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\nfunc caller() int { return helper() }\nfunc helper() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    let position = Position { row: 3, column: 5 };
    let live_at_position =
        trace_symbol_graph_at_position(&dir, &source_path, &position, TraceDirection::Callers)
            .unwrap();
    assert_eq!(live_at_position.symbol.symbol_id, "helper");
    assert_eq!(live_at_position.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");

    let persisted_at_position = trace_symbol_graph_at_position_from_index(
        &db_path,
        &source_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_at_position.symbol.symbol_id, "helper");
    assert_eq!(persisted_at_position.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_local_package_imported_function_calls_in_live_and_persisted_indexes() {
    let dir = temporary_dir();
    let caller_path = dir.join("cmd").join("main.go");
    let service_path = dir.join("internal").join("service").join("service.go");
    let utility_path = dir.join("internal").join("utility").join("utility.go");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(service_path.parent().unwrap()).unwrap();
    fs::create_dir_all(utility_path.parent().unwrap()).unwrap();
    fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &caller_path,
        "package main\n\nimport (\n    \"example.com/project/internal/service\"\n    utility_alias \"example.com/project/internal/utility\"\n)\n\ntype local struct{}\nfunc (local) Value() int { return 0 }\nfunc caller() int { return service.Value() + utility_alias.Other() }\nfunc shadowed() int { service := local{}; return service.Value() }\n",
    )
    .unwrap();
    fs::write(
        &service_path,
        "package service\n\nfunc Value() int { return 1 }\n",
    )
    .unwrap();
    fs::write(
        &utility_path,
        "package utility\n\nfunc Other() int { return 2 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.symbol.symbol_id, "Value");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.symbol.symbol_id, "Value");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");

    let position = Position { row: 2, column: 5 };
    let persisted_at_position = trace_symbol_graph_at_position_from_index(
        &db_path,
        &service_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_at_position.symbol.symbol_id, "Value");
    assert_eq!(persisted_at_position.callers.len(), 1);
    assert_eq!(persisted_at_position.callers[0].symbol_id, "caller");
}

#[test]
fn does_not_trace_go_direct_calls_across_source_files() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let helper_path = dir.join("helper.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc caller() int { return helper() }\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package metrics\n\nfunc helper() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert!(persisted.callers.is_empty());
}
