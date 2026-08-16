use super::*;

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
fn traces_javascript_default_import_call_edge_at_position_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "export default function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import selected from \"./helper\";\nexport function caller(value: number): number { return selected(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 24 };
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
fn traces_javascript_default_import_call_edge_in_mjs_modules() {
    let dir = temporary_dir();
    let helper = dir.join("helper.mjs");
    let caller = dir.join("caller.mjs");

    fs::write(
        &helper,
        "export default function helper(value) { return value + 1; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import selected from \"./helper.mjs\";\nexport function caller(value) { return selected(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.symbol.semantic_path, "caller");
    let helper_symbol = live
        .callees
        .iter()
        .find(|symbol| symbol.semantic_path == "helper")
        .unwrap_or_else(|| {
            panic!(
                "expected caller to resolve to the mjs default export, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn traces_javascript_default_import_call_edge_through_reexport_bridge() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export default function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&bridge, "export { default } from \"./helper\";\n").unwrap();
    fs::write(
        &caller,
        "import forwarded from \"./bridge\";\nexport function caller(value: number): number { return forwarded(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.symbol.semantic_path, "caller");
    let helper_symbol = live
        .callees
        .iter()
        .find(|symbol| symbol.semantic_path == "helper")
        .unwrap_or_else(|| {
            panic!(
                "expected caller to resolve to helper through the bridge, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn keeps_anonymous_default_export_import_edges_fail_closed() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export default function (value: number): number { return value + 1; }\nexport function selected(): number { return 7; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import selected from \"./helper\";\nexport function caller(value: number): number { return selected(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "anonymous default exports must not resolve import calls, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_namespace_import_call_edges_capability_gated() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./helper\";\nexport function caller(value: number): number { return ns(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "namespace imports remain capability-gated, callees: {:?}",
        live.callees
    );
}
