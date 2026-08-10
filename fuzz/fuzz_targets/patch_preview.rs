#![no_main]

use std::path::Path;

use arborist_core::{
    LanguageCapabilities, LanguageId, builtin_language_registry, preview_patch_ast_node,
    supported_languages,
};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 192 * 1024;

fn patch_language_ids() -> Vec<LanguageId> {
    supported_languages()
        .into_iter()
        .filter_map(|name| serde_json::from_str::<LanguageId>(&format!("\"{name}\"")).ok())
        .filter(|language_id| {
            builtin_language_registry()
                .descriptor(*language_id)
                .is_some_and(|descriptor| {
                    descriptor
                        .capabilities
                        .contains(LanguageCapabilities::PATCH_TARGETING)
                })
        })
        .collect()
}

fn extension_for(language_ids: &[LanguageId], selector: u8) -> &'static str {
    let language_id = language_ids[selector as usize % language_ids.len()];
    builtin_language_registry()
        .descriptor(language_id)
        .expect("patch language must have a descriptor")
        .extensions[0]
}

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    let language_ids = patch_language_ids();
    let extension = extension_for(&language_ids, data.first().copied().unwrap_or_default());
    let payload = data.get(1..).unwrap_or_default();
    let source_end = payload.len() / 3;
    let target_end = source_end + (payload.len() - source_end) / 2;
    let source = String::from_utf8_lossy(&payload[..source_end]);
    let target = String::from_utf8_lossy(&payload[source_end..target_end]);
    let replacement = String::from_utf8_lossy(&payload[target_end..]);
    let path = Path::new("fuzz").with_extension(extension);

    let _ = preview_patch_ast_node(&path, &source, &target, &replacement, None);
});