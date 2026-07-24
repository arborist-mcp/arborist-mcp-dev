pub(super) use std::fs;
pub(super) use std::path::Path;

pub(super) use super::support::temporary_dir;
pub(super) use super::{
    TraceDirection, rebuild_symbol_index, trace_symbol_graph, trace_symbol_graph_from_index,
};

mod bindings;
mod class_scope;
mod core;
mod imports_calls;
mod match_case;
