use std::path::Path;

use anyhow::Result;

use crate::symbol_query::SymbolQueryContext;

mod index;
mod workspace;

pub use index::*;
pub use workspace::*;

#[derive(Debug, Clone, Copy)]
enum SourceQueryRoot<'a> {
    Workspace(&'a Path),
    Index(&'a Path),
}

fn with_source_query_context<T>(
    root: SourceQueryRoot<'_>,
    path: &Path,
    source: &str,
    query: impl FnOnce(&SymbolQueryContext) -> Result<T>,
) -> Result<T> {
    let context = match root {
        SourceQueryRoot::Workspace(workspace_root) => SymbolQueryContext::workspace(workspace_root),
        SourceQueryRoot::Index(db_path) => SymbolQueryContext::index(db_path),
    }?
    .with_source_overlay(path, source)?;
    query(&context)
}
