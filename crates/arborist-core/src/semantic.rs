use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Result, anyhow};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

use crate::language::{contains_node, language_for_id, node_text, normalize_path};
use crate::model::{LanguageId, SemanticSkeleton, SemanticSkeletonSymbol};

mod c;

pub(crate) use c::c_is_callable_declaration;
pub(crate) use c::c_named_node_name;
pub(crate) use c::c_symbol_nodes;
pub(crate) use c::has_c_internal_linkage;
pub use c::{c_function_header, c_semantic_path, c_symbol_id_for_node};
pub(crate) use c::{c_parameters, c_return_type};

pub fn get_semantic_skeleton(
    path: &Path,
    language_id: LanguageId,
    source: &str,
    tree: &Tree,
    depth_limit: usize,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    match language_id {
        LanguageId::Python => build_python_skeleton(path, source, tree, depth_limit, expand_nodes),
        LanguageId::C | LanguageId::Cpp => c::build_c_skeleton(path, source, tree, expand_nodes),
    }
}

pub fn semantic_path(node: Node<'_>, source: &str) -> Result<String> {
    let mut segments = Vec::new();
    let mut current = Some(node);

    while let Some(candidate) = current {
        if matches!(candidate.kind(), "class_definition" | "function_definition")
            && let Some(name_node) = candidate.child_by_field_name("name")
        {
            segments.push(node_text(name_node, source)?.trim().to_string());
        }
        current = candidate.parent();
    }

    segments.reverse();
    Ok(segments.join("."))
}

pub fn semantic_depth(node: Node<'_>) -> usize {
    let mut depth = 0;
    let mut current = Some(node);

    while let Some(candidate) = current {
        if matches!(candidate.kind(), "class_definition" | "function_definition") {
            depth += 1;
        }
        current = candidate.parent();
    }

    depth
}

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

pub(crate) fn semantic_parent_path(path: &str) -> Option<String> {
    if is_file_scoped_c_semantic_path(path) {
        return None;
    }

    path.rsplit_once("::")
        .or_else(|| path.rsplit_once('.'))
        .map(|(parent, _)| parent.to_string())
        .filter(|parent| !parent.is_empty())
}

fn is_file_scoped_c_semantic_path(path: &str) -> bool {
    if path.contains('/') || path.contains('\\') {
        return true;
    }

    path.rsplit_once("::")
        .and_then(|(scope, _)| scope.rsplit_once('.').map(|(_, extension)| extension))
        .is_some_and(|extension| {
            [
                "c", "h", "cc", "cpp", "cxx", "c++", "hpp", "hh", "hxx", "h++",
            ]
            .iter()
            .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
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

pub fn find_semantic_node<'tree>(
    language_id: LanguageId,
    path: &Path,
    tree: &'tree Tree,
    source: &str,
    target_path: &str,
) -> Result<Option<Node<'tree>>> {
    match language_id {
        LanguageId::Python => find_python_semantic_node(tree, source, target_path),
        LanguageId::C | LanguageId::Cpp => c::find_c_semantic_node(path, tree, source, target_path),
    }
}

pub fn ascend_to_symbol(language_id: LanguageId, node: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(node);

    while let Some(candidate) = current {
        if matches!(language_id, LanguageId::Python) && candidate.kind() == "decorated_definition" {
            let mut cursor = candidate.walk();
            for child in candidate.named_children(&mut cursor) {
                if matches!(child.kind(), "class_definition" | "function_definition") {
                    return Some(child);
                }
            }
        }

        let is_symbol = match language_id {
            LanguageId::Python => {
                matches!(candidate.kind(), "class_definition" | "function_definition")
            }
            LanguageId::C | LanguageId::Cpp => {
                matches!(
                    candidate.kind(),
                    "alias_declaration"
                        | "class_specifier"
                        | "concept_definition"
                        | "enum_specifier"
                        | "struct_specifier"
                        | "type_definition"
                        | "union_specifier"
                ) || candidate.kind() == "function_definition"
                    || c::c_is_callable_declaration(candidate)
            }
        };

        if is_symbol {
            return Some(candidate);
        }
        current = candidate.parent();
    }

    None
}

fn build_python_skeleton(
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

fn find_python_semantic_node<'tree>(
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
