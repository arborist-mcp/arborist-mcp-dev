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

fn with_source_query_context_with_trace_deadline<T>(
    root: SourceQueryRoot<'_>,
    path: &Path,
    source: &str,
    timeout_ms: Option<u64>,
    query: impl FnOnce(&SymbolQueryContext, &TraceQueryDeadline) -> Result<T>,
) -> Result<T> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    with_source_query_context_with_deadline(root, path, source, &deadline, query)
}

fn with_source_query_context_with_deadline<T>(
    root: SourceQueryRoot<'_>,
    path: &Path,
    source: &str,
    deadline: &TraceQueryDeadline,
    query: impl FnOnce(&SymbolQueryContext, &TraceQueryDeadline) -> Result<T>,
) -> Result<T> {
    deadline.check("source query context")?;
    let context = match root {
        SourceQueryRoot::Workspace(workspace_root) => SymbolQueryContext::workspace(workspace_root),
        SourceQueryRoot::Index(db_path) => SymbolQueryContext::index(db_path),
    }?;
    deadline.check("source query context")?;
    let context = context.with_source_overlay(path, source)?;
    deadline.check("source query overlay")?;
    let result = query(&context, deadline)?;
    deadline.check("source query result")?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::workspace::{
        list_symbols_with_source_filtered_with_timeout, read_symbol_with_source_and_timeout,
        search_symbols_with_source_filtered_with_timeout,
        trace_symbol_graph_with_source_and_timeout,
    };
    use super::{SourceQueryRoot, with_source_query_context_with_trace_deadline};
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
    fn source_trace_query_preserves_deadline_after_overlay_setup() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let workspace =
            std::env::temp_dir().join(format!("arborist-source-trace-deadline-{unique}"));
        let path = workspace.join("module.py");
        fs::create_dir_all(&workspace).expect("temporary workspace should be created");
        fs::write(&path, "def helper():\n    return 1\n")
            .expect("temporary source file should be written");

        let error = with_source_query_context_with_trace_deadline(
            SourceQueryRoot::Workspace(&workspace),
            &path,
            "def helper():\n    return 2\n",
            Some(100),
            |context, deadline| {
                thread::sleep(Duration::from_millis(150));
                context.trace_symbol_graph_with_deadline("helper", TraceDirection::Both, deadline)
            },
        )
        .expect_err("source trace should retain the deadline created before overlay setup");

        fs::remove_dir_all(&workspace).expect("temporary workspace should be removed");
        assert!(error.to_string().contains("override symbol loading"));
        assert!(error.to_string().contains("trace timeout exceeded"));
    }

    #[test]
    fn source_list_query_preserves_deadline_after_overlay_setup() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let workspace =
            std::env::temp_dir().join(format!("arborist-source-list-deadline-{unique}"));
        let path = workspace.join("module.py");
        fs::create_dir_all(&workspace).expect("temporary workspace should be created");
        fs::write(&path, "def helper():\n    return 1\n")
            .expect("temporary source file should be written");

        let error = with_source_query_context_with_trace_deadline(
            SourceQueryRoot::Workspace(&workspace),
            &path,
            "def helper():\n    return 2\n",
            Some(100),
            |context, deadline| {
                thread::sleep(Duration::from_millis(150));
                context.list_symbols_with_deadline(10, None, None, deadline)
            },
        )
        .expect_err("source list query should retain the deadline created before overlay setup");

        fs::remove_dir_all(&workspace).expect("temporary workspace should be removed");
        assert!(error.to_string().contains("workspace symbol loading"));
        assert!(error.to_string().contains("trace timeout exceeded"));
    }

    #[test]
    fn source_search_query_preserves_deadline_after_overlay_setup() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let workspace =
            std::env::temp_dir().join(format!("arborist-source-search-deadline-{unique}"));
        let path = workspace.join("module.py");
        fs::create_dir_all(&workspace).expect("temporary workspace should be created");
        fs::write(&path, "def helper():\n    return 1\n")
            .expect("temporary source file should be written");

        let error = with_source_query_context_with_trace_deadline(
            SourceQueryRoot::Workspace(&workspace),
            &path,
            "def helper():\n    return 2\n",
            Some(100),
            |context, deadline| {
                thread::sleep(Duration::from_millis(150));
                context.search_symbols_with_deadline("helper", 10, None, None, deadline)
            },
        )
        .expect_err("source search query should retain the deadline created before overlay setup");

        fs::remove_dir_all(&workspace).expect("temporary workspace should be removed");
        assert!(error.to_string().contains("workspace symbol loading"));
        assert!(error.to_string().contains("trace timeout exceeded"));
    }

    #[test]
    fn source_read_query_preserves_deadline_after_overlay_setup() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let workspace =
            std::env::temp_dir().join(format!("arborist-source-read-deadline-{unique}"));
        let path = workspace.join("module.py");
        fs::create_dir_all(&workspace).expect("temporary workspace should be created");
        fs::write(&path, "def helper():\n    return 1\n")
            .expect("temporary source file should be written");

        let error = with_source_query_context_with_trace_deadline(
            SourceQueryRoot::Workspace(&workspace),
            &path,
            "def helper():\n    return 2\n",
            Some(100),
            |context, deadline| {
                thread::sleep(Duration::from_millis(150));
                context.read_symbol_with_deadline("helper", deadline)
            },
        )
        .expect_err("source read query should retain the deadline created before overlay setup");

        fs::remove_dir_all(&workspace).expect("temporary workspace should be removed");
        assert!(error.to_string().contains("workspace symbol loading"));
        assert!(error.to_string().contains("trace timeout exceeded"));
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
