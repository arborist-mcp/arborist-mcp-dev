use std::path::Path;

use anyhow::{Result, bail};

use crate::language::{normalize_absolute_path, read_source, validate_source_length};
use crate::model::QueryCaptureResult;

mod execution;
pub(crate) mod owners;
mod validation;

pub const DEFAULT_TREE_QUERY_MAX_CAPTURES: usize = 10_000;
pub const MAX_TREE_QUERY_CAPTURES: usize = 100_000;
pub const DEFAULT_TREE_QUERY_MAX_BYTES: usize = 64 * 1024;
pub const DEFAULT_TREE_QUERY_TIMEOUT_MICROS: u64 = 500_000;
pub const MAX_TREE_QUERY_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
pub const DEFAULT_TREE_QUERY_MATCH_LIMIT: u32 = 32_768;

pub fn execute_tree_query_from_path(path: &Path, query: &str) -> Result<Vec<QueryCaptureResult>> {
    execute_tree_query_from_path_with_limit(path, query, DEFAULT_TREE_QUERY_MAX_CAPTURES)
}

pub fn execute_tree_query_from_path_with_limit(
    path: &Path,
    query: &str,
    max_captures: usize,
) -> Result<Vec<QueryCaptureResult>> {
    execute_tree_query_from_path_with_timeout(path, query, max_captures, None)
}

pub fn execute_tree_query_from_path_with_timeout(
    path: &Path,
    query: &str,
    max_captures: usize,
    timeout_ms: Option<u64>,
) -> Result<Vec<QueryCaptureResult>> {
    let path = normalize_absolute_path(path)?;
    let source = read_source(&path)?;
    validate_source_length(&path, source.len())?;
    if let Some(timeout_ms) = timeout_ms {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        if std::time::Instant::now() >= deadline {
            bail!("invalid Tree-sitter query timeout_ms: value must be greater than zero");
        }
    }
    execute_tree_query_with_timeout(&path, &source, query, max_captures, timeout_ms)
}

pub fn execute_tree_query(
    path: &Path,
    source: &str,
    query: &str,
) -> Result<Vec<QueryCaptureResult>> {
    execute_tree_query_with_limit(path, source, query, DEFAULT_TREE_QUERY_MAX_CAPTURES)
}

pub fn execute_tree_query_with_limit(
    path: &Path,
    source: &str,
    query: &str,
    max_captures: usize,
) -> Result<Vec<QueryCaptureResult>> {
    execute_tree_query_with_timeout(path, source, query, max_captures, None)
}

pub fn execute_tree_query_with_timeout(
    path: &Path,
    source: &str,
    query: &str,
    max_captures: usize,
    timeout_ms: Option<u64>,
) -> Result<Vec<QueryCaptureResult>> {
    execution::execute_tree_query_with_timeout(path, source, query, max_captures, timeout_ms)
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_TREE_QUERY_CAPTURES, MAX_TREE_QUERY_TIMEOUT_MS,
        validation::{validate_max_captures, validate_timeout},
    };

    #[test]
    fn validates_tree_query_timeout_bounds() {
        assert!(validate_timeout(Some(0)).is_err());
        assert!(validate_timeout(Some(MAX_TREE_QUERY_TIMEOUT_MS + 1)).is_err());
        assert_eq!(validate_timeout(None).unwrap(), 500_000);
        assert_eq!(validate_timeout(Some(2)).unwrap(), 2_000);
    }

    #[test]
    fn validates_tree_query_capture_bounds() {
        assert!(validate_max_captures(0).is_err());
        assert!(validate_max_captures(MAX_TREE_QUERY_CAPTURES + 1).is_err());
        assert!(validate_max_captures(MAX_TREE_QUERY_CAPTURES).is_ok());
    }
}
