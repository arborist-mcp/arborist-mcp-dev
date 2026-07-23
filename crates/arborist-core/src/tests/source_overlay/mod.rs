use std::fs;

use rusqlite::Connection;

use super::support::{normalize_string_path, temporary_dir};
use crate::{
    Position, SymbolQueryContext, TraceDirection, list_symbols_from_index_with_source_filtered,
    read_symbol_context_from_index_with_source, rebuild_symbol_index,
    search_symbols_from_index_with_source_filtered,
    trace_symbol_graph_at_position_from_index_with_source,
    trace_symbol_graph_from_index_with_source, validate_patch_with_trace_context_from_index,
    validate_patch_with_trace_context_from_path,
};

mod constructors;
mod core;
mod std_get;
mod wrappers;
