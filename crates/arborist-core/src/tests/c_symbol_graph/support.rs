use std::path::Path;

use anyhow::Result;

use crate::language::normalize_absolute_path;
use crate::model::{SymbolMeta, TraceDirection, TraceSymbolGraphResult};
use crate::symbol_index_state::load_symbol_index;
use crate::symbol_index_workspace::load_live_workspace_symbols;
use crate::symbol_query_execution::trace_from_symbols_with_timeout;

pub(super) struct PreparedSymbolGraph {
    symbols: Vec<SymbolMeta>,
    indexed_files: usize,
}

impl PreparedSymbolGraph {
    pub(super) fn from_workspace(workspace_root: &Path) -> Result<Self> {
        let (symbols, indexed_files) = load_live_workspace_symbols(workspace_root)?;
        Ok(Self {
            symbols,
            indexed_files,
        })
    }

    pub(super) fn from_index(db_path: &Path) -> Result<Self> {
        let db_path = normalize_absolute_path(db_path)?;
        let (symbols, indexed_files) = load_symbol_index(&db_path)?;
        Ok(Self {
            symbols,
            indexed_files,
        })
    }

    pub(super) fn trace(&self, symbol_path: &str) -> Result<TraceSymbolGraphResult> {
        trace_from_symbols_with_timeout(
            &self.symbols,
            self.indexed_files,
            symbol_path,
            TraceDirection::Both,
            None,
        )
    }
}
