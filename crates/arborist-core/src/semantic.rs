use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Result, anyhow};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

use crate::language::{
    c_include_targets, contains_kind, contains_node, first_identifier, language_for_id,
    last_type_identifier, node_text, normalize_path, parse_document, read_source,
    resolve_local_c_include,
};
use crate::model::{LanguageId, SemanticSkeleton};

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
        LanguageId::C => build_c_skeleton(path, source, tree, expand_nodes),
    }
}

pub fn semantic_path(node: Node<'_>, source: &str) -> Result<String> {
    let mut segments = Vec::new();
    let mut current = Some(node);

    while let Some(candidate) = current {
        if matches!(candidate.kind(), "class_definition" | "function_definition") {
            if let Some(name_node) = candidate.child_by_field_name("name") {
                segments.push(node_text(name_node, source)?.trim().to_string());
            }
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

pub fn python_header(node: Node<'_>, source: &str) -> Result<String> {
    let body = node
        .child_by_field_name("body")
        .ok_or_else(|| anyhow!("python symbol missing body"))?;
    Ok(source[node.start_byte()..body.start_byte()]
        .trim_end()
        .to_string())
}

pub fn c_function_header(node: Node<'_>, source: &str) -> Result<String> {
    let body = node
        .child_by_field_name("body")
        .ok_or_else(|| anyhow!("function_definition missing body"))?;
    let prefix = source[node.start_byte()..body.start_byte()].trim_end();
    Ok(format!("{prefix};"))
}

pub fn c_semantic_path(path: &Path, node: Node<'_>, source: &str) -> Result<Option<String>> {
    let symbol_name = match node.kind() {
        "type_definition" => last_type_identifier(node, source)?,
        "declaration" | "function_definition" => first_identifier(node, source)?,
        _ => None,
    };

    Ok(symbol_name.map(|name| {
        if has_c_internal_linkage(node, source) {
            format!("{}::{name}", normalize_path(path))
        } else {
            name
        }
    }))
}

pub fn c_symbol_id_for_node(path: &Path, node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(semantic_path) = c_semantic_path(path, node, source)? else {
        return Ok(None);
    };

    if semantic_path.contains("::") {
        return Ok(Some(semantic_path));
    }

    let Some(base_name) = c_symbol_base_name(node, source)? else {
        return Ok(None);
    };

    if is_c_header_path(path) {
        return Ok(Some(format!("{}::{base_name}", normalize_path(path))));
    }

    let anchor = c_symbol_family_anchor_for_name(path, source, &base_name)?;
    Ok(Some(format!("{anchor}::{base_name}")))
}

pub fn has_c_internal_linkage(node: Node<'_>, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "storage_class_specifier" {
            continue;
        }
        if node_text(child, source)
            .map(|text| text.trim() == "static")
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
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
        LanguageId::C => find_c_semantic_node(path, tree, source, target_path),
    }
}

pub fn ascend_to_symbol(language_id: LanguageId, node: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(node);

    while let Some(candidate) = current {
        let is_symbol = match language_id {
            LanguageId::Python => {
                matches!(candidate.kind(), "class_definition" | "function_definition")
            }
            LanguageId::C => {
                candidate.kind() == "type_definition"
                    || candidate.kind() == "function_definition"
                    || (candidate.kind() == "declaration"
                        && contains_kind(candidate, "function_declarator"))
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

        if semantic_depth(item) > depth_limit {
            continue;
        }

        let path = semantic_path(item, source)?;
        available_paths.push(path.clone());
        symbol_items.push((item, path));
    }

    let mut skeleton_items = Vec::new();
    let mut expanded_items = Vec::new();
    for (item, path) in symbol_items {
        if expanded_items
            .iter()
            .any(|ancestor: &Node<'_>| contains_node(*ancestor, item))
        {
            continue;
        }

        if expand_set.contains(path.as_str()) {
            skeleton_items.push(node_text(item, source)?.trim().to_string());
            expanded_items.push(item);
        } else {
            let header = python_header(item, source)?;
            skeleton_items.push(format!("{header} ..."));
        }
    }

    Ok(SemanticSkeleton {
        file: normalize_path(path),
        skeleton: skeleton_items.join("\n\n"),
        available_paths,
    })
}

fn build_c_skeleton(
    path: &Path,
    source: &str,
    tree: &Tree,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    let root = tree.root_node();
    let mut cursor = root.walk();
    let mut skeleton_items = Vec::new();
    let mut available_paths = Vec::new();
    let expand_set: BTreeSet<_> = expand_nodes.iter().map(String::as_str).collect();

    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "type_definition" => {
                skeleton_items.push(node_text(child, source)?.trim().to_string());
                if let Some(symbol) = c_semantic_path(path, child, source)? {
                    available_paths.push(symbol);
                }
            }
            "declaration" if contains_kind(child, "function_declarator") => {
                skeleton_items.push(node_text(child, source)?.trim().to_string());
                if let Some(symbol) = c_semantic_path(path, child, source)? {
                    available_paths.push(symbol);
                }
            }
            "function_definition" => {
                if let Some(symbol) = c_semantic_path(path, child, source)? {
                    if expand_set.contains(symbol.as_str()) {
                        skeleton_items.push(node_text(child, source)?.trim().to_string());
                    } else {
                        skeleton_items.push(c_function_header(child, source)?);
                    }
                    available_paths.push(symbol);
                }
            }
            _ => {}
        }
    }

    Ok(SemanticSkeleton {
        file: normalize_path(path),
        skeleton: skeleton_items.join("\n\n"),
        available_paths,
    })
}

fn find_python_semantic_node<'tree>(
    tree: &'tree Tree,
    source: &str,
    target_path: &str,
) -> Result<Option<Node<'tree>>> {
    search_python_symbol(tree.root_node(), source, target_path)
}

