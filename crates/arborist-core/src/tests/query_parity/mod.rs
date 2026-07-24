pub(super) use std::fs;
pub(super) use std::path::Path;

pub(super) use super::support::temporary_dir;
pub(super) use super::{
    Position, TraceDirection, VirtualFileSystem, list_symbols, list_symbols_context,
    list_symbols_context_from_index, list_symbols_discovery_context,
    list_symbols_discovery_context_from_index, list_symbols_filtered, list_symbols_from_index,
    list_symbols_from_index_filtered, list_symbols_neighborhood_context,
    list_symbols_neighborhood_context_from_index, patch_ast_node_at_position, read_symbol,
    read_symbol_at_position, read_symbol_at_position_from_index, read_symbol_context,
    read_symbol_context_from_index, read_symbol_discovery_context,
    read_symbol_discovery_context_at_position,
    read_symbol_discovery_context_at_position_from_index,
    read_symbol_discovery_context_at_position_with_source,
    read_symbol_discovery_context_from_index, read_symbol_from_index,
    read_symbol_neighborhood_context, read_symbol_neighborhood_context_from_index,
    rebuild_symbol_index, refresh_symbol_index, search_symbols, search_symbols_context,
    search_symbols_context_from_index, search_symbols_discovery_context,
    search_symbols_discovery_context_from_index, search_symbols_filtered,
    search_symbols_from_index, search_symbols_from_index_filtered,
    search_symbols_neighborhood_context, search_symbols_neighborhood_context_from_index,
    trace_symbol_graph_at_position, trace_symbol_graph_at_position_from_index,
    trace_symbol_graph_at_position_with_source, trace_symbol_graph_from_index,
    trace_symbol_neighborhood, trace_symbol_neighborhood_at_position,
    trace_symbol_neighborhood_at_position_from_index, trace_symbol_neighborhood_from_index,
    validate_patch_with_discovery_context_at_position,
    validate_patch_with_trace_context_at_position,
};
pub(super) use crate::language::normalize_path;

mod index;
mod list;
mod patch;
mod read;
mod search;
mod trace;
