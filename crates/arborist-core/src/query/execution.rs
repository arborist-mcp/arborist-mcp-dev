use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use tree_sitter::{Query, QueryCursor, QueryCursorOptions, StreamingIterator};

use crate::language::{
    LanguageCapabilities, builtin_language_registry, language_for_id, normalize_absolute_path,
    normalize_path, parse_document_with_timeout, position_from,
};
use crate::model::QueryCaptureResult;

use super::{DEFAULT_TREE_QUERY_MATCH_LIMIT, validation};

fn ensure_within_deadline(path: &Path, timeout_micros: u64, deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        bail!(
            "Tree-sitter query timed out for {} after {} microseconds",
            normalize_path(path),
            timeout_micros
        );
    }
    Ok(())
}

pub(super) fn execute_tree_query_with_timeout(
    path: &Path,
    source: &str,
    query: &str,
    max_captures: usize,
    timeout_ms: Option<u64>,
) -> Result<Vec<QueryCaptureResult>> {
    validation::validate_tree_query(query)?;
    validation::validate_max_captures(max_captures)?;
    let timeout_micros = validation::validate_timeout(timeout_ms)?;
    let deadline = Instant::now() + Duration::from_micros(timeout_micros);
    let path = normalize_absolute_path(path)?;
    ensure_within_deadline(&path, timeout_micros, deadline)?;
    let remaining_parse_micros = deadline
        .saturating_duration_since(Instant::now())
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;
    if remaining_parse_micros == 0 {
        bail!(
            "Tree-sitter query timed out for {} after {} microseconds",
            normalize_path(&path),
            timeout_micros
        );
    }
    let document = parse_document_with_timeout(&path, source, remaining_parse_micros)?;
    ensure_within_deadline(&path, timeout_micros, deadline)?;
    let registry = builtin_language_registry();
    registry.require_capability(
        document.language_id,
        LanguageCapabilities::TREE_QUERY,
        "Tree-sitter query execution",
    )?;
    let language = language_for_id(document.language_id);
    let root = document.tree.root_node();
    let adapter = registry
        .adapter(document.language_id)
        .expect("every LanguageId must have a builtin language adapter");
    let owner_candidates = adapter.query_owner_candidates(&path, root, source)?;
    ensure_within_deadline(&path, timeout_micros, deadline)?;
    let compiled = Query::new(&language, query)
        .with_context(|| format!("invalid Tree-sitter query for {}", normalize_path(&path)))?;

    let mut cursor = QueryCursor::new();
    cursor.set_match_limit(DEFAULT_TREE_QUERY_MATCH_LIMIT);
    let mut captures = Vec::new();
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
        let (owner_symbol_id, owner_semantic_path, owner_scope_path) =
            adapter.query_capture_owner(&path, source, node, owner_candidates.as_deref())?;
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
        ensure_within_deadline(&path, timeout_micros, deadline)?;
        capture.validate_public_output(index)?;
    }

    Ok(captures)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{ensure_within_deadline, execute_tree_query_with_timeout};

    #[test]
    fn deadline_helper_reports_expiration() {
        let error = ensure_within_deadline(
            std::path::Path::new("sample.py"),
            1,
            Instant::now() - Duration::from_micros(1),
        )
        .expect_err("expired query deadlines should fail before execution");

        assert!(error.to_string().contains("timed out"));
        assert!(error.to_string().contains("sample.py"));
    }

    #[test]
    fn parser_timeout_interrupts_large_source_before_query_execution() {
        let source = "(".repeat(1_000_000);
        let error = execute_tree_query_with_timeout(
            std::path::Path::new("sample.py"),
            &source,
            "(identifier) @id",
            10,
            Some(1),
        )
        .expect_err("parser timeout should interrupt oversized work");

        assert!(error.to_string().contains("timed out"));
    }
}
