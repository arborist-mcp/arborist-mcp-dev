#![no_main]

use std::path::Path;

use arborist_core::preview_patch_ast_node;
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 192 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    let language = data.first().copied().unwrap_or_default() % 3;
    let payload = data.get(1..).unwrap_or_default();
    let source_end = payload.len() / 3;
    let target_end = source_end + (payload.len() - source_end) / 2;
    let source = String::from_utf8_lossy(&payload[..source_end]);
    let target = String::from_utf8_lossy(&payload[source_end..target_end]);
    let replacement = String::from_utf8_lossy(&payload[target_end..]);
    let path = match language {
        0 => Path::new("fuzz.py"),
        1 => Path::new("fuzz.c"),
        _ => Path::new("fuzz.cpp"),
    };

    let _ = preview_patch_ast_node(path, &source, &target, &replacement, None);
});
