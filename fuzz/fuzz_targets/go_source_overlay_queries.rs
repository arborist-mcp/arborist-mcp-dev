#![no_main]

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use arborist_core::{
    Position, TraceDirection, list_symbols_with_source_filtered,
    read_symbol_at_position_with_source, read_symbol_discovery_context_with_source,
    read_symbol_with_source, search_symbols_with_source_filtered,
    trace_symbol_graph_with_source_and_timeout,
};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 192 * 1024;
const MAX_SELECTOR_BYTES: usize = 4 * 1024;
const BASELINE_SOURCE: &str = "package metrics\n\ntype Worker interface { Run(value int) error }\nfunc NewWorker[T any]() Worker { return nil }\nfunc (worker Worker) Run(value int) error { return nil }\nfunc caller() error { worker := NewWorker[int](); return worker.Run(1) }\n";
static NEXT_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    let workspace_id = NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "arborist-go-source-overlay-queries-fuzz-{}-{workspace_id}",
        std::process::id()
    ));
    let workspace_root = root.join("workspace");
    let source_path = workspace_root.join("metrics.go");
    let _ = fs::create_dir_all(&workspace_root);
    let _ = fs::write(&source_path, BASELINE_SOURCE);

    let source_end = data.len() / 2;
    let source = String::from_utf8_lossy(&data[..source_end]);
    let query = String::from_utf8_lossy(
        &data[source_end..(source_end.saturating_add(MAX_SELECTOR_BYTES)).min(data.len())],
    );
    let position_bytes = data.get(..8).unwrap_or_default();
    let position = Position {
        row: u32::from_le_bytes(
            position_bytes
                .get(..4)
                .unwrap_or(&[0; 4])
                .try_into()
                .unwrap(),
        ) as usize,
        column: u32::from_le_bytes(
            position_bytes
                .get(4..8)
                .unwrap_or(&[0; 4])
                .try_into()
                .unwrap(),
        ) as usize,
    };
    let overlay_path = match data.first().copied().unwrap_or_default() % 5 {
        0 => source_path.clone(),
        1 => workspace_root.join("added.go"),
        2 => workspace_root.join("vendor").join("ignored.go"),
        3 => workspace_root.join("notes.txt"),
        _ => root.join("outside.go"),
    };

    let _ =
        list_symbols_with_source_filtered(&workspace_root, &overlay_path, &source, 32, None, None);
    let _ = search_symbols_with_source_filtered(
        &workspace_root,
        &overlay_path,
        &source,
        &query,
        32,
        None,
        None,
    );
    for symbol_path in [&query, "caller", "Worker::Run", "NewWorker"] {
        let _ = trace_symbol_graph_with_source_and_timeout(
            &workspace_root,
            &overlay_path,
            &source,
            symbol_path,
            TraceDirection::Both,
            Some(10),
        );
    }
    let _ = read_symbol_with_source(&workspace_root, &overlay_path, &source, &query);
    let _ = read_symbol_at_position_with_source(&workspace_root, &overlay_path, &source, &position);
    let _ = read_symbol_discovery_context_with_source(
        &workspace_root,
        &overlay_path,
        &source,
        &query,
        TraceDirection::Both,
        2,
        32,
    );

    let baseline_position = Position { row: 6, column: 20 };
    let baseline_queries = ["caller", "Worker::Run", "NewWorker"];
    for symbol_path in baseline_queries {
        let _ =
            read_symbol_with_source(&workspace_root, &source_path, BASELINE_SOURCE, symbol_path);
        let _ = read_symbol_at_position_with_source(
            &workspace_root,
            &source_path,
            BASELINE_SOURCE,
            &baseline_position,
        );
        let _ = read_symbol_discovery_context_with_source(
            &workspace_root,
            &source_path,
            BASELINE_SOURCE,
            symbol_path,
            TraceDirection::Both,
            2,
            32,
        );
    }

    let _ = fs::remove_dir_all(root);
});
