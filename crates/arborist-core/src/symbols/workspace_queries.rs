mod list;
mod read;
mod search;
mod trace;

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{ensure_path_inside_workspace, normalize_absolute_path};
use crate::model::SymbolMeta;
use crate::symbol_index_workspace::resolve_workspace_symbols;

pub(super) fn load_live_workspace_symbols_at_path(
    workspace_root: &Path,
    file_path: &Path,
) -> Result<(PathBuf, Vec<SymbolMeta>, usize)> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let file_path = normalize_absolute_path(file_path)?;
    ensure_path_inside_workspace(&workspace_root, &file_path)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
    Ok((file_path, resolved_symbols, indexed_files))
}

pub use list::*;
pub use read::*;
pub use search::*;
pub use trace::*;
