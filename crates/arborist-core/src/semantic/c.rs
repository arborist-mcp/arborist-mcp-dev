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
    let display_node = c_function_display_node(node);
    let Some(body) = node.child_by_field_name("body") else {
        if contains_kind(node, "default_method_clause")
            || contains_kind(node, "delete_method_clause")
        {
            return Ok(node_text(display_node, source)?.trim().to_string());
        }
        return Err(anyhow!("function_definition missing body"));
    };
    let prefix = source[display_node.start_byte()..body.start_byte()].trim_end();
    Ok(format!("{prefix};"))
}

fn c_function_display_node(node: Node<'_>) -> Node<'_> {
    let mut display_node = node;
    let mut current = node.parent();

    while let Some(candidate) = current {
        if candidate.kind() == "template_declaration" {
            display_node = candidate;
        }
        current = candidate.parent();
    }

    display_node
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

fn c_function_declarator_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if !is_c_callable_node(node) {
        return Ok(None);
    }

    let Some(function_declarator) = c_function_declarator(node) else {
        return Ok(None);
    };
    let Some(declarator) = function_declarator.child_by_field_name("declarator") else {
        return Ok(None);
    };
    if !matches!(
        declarator.kind(),
        "qualified_identifier"
            | "identifier"
            | "field_identifier"
            | "type_identifier"
            | "destructor_name"
            | "operator_name"
            | "template_function"
    ) {
        return Ok(None);
    }

    Ok(Some(
        node_text(declarator, source)?
            .trim()
            .trim_start_matches("::")
            .to_string(),
    ))
}

pub(crate) fn c_named_node_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(name) = node.child_by_field_name("name") else {
        return Ok(None);
    };

    Ok(Some(node_text(name, source)?.trim().to_string()))
}

pub(crate) fn c_using_declaration_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if node.kind() != "using_declaration" {
        return Ok(None);
    }

    let mut cursor = node.walk();
    let Some(target) = node.named_children(&mut cursor).next() else {
        return Ok(None);
    };
    let target_name = node_text(target, source)?.trim();
    Ok(target_name.rsplit("::").next().map(ToOwned::to_owned))
}

fn c_operator_cast_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if !is_c_callable_node(node) {
        return Ok(None);
    }

    if let Some(qualified_name) = find_qualified_operator_cast(node) {
        let Some(operator_cast) = find_first_descendant_by_kind(qualified_name, "operator_cast")
        else {
            return Ok(None);
        };
        let Some(target_type) = operator_cast.child_by_field_name("type") else {
            return Ok(None);
        };
        let qualifier = source[qualified_name.start_byte()..operator_cast.start_byte()].trim();
        return Ok(Some(
            format!(
                "{qualifier}operator {}",
                node_text(target_type, source)?.trim()
            )
            .trim_start_matches("::")
            .to_string(),
        ));
    }

    let Some(declarator) = find_first_descendant_by_kind(node, "operator_cast") else {
        return Ok(None);
    };
    let Some(target_type) = declarator.child_by_field_name("type") else {
        return Ok(None);
    };

    Ok(Some(format!(
        "operator {}",
        node_text(target_type, source)?.trim()
    )))
}

fn find_qualified_operator_cast(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "qualified_identifier" && contains_kind(node, "operator_cast") {
        return Some(node);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(qualified_name) = find_qualified_operator_cast(child) {
            return Some(qualified_name);
        }
    }

    None
}

pub(crate) fn c_parameters(node: Node<'_>, source: &str) -> Result<Vec<String>> {
    if !is_c_callable_node(node) {
        return Ok(Vec::new());
    }

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
    if !is_c_callable_node(node) {
        return Ok(None);
    }

    let Some(function_declarator) = c_function_declarator(node) else {
        return Ok(None);
    };

    let prefix = source[node.start_byte()..function_declarator.start_byte()].trim();
    if prefix.is_empty() {
        return Ok(None);
    }

    let prefix = if node.kind() == "template_instantiation" {
        prefix
            .strip_prefix("template")
            .map(str::trim)
            .unwrap_or(prefix)
    } else {
        prefix
    };

    Ok(Some(prefix.to_string()))
}

pub(crate) fn c_template_instantiation_name(
    node: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    if node.kind() != "template_instantiation" {
        return Ok(None);
    }

    if let Some(function_name) = c_function_declarator_name(node, source)? {
        return Ok(Some(function_name));
    }

    let Some(type_node) = node.child_by_field_name("type") else {
        return Ok(None);
    };
    if !is_cpp_type_scope(type_node) {
        return Ok(None);
    }

    c_named_node_name(type_node, source)
}