fn find_c_semantic_node<'tree>(
    path: &Path,
    tree: &'tree Tree,
    source: &str,
    target_path: &str,
) -> Result<Option<Node<'tree>>> {
    let root = tree.root_node();
    let mut cursor = root.walk();
    let target_requires_symbol_id =
        target_path.contains("::") || target_path.contains('/') || target_path.contains('\\');
    let mut best_match = None;
    let mut best_rank = 0usize;

    for child in root.named_children(&mut cursor) {
        let symbol = c_semantic_path(path, child, source)?;
        let base_name = c_symbol_base_name(child, source)?;
        let symbol_id = if target_requires_symbol_id {
            c_symbol_id_for_node(path, child, source)?
        } else {
            None
        };

        let mut rank = None;
        if symbol_id.as_deref() == Some(target_path) {
            rank = Some(3000 + c_symbol_node_rank(child.kind()));
        } else if symbol.as_deref() == Some(target_path) {
            rank = Some(2000 + c_symbol_node_rank(child.kind()));
        } else if base_name.as_deref() == Some(target_path) {
            rank = Some(1000 + c_symbol_node_rank(child.kind()));
        }

        if let Some(rank) = rank {
            if rank > best_rank {
                best_rank = rank;
                best_match = Some(child);
            }
        }
    }

    Ok(best_match)
}

fn c_symbol_base_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    match node.kind() {
        "type_definition" => last_type_identifier(node, source),
        "declaration" if contains_kind(node, "function_declarator") => {
            first_identifier(node, source)
        }
        "function_definition" => first_identifier(node, source),
        _ => Ok(None),
    }
}

fn c_symbol_node_rank(node_kind: &str) -> usize {
    match node_kind {
        "function_definition" => 30,
        "type_definition" => 20,
        "declaration" => 10,
        _ => 0,
    }
}

fn c_symbol_family_anchor_for_name(path: &Path, source: &str, symbol_name: &str) -> Result<String> {
    let mut best_header = None;
    let mut best_rank = 0usize;

    for header_path in sibling_header_candidates(path) {
        if !header_path.exists() || !c_file_declares_symbol(&header_path, symbol_name)? {
            continue;
        }
        let rank = c_family_header_rank(path, &header_path, false);
        if rank > best_rank {
            best_rank = rank;
            best_header = Some(header_path);
        }
    }

    let mut visited = BTreeSet::new();
    let mut headers = BTreeSet::new();
    collect_declaring_include_headers(path, source, symbol_name, &mut headers, &mut visited)?;
    for header_path in headers {
        let rank = c_family_header_rank(path, Path::new(&header_path), true);
        if rank > best_rank {
            best_rank = rank;
            best_header = Some(Path::new(&header_path).to_path_buf());
        }
    }

    Ok(best_header
        .map(|header_path| normalize_path(&header_path))
        .unwrap_or_else(|| normalize_path(path)))
}

fn collect_declaring_include_headers(
    path: &Path,
    source: &str,
    symbol_name: &str,
    headers: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    let normalized = normalize_path(path);
    if !visited.insert(normalized) {
        return Ok(());
    }

    let document = parse_document(path, source)?;
    for include_target in c_include_targets(document.tree.root_node(), source)? {
        let Some(include_path) = resolve_local_c_include(path, &include_target) else {
            continue;
        };

        if is_c_header_path(&include_path) && c_file_declares_symbol(&include_path, symbol_name)? {
            headers.insert(normalize_path(&include_path));
        }

        let include_source = read_source(&include_path)?;
        collect_declaring_include_headers(
            &include_path,
            &include_source,
            symbol_name,
            headers,
            visited,
        )?;
    }

    Ok(())
}

fn c_file_declares_symbol(path: &Path, symbol_name: &str) -> Result<bool> {
    let source = read_source(path)?;
    let document = parse_document(path, &source)?;
    let root = document.tree.root_node();
    let mut cursor = root.walk();

    for child in root.named_children(&mut cursor) {
        if c_symbol_base_name(child, &source)?.as_deref() == Some(symbol_name) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn sibling_header_candidates(path: &Path) -> Vec<std::path::PathBuf> {
    ["h", "hpp", "hh"]
        .into_iter()
        .map(|extension| path.with_extension(extension))
        .collect()
}

fn c_family_header_rank(source_path: &Path, header_path: &Path, included: bool) -> usize {
    let mut rank = 0;
    if same_stem(source_path, header_path) {
        rank += 1000;
    }
    if included {
        rank += 500;
    }
    rank
}

fn same_stem(left: &Path, right: &Path) -> bool {
    left.file_stem()
        .and_then(|stem| stem.to_str())
        .zip(right.file_stem().and_then(|stem| stem.to_str()))
        .is_some_and(|(left_stem, right_stem)| left_stem == right_stem)
}

fn is_c_header_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "h" | "hpp" | "hh"))
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
        if let Some(child) = node.child(index) {
            if let Some(found) = search_python_symbol(child, source, target_path)? {
                return Ok(Some(found));
            }
        }
    }

    Ok(None)
}
