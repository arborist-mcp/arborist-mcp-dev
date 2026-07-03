use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::language::{
    language_for_id, normalize_path, parse_document, position_from, read_source,
};
use crate::model::QueryCaptureResult;

pub fn execute_tree_query_from_path(path: &Path, query: &str) -> Result<Vec<QueryCaptureResult>> {
    let source = read_source(path)?;
    execute_tree_query(path, &source, query)
}

pub fn execute_tree_query(
    path: &Path,
    source: &str,
    query: &str,
) -> Result<Vec<QueryCaptureResult>> {
    let document = parse_document(path, source)?;
    let language = language_for_id(document.language_id);
    let compiled = Query::new(&language, query)
        .with_context(|| format!("invalid Tree-sitter query for {}", normalize_path(path)))?;

    let mut cursor = QueryCursor::new();
    let mut captures = Vec::new();

    let mut query_captures =
        cursor.captures(&compiled, document.tree.root_node(), source.as_bytes());
    while let Some((query_match, capture_index)) = query_captures.next() {
        let capture = query_match.captures[*capture_index];
        let node = capture.node;
        captures.push(QueryCaptureResult {
            capture_name: compiled.capture_names()[capture.index as usize].to_string(),
            node_kind: node.kind().to_string(),
            text: node.utf8_text(source.as_bytes())?.to_string(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_point: position_from(node.start_position()),
            end_point: position_from(node.end_position()),
        });
    }

    Ok(captures)
}
