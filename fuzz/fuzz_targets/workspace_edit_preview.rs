#![no_main]

use arborist_core::{
    Position, PositionEdit, WorkspacePositionEdits, preview_workspace_position_edits,
};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 192 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    let source_end = data.len() / 2;
    let first_text_end = source_end + (data.len() - source_end) / 2;
    let source = String::from_utf8_lossy(&data[..source_end]).into_owned();
    let first_new_text = String::from_utf8_lossy(&data[source_end..first_text_end]).into_owned();
    let second_new_text = String::from_utf8_lossy(&data[first_text_end..]).into_owned();
    let position_bytes = data.get(..8).unwrap_or_default();
    let row = u32::from_le_bytes(position_bytes.get(..4).unwrap_or(&[0; 4]).try_into().unwrap())
        as usize;
    let column =
        u32::from_le_bytes(position_bytes.get(4..8).unwrap_or(&[0; 4]).try_into().unwrap())
            as usize;
    let first = Position { row, column };
    let second = Position {
        row: column,
        column: row,
    };

    let _ = preview_workspace_position_edits(&[WorkspacePositionEdits {
        file_path: "fuzz.py".to_string(),
        source: Some(source),
        edits: vec![
            PositionEdit {
                start: first.clone(),
                end: second.clone(),
                new_text: first_new_text,
            },
            PositionEdit {
                start: second,
                end: first,
                new_text: second_new_text,
            },
        ],
    }]);
});
