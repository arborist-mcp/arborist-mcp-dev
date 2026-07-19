#![no_main]

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use arborist_core::inspect_symbol_index;
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 192 * 1024;
static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(0);

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    let database_id = NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed);
    let db_path = std::env::temp_dir().join(format!(
        "arborist-symbol-index-fuzz-{}-{database_id}.db",
        std::process::id()
    ));
    let _ = fs::write(&db_path, data);
    let _ = inspect_symbol_index(&db_path);
    let _ = fs::remove_file(&db_path);
});
