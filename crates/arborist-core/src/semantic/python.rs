use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Result, anyhow};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

use crate::language::{contains_node, language_for_id, node_text, normalize_path};
use crate::model::{LanguageId, SemanticSkeleton, SemanticSkeletonSymbol};

use super::{semantic_depth, semantic_parent_path, semantic_path};

fn python_display_node(node: Node<'_>) -> Node<'_> {
    node.parent()
        .filter(|parent| parent.kind() == "decorated_definition")
        .unwrap_or(node)
}

pub(crate) fn python_display_byte_range(node: Node<'_>) -> (usize, usize) {
    let display_node = python_display_node(node);
    (display_node.start_byte(), display_node.end_byte())
}

pub(crate) fn python_display_header(node: Node<'_>, source: &str) -> Result<String> {
    let body = node
        .child_by_field_name("body")
        .ok_or_else(|| anyhow!("python symbol missing body"))?;
    let display_node = python_display_node(node);
    Ok(source[display_node.start_byte()..body.start_byte()]
        .trim_end()
        .to_string())
}

pub(crate) fn python_docstring(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(body) = node.child_by_field_name("body") else {
        return Ok(None);
    };
    let Some(first_statement) = body.named_child(0) else {
        return Ok(None);
    };
    if first_statement.kind() != "expression_statement" {
        return Ok(None);
    }

    let Some(first_expr) = first_statement.named_child(0) else {
        return Ok(None);
    };
    if !matches!(first_expr.kind(), "string" | "concatenated_string") {
        return Ok(None);
    }

    Ok(Some(node_text(first_expr, source)?.trim().to_string()))
}

pub(crate) fn python_parameters(node: Node<'_>, source: &str) -> Result<Vec<String>> {
    let Some(parameters) = node.child_by_field_name("parameters") else {
        return Ok(Vec::new());
    };

    let mut cursor = parameters.walk();
    let mut values = Vec::new();
    for child in parameters.named_children(&mut cursor) {
        values.push(node_text(child, source)?.trim().to_string());
    }
    Ok(values)
}

pub(crate) fn python_return_type(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(return_type) = node.child_by_field_name("return_type") else {
        return Ok(None);
    };

    Ok(Some(node_text(return_type, source)?.trim().to_string()))
}

pub(super) fn build_python_skeleton(
    path: &Path,
    source: &str,
    tree: &Tree,
    depth_limit: usize,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    let language = language_for_id(LanguageId::Python);
    let query = Query::new(
        &language,
        r#"
        (class_definition
            name: (identifier) @name
            body: (block) @body) @item

        (function_definition
            name: (identifier) @name
            body: (block) @body) @item
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let mut symbol_items = Vec::new();
    let mut available_paths = Vec::new();
    let mut available_symbols = Vec::new();
    let expand_set: BTreeSet<_> = expand_nodes.iter().map(String::as_str).collect();

    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    while let Some(query_match) = matches.next() {
        let mut item_node = None;

        for capture in query_match.captures.iter() {
            let capture_name = &query.capture_names()[capture.index as usize];
            if *capture_name == "item" {
                item_node = Some(capture.node);
            }
        }

        let Some(item) = item_node else {
            continue;
        };

        let path = semantic_path(item, source)?;
        let symbol_id = path.clone();
        if semantic_depth(item) > depth_limit
            && !expand_set.contains(path.as_str())
            && !expand_set.contains(symbol_id.as_str())
        {
            continue;
        }
        let scope_path = semantic_parent_path(&path);
        let signature = Some(python_display_header(item, source)?);
        let parameters = python_parameters(item, source)?;
        let return_type = python_return_type(item, source)?;
        let docstring = python_docstring(item, source)?;
        let byte_range = python_display_byte_range(item);
        available_paths.push(path.clone());
        available_symbols.push(SemanticSkeletonSymbol {
            symbol_id: symbol_id.clone(),
            semantic_path: path.clone(),
            scope_path,
            node_kind: item.kind().to_string(),
            byte_range,
            signature: signature.clone(),
            parameters,
            return_type,
            docstring,
        });
        symbol_items.push((item, path, symbol_id));
    }

    let mut skeleton_items = Vec::new();
    let mut expanded_items = Vec::new();
    for (item, path, symbol_id) in symbol_items {
        if expanded_items
            .iter()
            .any(|ancestor: &Node<'_>| contains_node(*ancestor, item))
        {
            continue;
        }

        let display_item = python_display_node(item);
        if expand_set.contains(path.as_str()) || expand_set.contains(symbol_id.as_str()) {
            skeleton_items.push(node_text(display_item, source)?.trim().to_string());
            expanded_items.push(item);
        } else {
            let header = python_display_header(item, source)?;
            skeleton_items.push(format!("{header} ..."));
        }
    }

    let result = SemanticSkeleton {
        file: normalize_path(path),
        skeleton: skeleton_items.join("\n\n"),
        available_paths,
        available_symbols,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub(super) fn find_python_semantic_node<'tree>(
    tree: &'tree Tree,
    source: &str,
    target_path: &str,
) -> Result<Option<Node<'tree>>> {
    search_python_symbol(tree.root_node(), source, target_path)
}

fn search_python_symbol<'tree>(
    node: Node<'tree>,
    source: &str,
    target_path: &str,
) -> Result<Option<Node<'tree>>> {
    if matches!(node.kind(), "class_definition" | "function_definition")
        && semantic_path(node, source)? == target_path
    {
        return Ok(Some(node));
    }

    let child_count = node.child_count();
    for index in 0..child_count {
        if let Some(child) = node.child(index)
            && let Some(found) = search_python_symbol(child, source, target_path)?
        {
            return Ok(Some(found));
        }
    }

    Ok(None)
}
