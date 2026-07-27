use std::path::Path;

use anyhow::Result;

use crate::symbol_query::SymbolQueryContext;
use crate::symbol_trace::TraceQueryDeadline;

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

fn with_source_query_context_with_timeout<T>(
    root: SourceQueryRoot<'_>,
    path: &Path,
    source: &str,
    timeout_ms: Option<u64>,
    query: impl FnOnce(&SymbolQueryContext, Option<u64>) -> Result<T>,
) -> Result<T> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("source query context")?;
    let context = match root {
        SourceQueryRoot::Workspace(workspace_root) => SymbolQueryContext::workspace(workspace_root),
        SourceQueryRoot::Index(db_path) => SymbolQueryContext::index(db_path),
    }?;
    deadline.check("source query context")?;
    let context = context.with_source_overlay(path, source)?;
    deadline.check("source query overlay")?;
    let timeout_ms = deadline.remaining_timeout_ms("source query execution")?;
    let result = query(&context, timeout_ms)?;
    deadline.check("source query result")?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::workspace::list_symbols_with_source_filtered_with_timeout;

    #[test]
    fn source_query_rejects_zero_timeout_before_overlay_setup() {
        let error = list_symbols_with_source_filtered_with_timeout(
            Path::new("missing-workspace"),
            Path::new("../outside.py"),
            "not valid source",
            10,
            None,
            None,
            Some(0),
        )
        .expect_err("zero timeout should be rejected before context setup");
        assert!(
            error
                .to_string()
                .contains("invalid trace timeout_ms: value must be greater than zero")
        );
    }
}
