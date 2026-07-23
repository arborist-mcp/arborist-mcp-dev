use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::{Node, Tree};

use super::super::semantic_parent_path;
use super::identity::cpp_callable_symbol_id;
use super::{
    c_function_declarator, c_function_declarator_name, c_function_display_node, c_function_header,
    c_is_callable_declaration, c_named_node_name, c_operator_cast_name, c_parameters,
    c_return_type, c_semantic_path, c_symbol_nodes, c_template_instantiation_name,
    c_using_declaration_name, is_c_callable_node,
};
use crate::language::{
    C_FAMILY_HEADER_EXTENSIONS, c_include_targets, detect_language, extension_case_candidates,
    first_identifier, is_c_header_path, last_type_identifier, node_text, normalize_path,
    parse_document, read_source, resolve_local_c_include,
};
use crate::model::{LanguageId, SemanticSkeleton, SemanticSkeletonSymbol};

// helpers used only by skeleton that lived after build_c_skeleton originally
// are included in skeleton_body below.

pub fn c_symbol_id_for_node(path: &Path, node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(semantic_path) = c_semantic_path(path, node, source)? else {
        return Ok(None);
    };

    if detect_language(path).ok() == Some(LanguageId::Cpp) && is_c_callable_node(node) {
        let signature = c_function_declarator(node)
            .map(|declarator| node_text(declarator, source).map(str::trim))
            .transpose()?
            .or_else(|| node_text(node, source).ok().map(str::trim));
        return Ok(Some(cpp_callable_symbol_id(
            &semantic_path,
            &c_parameters(node, source)?,
            signature,
        )));
    }

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

    for child in c_symbol_nodes(path, root, source)? {
        match child.kind() {
            "alias_declaration"
            | "class_specifier"
            | "concept_definition"
            | "enum_specifier"
            | "enumerator"
            | "namespace_alias_definition"
            | "struct_specifier"
            | "template_instantiation"
            | "type_definition"
            | "union_specifier"
            | "using_declaration" => {
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
            "declaration" | "field_declaration" if c_is_callable_declaration(child) => {
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
                        skeleton_items.push(
                            node_text(c_function_display_node(child), source)?
                                .trim()
                                .to_string(),
                        );
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
    let target_requires_symbol_id = target_path.contains("::")
        || target_path.contains('(')
        || target_path.contains('/')
        || target_path.contains('\\');
    let mut best_match = None;
    let mut best_rank = 0usize;

    for child in c_symbol_nodes(path, root, source)? {
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
    if let Some(function_name) = c_function_declarator_name(node, source)? {
        return Ok(function_name.rsplit("::").next().map(c_callable_base_name));
    }
    if let Some(operator_cast_name) = c_operator_cast_name(node, source)? {
        return Ok(Some(operator_cast_name));
    }

    match node.kind() {
        "type_definition" => last_type_identifier(node, source),
        "alias_declaration"
        | "class_specifier"
        | "concept_definition"
        | "enum_specifier"
        | "enumerator"
        | "namespace_alias_definition"
        | "struct_specifier"
        | "union_specifier" => c_named_node_name(node, source),
        "using_declaration" => c_using_declaration_name(node, source),
        "template_instantiation" => c_template_instantiation_name(node, source),
        "declaration" | "field_declaration" if c_is_callable_declaration(node) => {
            first_identifier(node, source)
        }
        "function_definition" => first_identifier(node, source),
        _ => Ok(None),
    }
}

fn c_callable_base_name(name: &str) -> String {
    name.split_once('<')
        .map(|(base_name, _)| base_name)
        .unwrap_or(name)
        .to_string()
}

fn c_symbol_node_rank(node_kind: &str) -> usize {
    match node_kind {
        "function_definition" => 30,
        "alias_declaration"
        | "class_specifier"
        | "concept_definition"
        | "enum_specifier"
        | "enumerator"
        | "namespace_alias_definition"
        | "struct_specifier"
        | "template_instantiation"
        | "type_definition"
        | "union_specifier"
        | "using_declaration" => 20,
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
