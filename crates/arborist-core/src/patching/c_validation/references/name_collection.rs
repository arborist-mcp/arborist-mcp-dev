use std::collections::BTreeSet;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, visit_tree};

pub(in super::super) fn collect_c_local_definitions(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    collect_c_local_definitions_in_node(node, source, names)?;
    collect_cpp_template_parameter_definitions(node, source, names)
}

fn collect_c_local_definitions_in_node(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if let Some(parent) = candidate.parent()
            && candidate.kind() == "identifier"
            && matches!(
                parent.kind(),
                "declaration"
                    | "init_declarator"
                    | "parameter_declaration"
                    | "optional_parameter_declaration"
                    | "variadic_parameter_declaration"
                    | "variadic_declarator"
                    | "function_declarator"
                    | "pointer_declarator"
                    | "array_declarator"
            )
        {
            let _ = node_text(candidate, source).map(|text| names.insert(text.trim().to_string()));
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn collect_cpp_template_parameter_definitions(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "template_declaration" {
            let mut cursor = candidate.walk();
            for child in candidate.named_children(&mut cursor) {
                if child.kind() == "template_parameter_list" {
                    collect_c_local_definitions_in_node(child, source, names)?;
                }
            }
        }
        current = candidate.parent();
    }
    Ok(())
}

pub(crate) fn collect_c_references(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    collect_c_references_with_options(node, source, references, false)
}

pub(crate) fn collect_c_graph_references(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    collect_c_references_with_options(node, source, references, true)
}

fn collect_c_references_with_options(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
    suppress_direct_qualified_call_components: bool,
) -> Result<()> {
    let mut template_parameters = BTreeSet::new();
    collect_cpp_template_parameter_definitions(node, source, &mut template_parameters)?;
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() == "identifier"
            && !is_c_enumerator_name(candidate)
            && !is_cpp_new_type_qualifier_recovery_identifier(candidate, source)
            && !is_cpp_type_template_argument(candidate)
            && (!suppress_direct_qualified_call_components
                || !is_direct_qualified_call_component(candidate))
        {
            let _ = node_text(candidate, source).map(|text| {
                let name = text.trim().to_string();
                if !template_parameters.contains(&name)
                    || is_qualified_identifier_component(candidate)
                {
                    references.insert(name);
                }
            });
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn is_cpp_new_type_qualifier_recovery_identifier(candidate: Node<'_>, source: &str) -> bool {
    let Some(error) = candidate.parent().filter(|parent| parent.is_error()) else {
        return false;
    };
    if error
        .parent()
        .is_none_or(|parent| parent.kind() != "new_expression")
    {
        return false;
    }
    let qualifier_prefix = source[error.start_byte()..candidate.start_byte()].trim();
    !qualifier_prefix.is_empty()
        && qualifier_prefix
            .split_whitespace()
            .all(|part| matches!(part, "const" | "volatile"))
}

fn is_qualified_identifier_component(node: Node<'_>) -> bool {
    node.parent()
        .is_some_and(|parent| parent.kind() == "qualified_identifier")
}

fn is_direct_qualified_call_component(node: Node<'_>) -> bool {
    let Some(qualified_identifier) = node.parent() else {
        return false;
    };
    is_direct_qualified_call(qualified_identifier)
}

fn is_direct_qualified_call(qualified_identifier: Node<'_>) -> bool {
    if qualified_identifier.kind() != "qualified_identifier" {
        return false;
    }
    qualified_identifier.parent().is_some_and(|parent| {
        parent.kind() == "call_expression"
            && parent
                .child_by_field_name("function")
                .is_some_and(|function| function == qualified_identifier)
    })
}

fn is_cpp_type_template_argument(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "template_argument_list" {
            return candidate
                .parent()
                .is_some_and(|parent| parent.kind() == "template_type");
        }
        if matches!(
            candidate.kind(),
            "call_expression" | "declaration" | "parameter_declaration"
        ) {
            return false;
        }
        current = candidate.parent();
    }
    false
}

fn is_c_enumerator_name(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        parent.kind() == "enumerator"
            && parent
                .child_by_field_name("name")
                .is_some_and(|name| name == node)
    })
}
