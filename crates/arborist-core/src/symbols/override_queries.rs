mod list;
mod read;
mod search;
mod trace;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::language::normalize_absolute_path;
use crate::model::SymbolMeta;
use crate::symbol_index_state::load_symbol_index_with_overrides;

pub(super) fn load_normalized_symbol_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let db_path = normalize_absolute_path(db_path)?;
    load_symbol_index_with_overrides(&db_path, file_overrides)
}

pub use list::*;
pub use read::*;
pub use search::*;
pub use trace::*;
