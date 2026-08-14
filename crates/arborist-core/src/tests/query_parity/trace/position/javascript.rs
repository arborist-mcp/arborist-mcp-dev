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
