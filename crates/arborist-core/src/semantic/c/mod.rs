use std::path::Path;

use anyhow::{Result, anyhow};
use tree_sitter::Node;

use crate::language::{
    contains_kind, first_identifier, last_type_identifier, node_text, normalize_path,
};

mod identity;
mod skeleton;
mod symbols;

pub(crate) use identity::cpp_callable_symbol_id;
pub use skeleton::c_symbol_id_for_node;
pub(crate) use skeleton::{
    build_c_skeleton, c_symbol_id_for_node_with_deadline, find_c_semantic_node,
};

use symbols::is_cpp_type_scope;
pub(crate) use symbols::{c_symbol_nodes, c_symbol_nodes_with_deadline};

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

pub(super) fn c_function_display_node(node: Node<'_>) -> Node<'_> {
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

pub(super) fn find_first_descendant_by_kind<'tree>(
    node: Node<'tree>,
    kind: &str,
) -> Option<Node<'tree>> {
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

pub(super) fn c_function_declarator(node: Node<'_>) -> Option<Node<'_>> {
    find_first_descendant_by_kind(node, "function_declarator")
}

pub(super) fn c_function_declarator_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
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

fn c_enumerator_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if node.kind() != "enumerator" {
        return Ok(None);
    }

    c_named_node_name(node, source)
}

fn c_enumerator_semantic_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(name) = c_enumerator_name(node, source)? else {
        return Ok(None);
    };
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "enum_specifier" {
            if c_is_scoped_enum(candidate, source)
                && let Some(enum_name) = c_named_node_name(candidate, source)?
            {
                return Ok(Some(format!("{enum_name}::{name}")));
            }
            break;
        }
        current = candidate.parent();
    }

    Ok(Some(name))
}

pub(crate) fn c_is_scoped_enumerator(node: Node<'_>, source: &str) -> bool {
    if node.kind() != "enumerator" {
        return false;
    }

    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "enum_specifier" {
            return c_is_scoped_enum(candidate, source);
        }
        current = candidate.parent();
    }

    false
}

fn c_is_scoped_enum(enum_node: Node<'_>, source: &str) -> bool {
    let Some(name) = enum_node.child_by_field_name("name") else {
        return false;
    };
    source[enum_node.start_byte()..name.start_byte()]
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|word| matches!(word, "class" | "struct"))
}

pub(super) fn c_operator_cast_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
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
    let mut cursor = parameters.walk();
    if parameters
        .children(&mut cursor)
        .any(|child| child.kind() == "...")
    {
        values.push("...".to_string());
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
        .or(c_enumerator_semantic_name(node, source)?)
        .or(match node.kind() {
            "type_definition" => last_type_identifier(node, source)?,
            "alias_declaration"
            | "class_specifier"
            | "concept_definition"
            | "enum_specifier"
            | "enumerator"
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

pub(crate) fn c_is_callable_declaration(node: Node<'_>) -> bool {
    matches!(node.kind(), "declaration" | "field_declaration")
        && (contains_kind(node, "function_declarator") || contains_kind(node, "operator_cast"))
}

pub(super) fn is_c_callable_node(node: Node<'_>) -> bool {
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
