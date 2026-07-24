use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use tree_sitter::{Query, QueryCursor, QueryCursorOptions, StreamingIterator};

use crate::language::{
    language_for_id, normalize_absolute_path, normalize_path, parse_document, position_from,
    read_source,
};
use crate::model::LanguageId;
use crate::model::QueryCaptureResult;
use crate::semantic::c_symbol_nodes;

mod owners;

pub const DEFAULT_TREE_QUERY_MAX_CAPTURES: usize = 10_000;
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
    let path = normalize_absolute_path(path)?;
    validate_tree_query(query)?;
    validate_max_captures(max_captures)?;
    let timeout_micros = validate_timeout(timeout_ms)?;
    let document = parse_document(&path, source)?;
    let language = language_for_id(document.language_id);
    let root = document.tree.root_node();
    let c_symbols = match document.language_id {
        LanguageId::C | LanguageId::Cpp => Some(c_symbol_nodes(&path, root, source)?),
        LanguageId::Python => None,
    };
    let compiled = Query::new(&language, query)
        .with_context(|| format!("invalid Tree-sitter query for {}", normalize_path(&path)))?;

    let mut cursor = QueryCursor::new();
    cursor.set_match_limit(DEFAULT_TREE_QUERY_MATCH_LIMIT);
    let mut captures = Vec::new();
    let deadline = Instant::now() + Duration::from_micros(timeout_micros);
    let mut timed_out = false;
    let mut progress_callback = |_: &tree_sitter::QueryCursorState| -> bool {
        if Instant::now() >= deadline {
            timed_out = true;
            return false;
        }
        true
    };
    let options = QueryCursorOptions::new().progress_callback(&mut progress_callback);

    let mut query_captures =
        cursor.captures_with_options(&compiled, root, source.as_bytes(), options);
    while let Some((query_match, capture_index)) = query_captures.next() {
        if Instant::now() >= deadline {
            timed_out = true;
            break;
        }
        if captures.len() >= max_captures {
            bail!(
                "Tree-sitter query capture limit exceeded for {}: max_captures={}",
                normalize_path(&path),
                max_captures
            );
        }
        let capture = query_match.captures[*capture_index];
        let node = capture.node;
        let (owner_symbol_id, owner_semantic_path, owner_scope_path) = owners::capture_owner(
            &path,
            source,
            document.language_id,
            node,
            c_symbols.as_deref(),
        )?;
        if Instant::now() >= deadline {
            timed_out = true;
            break;
        }
        captures.push(QueryCaptureResult {
            capture_name: compiled.capture_names()[capture.index as usize].to_string(),
            node_kind: node.kind().to_string(),
            text: node.utf8_text(source.as_bytes())?.to_string(),
            owner_symbol_id,
            owner_semantic_path,
            owner_scope_path,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_point: position_from(node.start_position()),
            end_point: position_from(node.end_position()),
        });
    }
    drop(query_captures);

    if timed_out {
        bail!(
            "Tree-sitter query timed out for {} after {} microseconds",
            normalize_path(&path),
            timeout_micros
        );
    }
    if cursor.did_exceed_match_limit() {
        bail!(
            "Tree-sitter query match limit exceeded for {}: match_limit={}",
            normalize_path(&path),
            DEFAULT_TREE_QUERY_MATCH_LIMIT
        );
    }

    for (index, capture) in captures.iter().enumerate() {
        capture.validate_public_output(index)?;
    }

    Ok(captures)
}

fn validate_tree_query(query: &str) -> Result<()> {
    if query.trim().is_empty() {
        bail!("invalid Tree-sitter query: query must not be blank");
    }
    if query.len() > DEFAULT_TREE_QUERY_MAX_BYTES {
        bail!(
            "invalid Tree-sitter query: query exceeds max query bytes ({})",
            DEFAULT_TREE_QUERY_MAX_BYTES
        );
    }

    Ok(())
}

fn validate_max_captures(max_captures: usize) -> Result<()> {
    if max_captures == 0 {
        bail!("invalid Tree-sitter query max_captures: value must be greater than zero");
    }

    Ok(())
}

fn validate_timeout(timeout_ms: Option<u64>) -> Result<u64> {
    let timeout_ms = timeout_ms.unwrap_or(DEFAULT_TREE_QUERY_TIMEOUT_MICROS / 1_000);
    if timeout_ms == 0 {
        bail!("invalid Tree-sitter query timeout_ms: value must be greater than zero");
    }
    if timeout_ms > MAX_TREE_QUERY_TIMEOUT_MS {
        bail!(
            "invalid Tree-sitter query timeout_ms: value must not exceed {}",
            MAX_TREE_QUERY_TIMEOUT_MS
        );
    }
    Ok(timeout_ms.saturating_mul(1_000))
}

#[cfg(test)]
mod tests {
    use super::{MAX_TREE_QUERY_TIMEOUT_MS, validate_timeout};

    #[test]
    fn validates_tree_query_timeout_bounds() {
        assert!(validate_timeout(Some(0)).is_err());
        assert!(validate_timeout(Some(MAX_TREE_QUERY_TIMEOUT_MS + 1)).is_err());
        assert_eq!(validate_timeout(None).unwrap(), 500_000);
        assert_eq!(validate_timeout(Some(2)).unwrap(), 2_000);
    }
}