pub fn c_semantic_path(path: &Path, node: Node<'_>, source: &str) -> Result<Option<String>> {
    let symbol_name = c_function_declarator_name(node, source)?
        .or(c_operator_cast_name(node, source)?)
        .or(c_template_instantiation_name(node, source)?)
        .or(match node.kind() {
            "type_definition" => last_type_identifier(node, source)?,
            "alias_declaration"
            | "class_specifier"
            | "concept_definition"
            | "enum_specifier"
            | "namespace_alias_definition"
            | "struct_specifier"
            | "union_specifier" => c_named_node_name(node, source)?,
            "using_declaration" => c_using_declaration_name(node, source)?,
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

pub(crate) fn c_symbol_nodes<'tree>(
    path: &Path,
    root: Node<'tree>,
    source: &str,
) -> Result<Vec<Node<'tree>>> {
    let mut symbols = Vec::new();
    collect_c_scope_symbols(root, &mut symbols);
    if !symbols
        .iter()
        .any(|node| node.kind() == "using_declaration")
    {
        return Ok(symbols);
    }

    let mut non_using_paths = BTreeSet::new();
    for node in &symbols {
        if node.kind() != "using_declaration"
            && let Some(symbol_path) = c_semantic_path(path, *node, source)?
        {
            non_using_paths.insert(symbol_path);
        }
    }

    let mut using_paths = BTreeSet::new();
    let mut deduplicated = Vec::new();
    for node in symbols {
        if node.kind() != "using_declaration" {
            deduplicated.push(node);
            continue;
        }
        let Some(symbol_path) = c_semantic_path(path, node, source)? else {
            continue;
        };
        if !non_using_paths.contains(&symbol_path) && using_paths.insert(symbol_path) {
            deduplicated.push(node);
        }
    }

    Ok(deduplicated)
}

fn collect_c_scope_symbols<'tree>(scope: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    if scope.kind() == "linkage_specification" {
        let Some(body) = scope.child_by_field_name("body") else {
            return;
        };
        if body.kind() == "declaration_list" {
            collect_c_scope_symbols(body, symbols);
        } else {
            collect_c_scope_child(body, symbols);
        }
        return;
    }

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
        collect_c_scope_child(child, symbols);
    }
}

fn collect_c_scope_child<'tree>(child: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    if matches!(
        child.kind(),
        "linkage_specification" | "namespace_definition"
    ) {
        collect_c_scope_symbols(child, symbols);
    } else if is_c_preprocessor_conditional(child) {
        collect_c_preprocessor_symbols(child, symbols);
    } else if child.kind() == "template_declaration" {
        collect_cpp_template_symbols(child, symbols);
    } else if is_cpp_type_scope(child) {
        collect_cpp_type_scope_symbols(child, symbols);
    } else if child.kind() == "declaration" {
        collect_c_named_type_definition_symbols(child, symbols);
        if is_c_symbol_node(child) {
            symbols.push(child);
        }
    } else if is_c_symbol_node(child) {
        symbols.push(child);
    }
}

fn is_c_preprocessor_conditional(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "preproc_else" | "preproc_elif" | "preproc_elifdef" | "preproc_if" | "preproc_ifdef"
    )
}

fn collect_c_preprocessor_symbols<'tree>(conditional: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    let mut cursor = conditional.walk();
    for child in conditional.named_children(&mut cursor) {
        collect_c_scope_child(child, symbols);
    }
}

fn is_cpp_type_scope(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "class_specifier" | "struct_specifier" | "union_specifier"
    )
}

fn collect_c_named_type_definition_symbols<'tree>(
    declaration: Node<'tree>,
    symbols: &mut Vec<Node<'tree>>,
) {
    let mut cursor = declaration.walk();
    for child in declaration.named_children(&mut cursor) {
        if is_cpp_type_scope(child) && child.child_by_field_name("body").is_some() {
            collect_cpp_type_scope_symbols(child, symbols);
        }
    }
}

fn collect_cpp_type_scope_symbols<'tree>(type_node: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    symbols.push(type_node);

    let Some(body) = type_node.child_by_field_name("body") else {
        return;
    };
    let mut cursor = body.walk();

    for child in body.named_children(&mut cursor) {
        collect_cpp_type_scope_child(child, symbols);
    }
}

