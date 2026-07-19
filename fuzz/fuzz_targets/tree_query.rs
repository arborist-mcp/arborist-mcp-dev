#![no_main]

use std::path::Path;

use arborist_core::execute_tree_query_with_limit;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let split = data.len() / 2;
    let source = String::from_utf8_lossy(&data[..split]);
    let query = String::from_utf8_lossy(&data[split..]);

    let _ = execute_tree_query_with_limit(Path::new("fuzz.py"), &source, &query, 32);
});
