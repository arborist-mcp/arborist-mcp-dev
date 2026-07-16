use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use tree_sitter::{Node, Tree};

use super::semantic_parent_path;
use crate::language::{
    C_FAMILY_HEADER_EXTENSIONS, c_include_targets, contains_kind, extension_case_candidates,
    first_identifier, is_c_header_path, last_type_identifier, node_text, normalize_path,
    parse_document, read_source, resolve_local_c_include,
};
use crate::model::{SemanticSkeleton, SemanticSkeletonSymbol};

pub fn c_function_header(node: Node<'_>, source: &str) -> Result<String> {
    let body = node
        .child_by_field_name("body")
        .ok_or_else(|| anyhow!("function_definition missing body"))?;
    let prefix = source[node.start_byte()..body.start_byte()].trim_end();
    Ok(format!("{prefix};"))
}

fn find_first_descendant_by_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = find_first_descendant_by_kind(child, kind) {
            return Some(found);
        }
    }

    None
}

fn c_function_declarator(node: Node<'_>) -> Option<Node<'_>> {
    find_first_descendant_by_kind(node, "function_declarator")
}

fn c_qualified_function_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(function_declarator) = c_function_declarator(node) else {
        return Ok(None);
    };
    let Some(declarator) = function_declarator.child_by_field_name("declarator") else {
        return Ok(None);
    };
    if declarator.kind() != "qualified_identifier" {
        return Ok(None);
    }

    Ok(Some(
        node_text(declarator, source)?
            .trim()
            .trim_start_matches("::")
            .to_string(),
    ))
}

pub(crate) fn c_parameters(node: Node<'_>, source: &str) -> Result<Vec<String>> {
    let Some(function_declarator) = c_function_declarator(node) else {
        return Ok(Vec::new());
    };
    let Some(parameters) = function_declarator.child_by_field_name("parameters") else {
        return Ok(Vec::new());
    };

    let mut cursor = parameters.walk();
    let mut values = Vec::new();
    for child in parameters.named_children(&mut cursor) {
        values.push(node_text(child, source)?.trim().to_string());
    }
    Ok(values)
}

pub(crate) fn c_return_type(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(function_declarator) = c_function_declarator(node) else {
        return Ok(None);
    };

    let prefix = source[node.start_byte()..function_declarator.start_byte()].trim();
    if prefix.is_empty() {
        return Ok(None);
    }

    Ok(Some(prefix.to_string()))
}

pub fn c_semantic_path(path: &Path, node: Node<'_>, source: &str) -> Result<Option<String>> {
    let symbol_name = c_qualified_function_name(node, source)?.or(match node.kind() {
        "type_definition" => last_type_identifier(node, source)?,
        "declaration" | "field_declaration" | "function_definition" => {
            first_identifier(node, source)?
        }
        _ => None,
    });

    let scope_path = c_scope_path(node, source)?;
    Ok(symbol_name.map(|name| {
        let name = c_qualified_name_in_scope(&scope_path, &name);
        if has_c_internal_linkage(node, source) {
            format!("{}::{name}", normalize_path(path))
        } else {
            name
        }
    }))
}

fn c_qualified_name_in_scope(scope_path: &str, name: &str) -> String {
    if scope_path.is_empty()
        || name == scope_path
        || name
            .strip_prefix(scope_path)
            .is_some_and(|suffix| suffix.starts_with("::"))
    {
        name.to_string()
    } else {
        format!("{scope_path}::{name}")
    }
}

pub(crate) fn c_symbol_nodes(root: Node<'_>) -> Vec<Node<'_>> {
    let mut symbols = Vec::new();
    collect_c_scope_symbols(root, &mut symbols);
    symbols
}

fn collect_c_scope_symbols<'tree>(scope: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    let scope = if scope.kind() == "namespace_definition" {
        match scope.child_by_field_name("body") {
            Some(body) => body,
            None => return,
        }
    } else {
        scope
    };
    let mut cursor = scope.walk();

    for child in scope.named_children(&mut cursor) {
        if child.kind() == "namespace_definition" {
            collect_c_scope_symbols(child, symbols);
        } else if child.kind() == "class_specifier" {
            collect_cpp_class_method_symbols(child, symbols);
        } else if is_c_symbol_node(child) {
            symbols.push(child);
        }
    }
}

fn collect_cpp_class_method_symbols<'tree>(
    class_node: Node<'tree>,
    symbols: &mut Vec<Node<'tree>>,
) {
    let Some(body) = class_node.child_by_field_name("body") else {
        return;
    };
    let mut cursor = body.walk();

    for child in body.named_children(&mut cursor) {
        if child.kind() == "class_specifier" {
            collect_cpp_class_method_symbols(child, symbols);
        } else if is_c_symbol_node(child) {
            symbols.push(child);
        }
    }
}

fn is_c_symbol_node(node: Node<'_>) -> bool {
    matches!(node.kind(), "type_definition" | "function_definition")
        || matches!(node.kind(), "declaration" | "field_declaration")
            && contains_kind(node, "function_declarator")
}

