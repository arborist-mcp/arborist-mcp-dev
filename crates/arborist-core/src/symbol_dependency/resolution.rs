mod cpp_callables;
mod graph;
mod indexes;
mod path_groups;
mod python;
mod ranking;
mod references;
mod symbol_ids;
mod template_paths;
mod type_alias;

use anyhow::Result;

use crate::symbol_index_model::IndexedSymbol;
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) use graph::{
    resolve_symbol_dependencies, resolve_symbol_dependencies_with_overrides,
    resolve_symbol_dependencies_with_overrides_with_deadline,
};
pub(super) use indexes::{build_name_index, build_semantic_path_index, raw_symbol_indexes_by_id};
pub(super) use ranking::indexed_symbol_rank;
pub(super) use references::{
    resolve_dependencies_for_symbol, resolve_dependencies_for_symbol_with_deadline,
};
pub(super) use template_paths::cpp_template_base_path;

pub(crate) fn assign_symbol_ids(raw_symbols: &mut [IndexedSymbol]) -> Result<()> {
    symbol_ids::assign_symbol_ids(raw_symbols)
}

pub(crate) fn assign_symbol_ids_with_deadline(
    raw_symbols: &mut [IndexedSymbol],
    deadline: &WorkspaceScanDeadline,
) -> Result<()> {
    symbol_ids::assign_symbol_ids_with_deadline(raw_symbols, Some(deadline))
}
