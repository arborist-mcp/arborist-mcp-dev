mod list;
mod read;
mod search;
mod trace;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{ensure_path_inside_workspace, normalize_absolute_path};
use crate::model::SymbolMeta;
use crate::symbol_index_state::load_symbol_index_with_overrides;
use crate::symbol_index_workspace::resolve_workspace_symbols_with_overrides;

pub(super) fn load_normalized_symbol_index_with_overrides(
    db_path: &Path,
    file_overrides: &BTreeMap<String, String>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let db_path = normalize_absolute_path(db_path)?;
    load_symbol_index_with_overrides(&db_path, file_overrides)
}

pub(super) fn load_workspace_symbols_with_overrides_at_path(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    file_path: &Path,
) -> Result<(PathBuf, Vec<SymbolMeta>, usize)> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(&workspace_root, file_overrides)?;
    Ok((file_path, resolved_symbols, indexed_files))
}

pub use list::*;
pub use read::*;
pub use search::*;
pub use trace::*;
