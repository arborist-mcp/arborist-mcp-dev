pub(super) use std::fs;

pub(super) use rusqlite::Connection;

pub(super) use super::support::{
    create_incomplete_symbol_index_tables,
    create_legacy_symbol_index_schema_without_reference_names, create_minimal_symbol_index_schema,
    create_symbol_index_schema_with_text_byte_columns, downgrade_symbol_index_schema_to_v2,
    downgrade_symbol_index_schema_to_v3, symbol_table_column_type, symbol_table_columns,
    temporary_dir,
};
pub(super) use crate::language::normalize_path;
pub(super) use crate::{
    MAX_WORKSPACE_SCAN_TIMEOUT_MS, TraceDirection, WorkspaceScanLimits, inspect_symbol_index,
    inspect_symbol_index_with_timeout, migrate_symbol_index, read_symbol_from_index,
    rebuild_symbol_index, rebuild_symbol_index_with_limits, refresh_symbol_index_for_file,
    refresh_symbol_index_for_file_with_limits, search_symbols_from_index,
    trace_symbol_graph_from_index,
};

mod inspect;
mod migration;
mod rebuild_refresh;
mod trace;
