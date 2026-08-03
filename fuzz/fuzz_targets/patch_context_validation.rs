#![no_main]

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use arborist_core::{
    Position, TraceDirection, rebuild_symbol_index,
    validate_patch_with_discovery_context_at_position_from_index_with_timeout,
    validate_patch_with_discovery_context_at_position_with_timeout,
    validate_patch_with_graph_context_from_index_with_timeout,
    validate_patch_with_graph_context_with_timeout,
    validate_patch_with_neighborhood_context_at_position_from_index_with_timeout,
    validate_patch_with_neighborhood_context_at_position_with_timeout,
    validate_patch_with_trace_context_from_index_with_timeout,
    validate_patch_with_trace_context_with_timeout,
};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 192 * 1024;
const MAX_SELECTOR_BYTES: usize = 4 * 1024;
const MAX_TRACE_TIMEOUT_MS: u64 = 100;
const MAX_DEPTH: usize = 2;
const MAX_NODES: usize = 32;
const BASELINE_SOURCE: &str = concat!(
    "def helper(value: int) -> int:\n",
    "    return value + 1\n\n",
    "def caller(value: int) -> int:\n",
    "    return helper(value)\n",
);
const BASELINE_REPLACEMENT: &str =
    concat!("def helper(value: int) -> int:\n", "    return value + 2\n",);
static NEXT_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

fn direction(control: u8) -> TraceDirection {
    match control % 3 {
        0 => TraceDirection::Callers,
        1 => TraceDirection::Callees,
        _ => TraceDirection::Both,
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let workspace_id = NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
    let workspace_root = std::env::temp_dir().join(format!(
        "arborist-patch-context-validation-fuzz-{}-{workspace_id}",
        std::process::id()
    ));
    let source_path = workspace_root.join("module.py");
    let db_path = workspace_root.join("symbols.db");
    let _ = fs::create_dir_all(&workspace_root);
    let _ = fs::write(&source_path, BASELINE_SOURCE);
    let _ = rebuild_symbol_index(&workspace_root, &db_path);
    let control = data.first().copied().unwrap_or_default();
    let payload = data.get(1..).unwrap_or_default();
    let source_end = payload.len() / 2;
    let target_end = (source_end.saturating_add(MAX_SELECTOR_BYTES)).min(payload.len());
    let generated_source = String::from_utf8_lossy(&payload[..source_end]);
    let generated_target = String::from_utf8_lossy(&payload[source_end..target_end]);
    let generated_replacement = String::from_utf8_lossy(&payload[target_end..]);
    let source = if control & 0b0001 == 0 {
        BASELINE_SOURCE
    } else {
        &generated_source
    };
    let semantic_target = if control & 0b0010 == 0 {
        "helper"
    } else {
        &generated_target
    };
    let replacement = if control & 0b0100 == 0 {
        BASELINE_REPLACEMENT
    } else {
        &generated_replacement
    };
    let bypass_reason = (control & 0b1000 != 0).then_some("fuzz commit override");
    let trace_direction = direction(control >> 4);
    let position = Position {
        row: usize::from(*data.get(1).unwrap_or(&0)),
        column: usize::from(*data.get(2).unwrap_or(&0)),
    };
    match (control >> 6) & 0b11 {
        0 => {
            let _ = validate_patch_with_trace_context_with_timeout(
                &workspace_root,
                &source_path,
                source,
                semantic_target,
                replacement,
                bypass_reason,
                trace_direction,
                Some(MAX_TRACE_TIMEOUT_MS),
            );
            let _ = validate_patch_with_trace_context_from_index_with_timeout(
                &db_path,
                &source_path,
                source,
                semantic_target,
                replacement,
                bypass_reason,
                trace_direction,
                Some(MAX_TRACE_TIMEOUT_MS),
            );
        }
        1 => {
            let _ = validate_patch_with_graph_context_with_timeout(
                &workspace_root,
                &source_path,
                source,
                semantic_target,
                replacement,
                bypass_reason,
                trace_direction,
                MAX_DEPTH,
                MAX_NODES,
                Some(MAX_TRACE_TIMEOUT_MS),
            );
            let _ = validate_patch_with_graph_context_from_index_with_timeout(
                &db_path,
                &source_path,
                source,
                semantic_target,
                replacement,
                bypass_reason,
                trace_direction,
                MAX_DEPTH,
                MAX_NODES,
                Some(MAX_TRACE_TIMEOUT_MS),
            );
        }
        2 => {
            let _ = validate_patch_with_neighborhood_context_at_position_with_timeout(
                &workspace_root,
                &source_path,
                source,
                &position,
                replacement,
                bypass_reason,
                trace_direction,
                MAX_DEPTH,
                MAX_NODES,
                Some(MAX_TRACE_TIMEOUT_MS),
            );
            let _ = validate_patch_with_neighborhood_context_at_position_from_index_with_timeout(
                &db_path,
                &source_path,
                source,
                &position,
                replacement,
                bypass_reason,
                trace_direction,
                MAX_DEPTH,
                MAX_NODES,
                Some(MAX_TRACE_TIMEOUT_MS),
            );
        }
        _ => {
            let _ = validate_patch_with_discovery_context_at_position_with_timeout(
                &workspace_root,
                &source_path,
                source,
                &position,
                replacement,
                bypass_reason,
                trace_direction,
                MAX_DEPTH,
                MAX_NODES,
                Some(MAX_TRACE_TIMEOUT_MS),
            );
            let _ = validate_patch_with_discovery_context_at_position_from_index_with_timeout(
                &db_path,
                &source_path,
                source,
                &position,
                replacement,
                bypass_reason,
                trace_direction,
                MAX_DEPTH,
                MAX_NODES,
                Some(MAX_TRACE_TIMEOUT_MS),
            );
        }
    }
    let _ = fs::remove_dir_all(workspace_root);
});
