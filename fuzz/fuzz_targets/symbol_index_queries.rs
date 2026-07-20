#![no_main]

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use arborist_core::{
    list_symbols_from_index, read_symbol_from_index, rebuild_symbol_index,
    search_symbols_from_index, trace_symbol_graph_from_index_with_timeout, TraceDirection,
};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 192 * 1024;
const MAX_SELECTOR_BYTES: usize = 4 * 1024;
static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(0);

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    let database_id = NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed);
    let workspace_root = std::env::temp_dir().join(format!(
        "arborist-symbol-index-queries-fuzz-{}-{database_id}",
        std::process::id()
    ));
    let source_path = workspace_root.join("module.py");
    let db_path = workspace_root.join("symbols.db");
    let _ = fs::create_dir_all(&workspace_root);
    let _ = fs::write(
        &source_path,
        "def helper(value: int) -> int:\n    return value + 1\n\ndef caller(value: int) -> int:\n    return helper(value)\n",
    );
    let _ = rebuild_symbol_index(&workspace_root, &db_path);

    let split = data.len() / 2;
    let query = String::from_utf8_lossy(&data[..split.min(MAX_SELECTOR_BYTES)]);
    let symbol_path = String::from_utf8_lossy(
        &data[split..(split.saturating_add(MAX_SELECTOR_BYTES)).min(data.len())],
    );

    let _ = list_symbols_from_index(&db_path, 32);
    let _ = search_symbols_from_index(&db_path, &query, 32);
    let _ = read_symbol_from_index(&db_path, &symbol_path);
    let _ = read_symbol_from_index(&db_path, "caller");
    let _ = trace_symbol_graph_from_index_with_timeout(
        &db_path,
        &symbol_path,
        TraceDirection::Both,
        Some(10),
    );
    let _ = trace_symbol_graph_from_index_with_timeout(
        &db_path,
        "caller",
        TraceDirection::Both,
        Some(10),
    );

    let _ = fs::remove_dir_all(workspace_root);
});
