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
fn keeps_bare_namespace_import_calls_fail_closed() {
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
        "bare namespace usage must stay fail-closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn traces_javascript_namespace_import_member_call_edge_at_position_in_live_workspace_and_persisted_index()
 {
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
        "import * as ns from \"./helper\";\nexport function caller(value: number): number { return ns.helper(value); }\n",
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
fn traces_javascript_namespace_import_member_call_edge_in_mjs_modules() {
    let dir = temporary_dir();
    let helper = dir.join("helper.mjs");
    let caller = dir.join("caller.mjs");

    fs::write(
        &helper,
        "export function helper(value) { return value + 1; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./helper.mjs\";\nexport function caller(value) { return ns.helper(value); }\n",
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
                "expected caller to resolve to the mjs namespace member, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn keeps_namespace_import_unknown_member_calls_fail_closed_without_falling_back() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let caller = dir.join("caller.ts");

    fs::write(&helper, "export function other(): number { return 1; }\n").unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./helper\";\nfunction helper(): number { return 2; }\nexport function caller(): number { return ns.helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "unknown namespace members must fail closed without same-named fallback, callees: {:?}",
        live.callees
    );
}

#[test]
fn traces_javascript_named_import_call_edge_through_star_reexport_bridge() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&bridge, "export * from \"./helper\";\n").unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./bridge\";\nexport function caller(value: number): number { return helper(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 16 };
    let live =
        trace_symbol_graph_at_position(&dir, &helper, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
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
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_javascript_named_import_call_edge_through_nested_star_reexports() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let mid = dir.join("mid.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&mid, "export * from \"./helper\";\n").unwrap();
    fs::write(&bridge, "export * from \"./mid\";\n").unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./bridge\";\nexport function caller(value: number): number { return helper(value); }\n",
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
                "expected caller to resolve through nested star re-exports, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn traces_javascript_named_import_call_edge_through_star_then_named_reexport() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let mid = dir.join("mid.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&mid, "export { helper } from \"./helper\";\n").unwrap();
    fs::write(&bridge, "export * from \"./mid\";\n").unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./bridge\";\nexport function caller(value: number): number { return helper(value); }\n",
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
                "expected caller to resolve through star then named re-export, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn traces_javascript_named_import_call_edge_through_star_reexport_in_mjs_modules() {
    let dir = temporary_dir();
    let helper = dir.join("helper.mjs");
    let bridge = dir.join("bridge.mjs");
    let caller = dir.join("caller.mjs");

    fs::write(
        &helper,
        "export function helper(value) { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&bridge, "export * from \"./helper.mjs\";\n").unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./bridge.mjs\";\nexport function caller(value) { return helper(value); }\n",
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
                "expected caller to resolve to the mjs helper through the star bridge, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn resolves_star_reexport_named_import_to_direct_export_when_both_present() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "export * from \"./helper\";\nexport function helper(value: number): number { return value + 2; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./bridge\";\nexport function caller(value: number): number { return helper(value); }\n",
    )
    .unwrap();

    // The bridge's direct export shadows the star re-export, so the call
    // resolves to the bridge's own helper, not the helper module's.
    let bridge_position = Position { row: 1, column: 16 };
    let bridge_callers =
        trace_symbol_graph_at_position(&dir, &bridge, &bridge_position, TraceDirection::Callers)
            .unwrap();
    assert_eq!(bridge_callers.callers.len(), 1);
    assert_eq!(bridge_callers.callers[0].semantic_path, "caller");

    let helper_position = Position { row: 0, column: 16 };
    let helper_callers =
        trace_symbol_graph_at_position(&dir, &helper, &helper_position, TraceDirection::Callers)
            .unwrap();
    assert!(
        helper_callers.callers.is_empty(),
        "the shadowed star re-export must not receive the call, callers: {:?}",
        helper_callers.callers
    );
}

