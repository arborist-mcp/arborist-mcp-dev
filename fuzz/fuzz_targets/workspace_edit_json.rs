#![no_main]

use arborist_core::{WorkspacePositionEdits, preview_workspace_position_edits};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 192 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    let Ok(files) = serde_json::from_slice::<Vec<WorkspacePositionEdits>>(data) else {
        return;
    };
    let _ = preview_workspace_position_edits(&files);
});
