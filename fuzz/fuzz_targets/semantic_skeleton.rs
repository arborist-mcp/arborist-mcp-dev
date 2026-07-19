#![no_main]

use std::path::Path;

use arborist_core::get_semantic_skeleton;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let (extension, source) = match data.first().copied().unwrap_or_default() % 3 {
        0 => ("fuzz.py", data.get(1..).unwrap_or_default()),
        1 => ("fuzz.c", data.get(1..).unwrap_or_default()),
        _ => ("fuzz.cpp", data.get(1..).unwrap_or_default()),
    };
    let source = String::from_utf8_lossy(source);

    let _ = get_semantic_skeleton(Path::new(extension), &source, 8, &[]);
});
