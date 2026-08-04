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
fn traces_csharp_unqualified_same_type_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "class Counter {\n    int Helper() => 1;\n    int Caller() => Helper();\n    int ExplicitThis() => this.Helper();\n    int First(int value) => value;\n    long First(long value) => value;\n    long Ambiguous() => First(1L);\n    int Flexible(params int[] values) => values.Length;\n    int ParamsCaller() => Flexible(1);\n}\n",
    )
    .unwrap();

    let helper_path = "Counter::Helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Counter::Caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_path);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Counter::Caller");

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
