use std::fs;

use rusqlite::Connection;

use super::support::{
    create_legacy_symbol_index_schema_without_reference_names, create_minimal_symbol_index_schema,
    symbol_table_columns, temporary_dir,
};
use crate::language::normalize_path;
use crate::{
    TraceDirection, rebuild_symbol_index, refresh_symbol_index_for_file,
    trace_symbol_graph_from_index,
};
mod dependencies;
mod validation;