fn c_scope_path(node: Node<'_>, source: &str) -> Result<String> {
    let mut scopes = Vec::new();
    let mut current = node.parent();

    while let Some(candidate) = current {
        if matches!(candidate.kind(), "namespace_definition" | "class_specifier")
            && let Some(name) = candidate.child_by_field_name("name")
        {
            scopes.push(node_text(name, source)?.trim().to_string());
        }
        current = candidate.parent();
    }

    scopes.reverse();
    Ok(scopes.join("::"))
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

pub(crate) fn has_c_internal_linkage(node: Node<'_>, source: &str) -> bool {
    if has_class_ancestor(node) {
        return false;
    }

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

fn has_class_ancestor(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "class_specifier" {
            return true;
        }
        current = candidate.parent();
    }
    false
}

pub(crate) fn build_c_skeleton(
    path: &Path,
    source: &str,
    tree: &Tree,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    let root = tree.root_node();
    let mut skeleton_items = Vec::new();
    let mut available_paths = Vec::new();
    let mut available_symbols = Vec::new();
    let expand_set: BTreeSet<_> = expand_nodes.iter().map(String::as_str).collect();

    for child in c_symbol_nodes(root) {
        match child.kind() {
            "type_definition" => {
                let text = node_text(child, source)?.trim().to_string();
                skeleton_items.push(text.clone());
                if let Some(symbol) = c_semantic_path(path, child, source)? {
                    let symbol_id = c_symbol_id_for_node(path, child, source)?
                        .unwrap_or_else(|| symbol.clone());
                    let scope_path = semantic_parent_path(&symbol);
                    let parameters = c_parameters(child, source)?;
                    let return_type = c_return_type(child, source)?;
                    available_paths.push(symbol.clone());
                    available_symbols.push(SemanticSkeletonSymbol {
                        symbol_id,
                        semantic_path: symbol,
                        scope_path,
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(text),
                        parameters,
                        return_type,
                        docstring: None,
                    });
                }
            }
            "declaration" | "field_declaration" if contains_kind(child, "function_declarator") => {
                let text = node_text(child, source)?.trim().to_string();
                skeleton_items.push(text.clone());
                if let Some(symbol) = c_semantic_path(path, child, source)? {
                    let symbol_id = c_symbol_id_for_node(path, child, source)?
                        .unwrap_or_else(|| symbol.clone());
                    let scope_path = semantic_parent_path(&symbol);
                    let parameters = c_parameters(child, source)?;
                    let return_type = c_return_type(child, source)?;
                    available_paths.push(symbol.clone());
                    available_symbols.push(SemanticSkeletonSymbol {
                        symbol_id,
                        semantic_path: symbol,
                        scope_path,
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(text),
                        parameters,
                        return_type,
                        docstring: None,
                    });
                }
            }
            "function_definition" => {
                if let Some(symbol) = c_semantic_path(path, child, source)? {
                    let symbol_id = c_symbol_id_for_node(path, child, source)?
                        .unwrap_or_else(|| symbol.clone());
                    let scope_path = semantic_parent_path(&symbol);
                    let signature = c_function_header(child, source)?;
                    let parameters = c_parameters(child, source)?;
                    let return_type = c_return_type(child, source)?;
                    if expand_set.contains(symbol.as_str())
                        || expand_set.contains(symbol_id.as_str())
                    {
                        skeleton_items.push(node_text(child, source)?.trim().to_string());
                    } else {
                        skeleton_items.push(signature.clone());
                    }
                    available_paths.push(symbol.clone());
                    available_symbols.push(SemanticSkeletonSymbol {
                        symbol_id,
                        semantic_path: symbol,
                        scope_path,
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(signature),
                        parameters,
                        return_type,
                        docstring: None,
                    });
                }
            }
            _ => {}
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

pub(crate) fn find_c_semantic_node<'tree>(
    path: &Path,
    tree: &'tree Tree,
    source: &str,
    target_path: &str,
) -> Result<Option<Node<'tree>>> {
    let root = tree.root_node();
    let target_requires_symbol_id =
        target_path.contains("::") || target_path.contains('/') || target_path.contains('\\');
    let mut best_match = None;
    let mut best_rank = 0usize;

    for child in c_symbol_nodes(root) {
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

        if let Some(rank) = rank
            && rank > best_rank
        {
            best_rank = rank;
            best_match = Some(child);
        }
    }

    Ok(best_match)
}

fn c_symbol_base_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if let Some(qualified_name) = c_qualified_function_name(node, source)? {
        return Ok(qualified_name.rsplit("::").next().map(ToString::to_string));
    }

    match node.kind() {
        "type_definition" => last_type_identifier(node, source),
        "declaration" | "field_declaration" if contains_kind(node, "function_declarator") => {
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
        "declaration" | "field_declaration" => 10,
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

fn sibling_header_candidates(path: &Path) -> Vec<PathBuf> {
    extension_case_candidates(path, C_FAMILY_HEADER_EXTENSIONS)
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
