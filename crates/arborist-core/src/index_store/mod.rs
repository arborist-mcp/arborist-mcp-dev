pub(crate) use core::*;
pub(crate) use loading::{
    load_indexed_symbols_grouped_by_file, load_resolved_symbols, validate_legacy_indexed_symbols,
};
pub(crate) use metadata::{count_table_rows, load_file_states};
pub(crate) use validation::validate_resolved_symbol_edges;

mod core;
mod loading;
mod metadata;
mod validation;