fn collect_cpp_type_scope_child<'tree>(child: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    if is_c_preprocessor_conditional(child) {
        let mut cursor = child.walk();
        for nested_child in child.named_children(&mut cursor) {
            collect_cpp_type_scope_child(nested_child, symbols);
        }
    } else if child.kind() == "friend_declaration" {
        collect_cpp_friend_function_symbols(child, symbols);
    } else if is_cpp_type_scope(child) {
        collect_cpp_type_scope_symbols(child, symbols);
    } else if child.kind() == "field_declaration" {
        collect_cpp_nested_type_symbols(child, symbols);
        if c_is_callable_declaration(child) {
            symbols.push(child);
        }
    } else if child.kind() == "template_declaration" {
        collect_cpp_template_symbols(child, symbols);
    } else if is_c_symbol_node(child) {
        symbols.push(child);
    }
}

fn collect_cpp_friend_function_symbols<'tree>(
    friend_declaration: Node<'tree>,
    symbols: &mut Vec<Node<'tree>>,
) {
    let mut cursor = friend_declaration.walk();
    for child in friend_declaration.named_children(&mut cursor) {
        if child.kind() == "function_definition" || c_is_callable_declaration(child) {
            symbols.push(child);
        }
    }
}

fn collect_cpp_nested_type_symbols<'tree>(
    declaration: Node<'tree>,
    symbols: &mut Vec<Node<'tree>>,
) {
    let mut cursor = declaration.walk();
    for child in declaration.named_children(&mut cursor) {
        if is_cpp_type_scope(child) {
            collect_cpp_type_scope_symbols(child, symbols);
        } else if child.kind() == "enum_specifier" {
            symbols.push(child);
        }
    }
}

fn collect_cpp_template_symbols<'tree>(template_node: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    let mut cursor = template_node.walk();

    for child in template_node.named_children(&mut cursor) {
        if child.kind() == "template_declaration" {
            collect_cpp_template_symbols(child, symbols);
        } else if child.kind() == "friend_declaration" {
            collect_cpp_friend_function_symbols(child, symbols);
        } else if is_cpp_type_scope(child) {
            collect_cpp_type_scope_symbols(child, symbols);
        } else if child.kind() == "declaration" {
            collect_c_named_type_definition_symbols(child, symbols);
            if is_c_symbol_node(child) {
                symbols.push(child);
            }
        } else if is_c_symbol_node(child) {
            symbols.push(child);
        }
    }
}

fn is_c_symbol_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "alias_declaration"
            | "class_specifier"
            | "concept_definition"
            | "enum_specifier"
            | "namespace_alias_definition"
            | "struct_specifier"
            | "template_instantiation"
            | "type_definition"
            | "union_specifier"
            | "using_declaration"
            | "function_definition"
    ) || c_is_callable_declaration(node)
}

pub(crate) fn c_is_callable_declaration(node: Node<'_>) -> bool {
    matches!(node.kind(), "declaration" | "field_declaration")
        && (contains_kind(node, "function_declarator") || contains_kind(node, "operator_cast"))
}

fn is_c_callable_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "function_definition" | "template_instantiation"
    ) || c_is_callable_declaration(node)
}

fn c_scope_path(node: Node<'_>, source: &str) -> Result<String> {
    let mut scopes = Vec::new();
    let mut current = node.parent();
    let skip_enclosing_type_scopes = has_friend_declaration_ancestor(node);

    while let Some(candidate) = current {
        if (candidate.kind() == "namespace_definition"
            || (!skip_enclosing_type_scopes && is_cpp_type_scope(candidate)))
            && let Some(name) = candidate.child_by_field_name("name")
        {
            scopes.push(node_text(name, source)?.trim().to_string());
        }
        current = candidate.parent();
    }

    scopes.reverse();
    Ok(scopes.join("::"))
}

fn has_friend_declaration_ancestor(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "friend_declaration" {
            return true;
        }
        current = candidate.parent();
    }
    false
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
    if has_type_scope_ancestor(node) {
        return false;
    }
    if has_anonymous_namespace_ancestor(node) {
        return true;
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

fn has_anonymous_namespace_ancestor(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "namespace_definition"
            && candidate.child_by_field_name("name").is_none()
        {
            return true;
        }
        current = candidate.parent();
    }
    false
}

fn has_type_scope_ancestor(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if is_cpp_type_scope(candidate) {
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

    for child in c_symbol_nodes(path, root, source)? {
        match child.kind() {
            "alias_declaration"
            | "class_specifier"
            | "concept_definition"
            | "enum_specifier"
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
    let target_requires_symbol_id =
        target_path.contains("::") || target_path.contains('/') || target_path.contains('\\');
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
