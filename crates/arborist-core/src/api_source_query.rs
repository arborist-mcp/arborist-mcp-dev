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

    use super::workspace::{
        list_symbols_with_source_filtered_with_timeout, read_symbol_with_source_and_timeout,
        search_symbols_with_source_filtered_with_timeout,
        trace_symbol_graph_with_source_and_timeout,
    };
    use crate::model::TraceDirection;
    use crate::symbol_trace::MAX_TRACE_TIMEOUT_MS;

    fn assert_zero_timeout(error: anyhow::Error) {
        assert!(
            error
                .to_string()
                .contains("invalid trace timeout_ms: value must be greater than zero")
        );
    }

    #[test]
    fn source_query_rejects_excessive_timeout_before_overlay_setup() {
        let error = list_symbols_with_source_filtered_with_timeout(
            Path::new("missing-workspace"),
            Path::new("../outside.py"),
            "not valid source",
            10,
            None,
            None,
            Some(MAX_TRACE_TIMEOUT_MS + 1),
        )
        .expect_err("excessive timeout should be rejected before context setup");

        assert!(
            error
                .to_string()
                .contains("invalid trace timeout_ms: value must not exceed")
        );
    }

    #[test]
    fn source_query_families_reject_zero_timeout_before_overlay_setup() {
        let workspace = Path::new("missing-workspace");
        let path = Path::new("../outside.py");
        let source = "not valid source";

        assert_zero_timeout(
            list_symbols_with_source_filtered_with_timeout(
                workspace,
                path,
                source,
                10,
                None,
                None,
                Some(0),
            )
            .expect_err("list should reject zero timeout before context setup"),
        );
        assert_zero_timeout(
            search_symbols_with_source_filtered_with_timeout(
                workspace,
                path,
                source,
                "helper",
                10,
                None,
                None,
                Some(0),
            )
            .expect_err("search should reject zero timeout before context setup"),
        );
        assert_zero_timeout(
            read_symbol_with_source_and_timeout(workspace, path, source, "helper", Some(0))
                .expect_err("read should reject zero timeout before context setup"),
        );
        assert_zero_timeout(
            trace_symbol_graph_with_source_and_timeout(
                workspace,
                path,
                source,
                "helper",
                TraceDirection::Both,
                Some(0),
            )
            .expect_err("trace should reject zero timeout before context setup"),
        );
    }
}
