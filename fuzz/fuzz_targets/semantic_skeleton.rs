#![no_main]

use std::path::Path;

use arborist_core::{LanguageId, builtin_language_registry, get_semantic_skeleton, supported_languages};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 192 * 1024;

fn registered_language_ids() -> Vec<LanguageId> {
    supported_languages()
        .into_iter()
        .filter_map(|name| serde_json::from_str::<LanguageId>(&format!("\"{name}\"")).ok())
        .collect()
}

fn extension_for(language_ids: &[LanguageId], selector: u8) -> &'static str {
    let language_id = language_ids[selector as usize % language_ids.len()];
    builtin_language_registry()
        .descriptor(language_id)
        .expect("registered language must have a descriptor")
        .extensions[0]
}

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    let language_ids = registered_language_ids();
    let extension = extension_for(&language_ids, data.first().copied().unwrap_or_default());
    let source = String::from_utf8_lossy(data.get(1..).unwrap_or_default());
    let path = Path::new("fuzz").with_extension(extension);

    let _ = get_semantic_skeleton(&path, &source, 8, &[]);
});