#[test]
fn keeps_star_reexport_ambiguous_named_imports_fail_closed() {
    let dir = temporary_dir();
    let first = dir.join("first.ts");
    let second = dir.join("second.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(&first, "export function helper(): number { return 1; }\n").unwrap();
    fs::write(&second, "export function helper(): number { return 2; }\n").unwrap();
    fs::write(
        &bridge,
        "export * from \"./first\";\nexport * from \"./second\";\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./bridge\";\nexport function caller(): number { return helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "ambiguous star re-exports must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_star_reexport_cyclic_chains_fail_closed() {
    let dir = temporary_dir();
    let first = dir.join("first.ts");
    let second = dir.join("second.ts");
    let caller = dir.join("caller.ts");

    fs::write(&first, "export * from \"./second\";\n").unwrap();
    fs::write(&second, "export * from \"./first\";\n").unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./first\";\nexport function caller(): number { return helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "cyclic star re-exports must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_star_reexport_non_exported_member_fail_closed() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(&helper, "function helper(): number { return 1; }\n").unwrap();
    fs::write(&bridge, "export * from \"./helper\";\n").unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./bridge\";\nexport function caller(): number { return helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "non-exported star members must stay fail-closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_star_reexport_default_exports_fail_closed() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export default function helper(): number { return 1; }\n",
    )
    .unwrap();
    fs::write(&bridge, "export * from \"./helper\";\n").unwrap();
    fs::write(
        &caller,
        "import selected from \"./bridge\";\nexport function caller(): number { return selected(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "star re-exports must not forward default exports, callees: {:?}",
        live.callees
    );
}

#[test]
fn traces_javascript_namespace_reexport_member_call_edge_at_position_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&bridge, "export * as ns from \"./helper\";\n").unwrap();
    fs::write(
        &caller,
        "import { ns } from \"./bridge\";\nexport function caller(value: number): number { return ns.helper(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 16 };
    let live =
        trace_symbol_graph_at_position(&dir, &helper, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
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
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_javascript_namespace_reexport_member_call_edge_in_mjs_modules() {
    let dir = temporary_dir();
    let helper = dir.join("helper.mjs");
    let bridge = dir.join("bridge.mjs");
    let caller = dir.join("caller.mjs");

    fs::write(
        &helper,
        "export function helper(value) { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&bridge, "export * as ns from \"./helper.mjs\";\n").unwrap();
    fs::write(
        &caller,
        "import { ns } from \"./bridge.mjs\";\nexport function caller(value) { return ns.helper(value); }\n",
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
                "expected caller to resolve to the mjs namespace re-export member, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn keeps_namespace_reexport_unknown_member_calls_fail_closed_without_falling_back() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(&helper, "export function other(): number { return 1; }\n").unwrap();
    fs::write(&bridge, "export * as ns from \"./helper\";\n").unwrap();
    fs::write(
        &caller,
        "import { ns } from \"./bridge\";\nfunction helper(): number { return 2; }\nexport function caller(): number { return ns.helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "unknown namespace re-export members must fail closed without same-named fallback, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_namespace_reexport_missing_targets_fail_closed() {
    let dir = temporary_dir();
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(&bridge, "export * as ns from \"./missing\";\n").unwrap();
    fs::write(
        &caller,
        "import { ns } from \"./bridge\";\nexport function caller(): number { return ns.helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "missing namespace re-export targets must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn traces_javascript_namespace_reexport_member_call_edge_through_star_reexport_bridge_at_position_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let mid = dir.join("mid.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&mid, "export * from \"./helper\";\n").unwrap();
    fs::write(&bridge, "export * as ns from \"./mid\";\n").unwrap();
    fs::write(
        &caller,
        "import { ns } from \"./bridge\";\nexport function caller(value: number): number { return ns.helper(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 16 };
    let live =
        trace_symbol_graph_at_position(&dir, &helper, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 4);
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
    assert_eq!(persisted.indexed_files, 4);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_javascript_namespace_import_member_call_edge_through_named_reexport_bridge() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&bridge, "export { helper } from \"./helper\";\n").unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./bridge\";\nexport function caller(value: number): number { return ns.helper(value); }\n",
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
                "expected caller to resolve to helper through the named re-export bridge, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn traces_javascript_namespace_reexport_member_call_edge_through_named_reexport_alias() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let mid = dir.join("mid.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&mid, "export { helper as other } from \"./helper\";\n").unwrap();
    fs::write(&bridge, "export * as ns from \"./mid\";\n").unwrap();
    fs::write(
        &caller,
        "import { ns } from \"./bridge\";\nexport function caller(value: number): number { return ns.other(value); }\n",
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
                "expected caller to resolve the aliased namespace member to helper, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn keeps_namespace_member_ambiguous_star_reexports_fail_closed() {
    let dir = temporary_dir();
    let first = dir.join("first.ts");
    let second = dir.join("second.ts");
    let mid = dir.join("mid.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(&first, "export function helper(): number { return 1; }\n").unwrap();
    fs::write(&second, "export function helper(): number { return 2; }\n").unwrap();
    fs::write(
        &mid,
        "export * from \"./first\";\nexport * from \"./second\";\n",
    )
    .unwrap();
    fs::write(&bridge, "export * as ns from \"./mid\";\n").unwrap();
    fs::write(
        &caller,
        "import { ns } from \"./bridge\";\nexport function caller(): number { return ns.helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "ambiguous star re-exported namespace members must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_namespace_member_non_exported_symbols_fail_closed() {
    let dir = temporary_dir();
    let mid = dir.join("mid.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(&mid, "function helper(): number { return 1; }\n").unwrap();
    fs::write(&bridge, "export * as ns from \"./mid\";\n").unwrap();
    fs::write(
        &caller,
        "import { ns } from \"./bridge\";\nexport function caller(): number { return ns.helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "non-exported namespace members must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn traces_javascript_namespace_import_default_member_call_edge_at_position_in_live_workspace_and_persisted_index()
 {
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
        "import * as ns from \"./helper\";\nexport function caller(value: number): number { return ns.default(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 27 };
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
fn traces_javascript_namespace_reexport_default_member_call_edge() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export default function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&bridge, "export * as ns from \"./helper\";\n").unwrap();
    fs::write(
        &caller,
        "import { ns } from \"./bridge\";\nexport function caller(value: number): number { return ns.default(value); }\n",
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
                "expected caller to resolve to the namespace re-export default member, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn traces_javascript_namespace_default_member_call_edge_through_default_reexport_bridge() {
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
        "import * as ns from \"./bridge\";\nexport function caller(value: number): number { return ns.default(value); }\n",
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
                "expected caller to resolve to the re-exported default member, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn keeps_namespace_default_member_anonymous_exports_fail_closed() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export default function (value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./helper\";\nexport function caller(value: number): number { return ns.default(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "anonymous default exports must fail closed for namespace default members, callees: {:?}",
        live.callees
    );
}

#[test]
fn traces_javascript_namespace_object_call_edge_to_commonjs_callable_at_position_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "function helper(value: number): number { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./helper\";\nexport function caller(value: number): number { return ns(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 9 };
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
fn traces_javascript_namespace_object_call_edge_to_commonjs_callable_in_cjs_modules() {
    let dir = temporary_dir();
    let helper = dir.join("helper.cjs");
    let caller = dir.join("caller.cjs");

    fs::write(
        &helper,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./helper.cjs\";\nexport function caller(value) { return ns(value); }\n",
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
                "expected caller to resolve to the cjs callable export, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn traces_javascript_namespace_object_call_edge_through_reexport_bridge() {
    let dir = temporary_dir();
    let helper = dir.join("helper.cjs");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(&bridge, "export * as ns from \"./helper.cjs\";\n").unwrap();
    fs::write(
        &caller,
        "import { ns } from \"./bridge\";\nexport function caller(value) { return ns(value); }\n",
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
                "expected caller to resolve to the cjs callable through the bridge, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(helper_symbol.symbol_id, "helper");
}

#[test]
fn keeps_javascript_namespace_object_calls_fail_closed_for_esm_modules() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "export default function helper(value: number): number { return value + 1; }\n",
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
        "ESM namespace objects are not callable, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_javascript_namespace_object_calls_fail_closed_for_mjs_modules() {
    let dir = temporary_dir();
    let helper = dir.join("helper.mjs");
    let caller = dir.join("caller.mjs");

    fs::write(
        &helper,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./helper.mjs\";\nexport function caller(value) { return ns(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        ".mjs namespace objects are never callable, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_javascript_namespace_object_calls_fail_closed_for_non_callable_commonjs() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "function helper(value: number): number { return value + 1; }\nmodule.exports = { helper };\n",
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
        "non-callable CommonJS exports must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn traces_javascript_require_namespace_member_call_edge_at_position_in_live_workspace_and_persisted_index()
 {
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
        "const ns = require(\"./helper\");\nexport function caller(value: number): number { return ns.helper(value); }\n",
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
fn traces_javascript_require_namespace_object_call_edge_at_position_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let legacy = dir.join("legacy.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &legacy,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const legacy = require(\"./legacy.cjs\");\nexport function caller(value: number): number { return legacy(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 9 };
    let live =
        trace_symbol_graph_at_position(&dir, &legacy, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &legacy,
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
fn keeps_javascript_require_missing_module_calls_fail_closed() {
    let dir = temporary_dir();
    let caller = dir.join("caller.ts");

    fs::write(
        &caller,
        "const ns = require(\"./missing\");\nexport function caller(value: number): number { return ns.helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "missing require targets must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn traces_javascript_require_namespace_member_call_edge_to_commonjs_object_export_at_position_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let helper = dir.join("helper.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "function helper(value) { return value + 1; }\nmodule.exports = { helper };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const ns = require(\"./helper.cjs\");\nexport function caller(value: number): number { return ns.helper(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 9 };
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
fn traces_javascript_require_aliased_commonjs_object_export_namespace_member_call_edge() {
    let dir = temporary_dir();
    let helper = dir.join("helper.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "function localHelper(value) { return value + 1; }\nmodule.exports = { helper: localHelper };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const ns = require(\"./helper.cjs\");\nexport function caller(value: number): number { return ns.helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "localHelper");
    assert_eq!(live.callees[0].file_path, normalize_path(&helper));
}

#[test]
fn traces_javascript_require_namespace_member_call_edge_to_commonjs_exports_member_at_position_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let helper = dir.join("helper.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "function helper(value) { return value + 1; }\nexports.helper = helper;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const ns = require(\"./helper.cjs\");\nexport function caller(value: number): number { return ns.helper(value); }\n",
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
fn traces_javascript_require_aliased_commonjs_exports_member_call_edge_in_live_workspace() {
    let dir = temporary_dir();
    let helper = dir.join("helper.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "function localHelper(value) { return value + 1; }\nexports.helper = localHelper;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const ns = require(\"./helper.cjs\");\nexport function caller(value: number): number { return ns.helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "localHelper");
    assert_eq!(live.callees[0].file_path, normalize_path(&helper));
}

#[test]
fn traces_javascript_import_equals_namespace_member_call_edge_at_position_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let helper = dir.join("helper.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "function helper(value) { return value + 1; }\nmodule.exports = { helper };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import ns = require(\"./helper.cjs\");\nexport function caller(value: number): number { return ns.helper(value); }\n",
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
fn traces_javascript_import_equals_namespace_object_call_edge_in_live_workspace() {
    let dir = temporary_dir();
    let helper = dir.join("helper.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &helper,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import fn = require(\"./helper.cjs\");\nexport function caller(value: number): number { return fn(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&helper));
}

#[test]
fn traces_javascript_require_namespace_member_call_edge_through_module_reexport_bridge_at_position_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.helper = helper;\n",
    )
    .unwrap();
    fs::write(&bridge, "module.exports = require(\"./impl.cjs\");\n").unwrap();
    fs::write(
        &caller,
        "const ns = require(\"./bridge.cjs\");\nexport function caller(value: number): number { return ns.helper(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 16 };
    let live = trace_symbol_graph_at_position(&dir, &impl_path, &position, TraceDirection::Callers)
        .unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.symbol.symbol_id, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &impl_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_javascript_require_destructured_member_call_edge_through_module_reexport_bridge_in_live_workspace()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.helper = helper;\n",
    )
    .unwrap();
    fs::write(&bridge, "module.exports = require(\"./impl.cjs\");\n").unwrap();
    fs::write(
        &caller,
        "const { helper } = require(\"./bridge.cjs\");\nexport function caller(value: number): number { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_require_destructured_default_member_call_edge_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.helper = helper;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const { helper = fallback } = require(\"./impl.cjs\");\nconst { helper: bound = fallback } = require(\"./impl.cjs\");\nexport function caller(value: number): number { return helper(value) + bound(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "helper");
    assert_eq!(persisted.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_require_namespace_object_call_edge_through_module_reexport_bridge_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(&bridge, "module.exports = require(\"./impl.cjs\");\n").unwrap();
    fs::write(
        &caller,
        "const fn = require(\"./bridge.cjs\");\nexport function caller(value: number): number { return fn(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "helper");
    assert_eq!(persisted.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_default_import_call_edge_to_cjs_default_member_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.default = helper;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import helper from \"./impl.cjs\";\nexport function caller(value: number): number { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "helper");
    assert_eq!(persisted.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_default_import_call_edge_to_cjs_callable_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("server.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &impl_path,
        "function app(value) { return value + 1; }\nmodule.exports = app;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import app from \"./server.cjs\";\nexport function caller(value: number): number { return app(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "app");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "app");
    assert_eq!(persisted.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_default_import_call_edge_to_cjs_default_member_through_wholesale_chain_in_live_workspace()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.default = helper;\n",
    )
    .unwrap();
    fs::write(&bridge, "module.exports = require(\"./impl.cjs\");\n").unwrap();
    fs::write(
        &caller,
        "import helper from \"./bridge.cjs\";\nexport function caller(value: number): number { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_namespace_default_member_call_edge_to_cjs_default_member_in_live_workspace() {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.default = helper;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./impl.cjs\";\nexport function caller(value: number): number { return ns.default(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn keeps_javascript_namespace_default_member_call_to_cjs_callable_fail_closed() {
    let dir = temporary_dir();
    let impl_path = dir.join("server.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "function app(value) { return value + 1; }\nmodule.exports = app;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./server.cjs\";\nexport function caller(value: number): number { return ns.default(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "a callable module.exports does not expose a .default member, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_javascript_shadowed_exports_alias_member_calls_fail_closed_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let helper = dir.join("helper.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
            &helper,
            "function helper(value) { return value + 1; }\nfunction app() {}\nexports.helper = helper;\nmodule.exports = app;\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "const ns = require(\"./helper.cjs\");\nexport function caller(value: number): number { return ns.helper(value); }\n",
        )
        .unwrap();

    let position = Position { row: 0, column: 16 };
    let live =
        trace_symbol_graph_at_position(&dir, &helper, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, "helper");
    assert!(
        live.callers.is_empty(),
        "shadowed exports alias members must not resolve, callers: {:?}",
        live.callers
    );

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
    assert!(
        persisted.callers.is_empty(),
        "persisted shadowed exports alias members must not resolve, callers: {:?}",
        persisted.callers
    );
}

#[test]
fn traces_javascript_module_exports_attached_member_call_edge_at_position_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let helper = dir.join("helper.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
            &helper,
            "function app() {}\nfunction extraFn(value) { return value + 1; }\nmodule.exports = app;\nmodule.exports.extra = extraFn;\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "const ns = require(\"./helper.cjs\");\nexport function caller(value: number): number { return ns.extra(value); }\n",
        )
        .unwrap();

    let position = Position { row: 1, column: 20 };
    let live =
        trace_symbol_graph_at_position(&dir, &helper, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, "extraFn");
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
    assert_eq!(persisted.symbol.symbol_id, "extraFn");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_inline_require_member_call_edge_at_position_in_live_workspace_and_persisted_index() {
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
        "export function caller(value: number): number { return require(\"./helper\").helper(value); }\n",
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
fn traces_inline_require_object_call_edge_to_commonjs_callable_at_position_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let helper = dir.join("helper.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "function helper(value: number): number { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "export function caller(value: number): number { return require(\"./helper.cjs\")(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 9 };
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
fn keeps_inline_require_member_calls_fail_closed_without_falling_back() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let caller = dir.join("caller.ts");

    fs::write(&helper, "export function other(): number { return 1; }\n").unwrap();
    fs::write(
        &caller,
        "function helper(): number { return 2; }\nexport function caller(): number { return require(\"./helper\").helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "unknown inline require members must fail closed without same-named fallback, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_inline_require_object_calls_fail_closed_for_esm_modules() {
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
        "export function caller(value: number): number { return require(\"./helper\")(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "ESM inline require objects are never callable and must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_inline_require_member_calls_fail_closed_for_missing_module() {
    let dir = temporary_dir();
    let caller = dir.join("caller.ts");

    fs::write(
        &caller,
        "function helper(): number { return 2; }\nexport function caller(): number { return require(\"./missing\").helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "missing inline require modules must fail closed without same-named fallback, callees: {:?}",
        live.callees
    );
}

#[test]
fn traces_javascript_module_valued_export_member_call_edge_to_cjs_callable_at_position_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports.helper = require(\"./impl.cjs\");\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./bridge.cjs\";\nexport function caller(value) { return ns.helper(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 9 };
    let live = trace_symbol_graph_at_position(&dir, &impl_path, &position, TraceDirection::Callers)
        .unwrap();
    assert_eq!(live.symbol.symbol_id, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &impl_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_javascript_module_valued_object_literal_export_member_call_edge_to_cjs_callable_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports = { helper: require(\"./impl.cjs\") };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./bridge.cjs\";\nexport function caller(value) { return ns.helper(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 9 };
    let live = trace_symbol_graph_at_position(&dir, &impl_path, &position, TraceDirection::Callers)
        .unwrap();
    assert_eq!(live.symbol.symbol_id, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &impl_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_javascript_module_valued_export_member_call_edge_to_reexported_member_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.run = helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports.helper = require(\"./impl.cjs\").run;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./bridge.cjs\";\nexport function caller(value) { return ns.helper(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 9 };
    let live = trace_symbol_graph_at_position(&dir, &impl_path, &position, TraceDirection::Callers)
        .unwrap();
    assert_eq!(live.symbol.symbol_id, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &impl_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_javascript_namespace_member_call_edge_through_object_literal_spread_reexport_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.helper = helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports = { ...require(\"./impl.cjs\") };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./bridge.cjs\";\nexport function caller(value) { return ns.helper(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 9 };
    let live = trace_symbol_graph_at_position(&dir, &impl_path, &position, TraceDirection::Callers)
        .unwrap();
    assert_eq!(live.symbol.symbol_id, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &impl_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_javascript_destructured_member_call_edge_through_object_literal_spread_reexport_in_live_workspace()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.helper = helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports = { ...require(\"./impl.cjs\") };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const { helper } = require(\"./bridge.cjs\");\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_default_import_call_edge_through_object_literal_spread_reexport_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.default = helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports = { ...require(\"./impl.cjs\") };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import helper from \"./bridge.cjs\";\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "helper");
    assert_eq!(persisted.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_default_import_call_edge_through_object_literal_module_valued_default_member_in_live_workspace()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports = { default: require(\"./impl.cjs\") };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import helper from \"./bridge.cjs\";\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_namespace_default_member_call_edge_through_object_literal_spread_reexport_in_live_workspace()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.default = helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports = { ...require(\"./impl.cjs\") };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./bridge.cjs\";\nexport function caller(value) { return ns.default(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn keeps_javascript_default_import_edges_fail_closed_for_conflicting_object_literal_defaults() {
    let dir = temporary_dir();
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &bridge,
        "function first(value) { return value + 1; }\nfunction second(value) { return value + 2; }\nmodule.exports = { default: first, default: second };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import selected from \"./bridge.cjs\";\nexport function caller(value) { return selected(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "conflicting object-literal defaults must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn traces_javascript_destructured_member_call_edge_through_object_literal_module_valued_whole_alias_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports = { helper: require(\"./impl.cjs\") };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const { helper } = require(\"./bridge.cjs\");\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "helper");
    assert_eq!(persisted.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_destructured_member_call_edge_through_object_literal_module_valued_member_alias_in_live_workspace()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.run = helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports = { helper: require(\"./impl.cjs\").run };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const { helper } = require(\"./bridge.cjs\");\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_named_import_call_edge_through_object_literal_module_valued_member_alias_in_live_workspace()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nexports.run = helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports = { helper: require(\"./impl.cjs\").run };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./bridge.cjs\";\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_destructured_member_call_edge_through_member_assignment_module_valued_alias_in_live_workspace()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(&bridge, "exports.helper = require(\"./impl.cjs\");\n").unwrap();
    fs::write(
        &caller,
        "const { helper } = require(\"./bridge.cjs\");\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_destructured_member_call_edge_through_transitive_module_valued_alias_chain_in_live_workspace()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let mid = dir.join("mid.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
    )
    .unwrap();
    fs::write(&mid, "exports.run = require(\"./impl.cjs\");\n").unwrap();
    fs::write(
        &bridge,
        "module.exports = { helper: require(\"./mid.cjs\").run };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const { helper } = require(\"./bridge.cjs\");\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn traces_javascript_destructured_member_constructor_call_edge_through_module_valued_member_alias_in_live_workspace()
 {
    let dir = temporary_dir();
    let impl_path = dir.join("impl.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &impl_path,
        "class Helper { helper() { return 1; } }\nexports.Klass = Helper;\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports = { Helper: require(\"./impl.cjs\").Klass };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const { Helper } = require(\"./bridge.cjs\");\nexport function caller() { return new Helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Helper");
    assert_eq!(live.callees[0].file_path, normalize_path(&impl_path));
}

#[test]
fn keeps_javascript_destructured_member_calls_fail_closed_for_ambiguous_module_valued_aliases() {
    let dir = temporary_dir();
    let left = dir.join("left.cjs");
    let right = dir.join("right.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(&left, "function a() { return 1; }\nmodule.exports = a;\n").unwrap();
    fs::write(&right, "function b() { return 2; }\nmodule.exports = b;\n").unwrap();
    fs::write(
        &bridge,
        "module.exports = { helper: require(\"./left.cjs\"), helper: require(\"./right.cjs\") };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const { helper } = require(\"./bridge.cjs\");\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "ambiguous module-valued aliases must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_javascript_destructured_member_calls_fail_closed_for_missing_module_valued_targets() {
    let dir = temporary_dir();
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &bridge,
        "module.exports = { helper: require(\"./missing.cjs\") };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const { helper } = require(\"./bridge.cjs\");\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "missing module-valued targets must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_javascript_destructured_member_calls_fail_closed_for_non_callable_whole_module_aliases() {
    let dir = temporary_dir();
    let obj_path = dir.join("obj.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &obj_path,
        "function other() { return 1; }\nmodule.exports = { other };\n",
    )
    .unwrap();
    fs::write(
        &bridge,
        "module.exports = { helper: require(\"./obj.cjs\") };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const { helper } = require(\"./bridge.cjs\");\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "non-callable whole-module aliases must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_javascript_destructured_member_calls_fail_closed_for_module_valued_alias_cycles() {
    let dir = temporary_dir();
    let bridge = dir.join("bridge.cjs");
    let impl_path = dir.join("impl.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &bridge,
        "module.exports = { helper: require(\"./impl.cjs\").run };\n",
    )
    .unwrap();
    fs::write(
        &impl_path,
        "module.exports = { run: require(\"./bridge.cjs\").helper };\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const { helper } = require(\"./bridge.cjs\");\nexport function caller(value) { return helper(value); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "cyclic module-valued aliases must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_javascript_module_valued_export_member_calls_fail_closed_for_non_callable_aliases() {
    let dir = temporary_dir();
    let obj_path = dir.join("obj.cjs");
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &obj_path,
        "function other() { return 1; }\nmodule.exports = { other };\n",
    )
    .unwrap();
    fs::write(&bridge, "module.exports.helper = require(\"./obj.cjs\");\n").unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./bridge.cjs\";\nexport function caller() { return ns.helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "non-callable module-valued export members must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_javascript_module_valued_export_member_calls_fail_closed_for_missing_aliases() {
    let dir = temporary_dir();
    let bridge = dir.join("bridge.cjs");
    let caller = dir.join("caller.ts");

    fs::write(
        &bridge,
        "module.exports.helper = require(\"./missing.cjs\");\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./bridge.cjs\";\nexport function caller() { return ns.helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "missing module-valued export aliases must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn traces_javascript_constructor_call_edge_to_named_import_class_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let counter = dir.join("counter.ts");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &counter,
        "export class Counter { constructor(value) { this.value = value; } }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import { Counter } from \"./counter\";\nexport function caller() { return new Counter(1); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 13 };
    let live =
        trace_symbol_graph_at_position(&dir, &counter, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, "Counter");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &counter,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "Counter");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_javascript_namespace_constructor_call_edge_to_exported_class_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let counter = dir.join("counter.ts");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(&counter, "export class Counter { constructor() {} }\n").unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./counter\";\nexport function caller() { return new ns.Counter(); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 13 };
    let live =
        trace_symbol_graph_at_position(&dir, &counter, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, "Counter");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &counter,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "Counter");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

#[test]
fn traces_javascript_constructor_call_edge_to_local_class() {
    let dir = temporary_dir();
    let app = dir.join("app.ts");

    fs::write(
        &app,
        "class Counter { constructor() {} }\nexport function caller() { return new Counter(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.symbol.semantic_path, "caller");
    let counter = live
        .callees
        .iter()
        .find(|symbol| symbol.semantic_path == "Counter")
        .unwrap_or_else(|| {
            panic!(
                "expected caller to construct the local class, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(counter.symbol_id, "Counter");
}

#[test]
fn traces_javascript_default_import_constructor_call_edge_to_exported_class() {
    let dir = temporary_dir();
    let counter = dir.join("counter.ts");
    let caller = dir.join("caller.ts");

    fs::write(
        &counter,
        "export default class Counter { constructor() {} }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import Counter from \"./counter\";\nexport function caller() { return new Counter(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.symbol.semantic_path, "caller");
    let counter = live
        .callees
        .iter()
        .find(|symbol| symbol.semantic_path == "Counter")
        .unwrap_or_else(|| {
            panic!(
                "expected caller to construct the default-imported class, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(counter.symbol_id, "Counter");
}

#[test]
fn traces_javascript_require_binding_constructor_call_edge_to_commonjs_callable() {
    let dir = temporary_dir();
    let counter = dir.join("counter.js");
    let caller = dir.join("caller.js");

    fs::write(
        &counter,
        "class Counter { constructor() {} }\nmodule.exports = Counter;\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "const Counter = require(\"./counter\");\nexport function caller() { return new Counter(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.symbol.semantic_path, "caller");
    let counter = live
        .callees
        .iter()
        .find(|symbol| symbol.semantic_path == "Counter")
        .unwrap_or_else(|| {
            panic!(
                "expected caller to construct the require-bound CommonJS callable, callees: {:?}",
                live.callees
            )
        });
    assert_eq!(counter.symbol_id, "Counter");
}

#[test]
fn keeps_javascript_constructor_calls_fail_closed_for_non_namespace_receivers() {
    let dir = temporary_dir();
    let app = dir.join("app.ts");

    fs::write(
        &app,
        "class Missing { }\nexport function caller(ns) { return new ns.Missing(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.symbol.semantic_path, "caller");
    assert!(
        live.callees
            .iter()
            .all(|symbol| symbol.semantic_path != "Missing"),
        "constructor calls through non-namespace receivers must fail closed, callees: {:?}",
        live.callees
    );
}

#[test]
fn keeps_javascript_constructor_calls_fail_closed_for_missing_namespace_members() {
    let dir = temporary_dir();
    let counter = dir.join("counter.ts");
    let caller = dir.join("caller.ts");

    fs::write(&counter, "export class Present { }\n").unwrap();
    fs::write(
        &caller,
        "import * as ns from \"./counter\";\nexport function caller() { return new ns.Missing(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.symbol.semantic_path, "caller");
    assert!(
        live.callees.is_empty(),
        "constructor calls through missing namespace members must fail closed, callees: {:?}",
        live.callees
    );
}
