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
