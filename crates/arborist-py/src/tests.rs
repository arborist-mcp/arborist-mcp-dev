mod core_index_bindings;
mod json_args;
mod patch_apply_bindings;
mod patch_validation_bindings;
mod replay_payload_validation;
mod replay_semantic_validation;
mod replay_trace_validation;
mod symbol_query_bindings;
mod vfs_bindings;

use super::{
    ArboristCore, NeighborhoodBounds, PatchAstNodeResult, TraceSymbolGraphResult, parse_json_arg,
};
use crate::json_args::{MAX_JSON_ARG_BYTES, MAX_JSON_ARG_DEPTH};
use arborist_core::{
    MAX_PATCH_TIMEOUT_MS, MAX_SYMBOL_INDEX_REGISTRY_TIMEOUT_MS, MAX_VIRTUAL_FILE_COMMIT_TIMEOUT_MS,
    MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS, MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS, PositionEdit,
    TraceDirection, patch_ast_node_from_path, trace_symbol_graph,
};
use serde_json::Value;
use std::fs;
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

fn prepare_python() {
    static PREPARE: Once = Once::new();
    PREPARE.call_once(pyo3::prepare_freethreaded_python);
}
