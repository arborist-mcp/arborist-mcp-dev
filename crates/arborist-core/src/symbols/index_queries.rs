mod index;
mod list;
mod read;
mod search;
mod trace;

use std::path::Path;

use anyhow::Result;

use crate::language::normalize_absolute_path;
use crate::model::SymbolMeta;
use crate::symbol_index_state::{load_symbol_index, load_symbol_index_with_timeout};

pub(super) fn load_normalized_symbol_index(db_path: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    let db_path = normalize_absolute_path(db_path)?;
    load_symbol_index(&db_path)
}

pub(super) fn load_normalized_symbol_index_with_timeout(
    db_path: &Path,
    timeout_ms: Option<u64>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let db_path = normalize_absolute_path(db_path)?;
    load_symbol_index_with_timeout(&db_path, timeout_ms)
}

pub use index::*;
pub use list::*;
pub use read::*;
pub use search::*;
pub use trace::*;
