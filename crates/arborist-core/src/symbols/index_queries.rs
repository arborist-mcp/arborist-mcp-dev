mod index;
mod list;
mod read;
mod search;
mod trace;

use std::path::Path;

use anyhow::Result;

use crate::language::normalize_absolute_path;
use crate::model::SymbolMeta;
use crate::symbol_index_state::load_symbol_index;

pub(super) fn load_normalized_symbol_index(db_path: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    let db_path = normalize_absolute_path(db_path)?;
    load_symbol_index(&db_path)
}

pub use index::*;
pub use list::*;
pub use read::*;
pub use search::*;
pub use trace::*;
