use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, visit_tree};

pub(super) fn collect_c_local_definitions(
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

pub(crate) fn collect_c_call_arities(
    node: Node<'_>,
    source: &str,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "call_expression" {
            return;
        }
        let Some(function) = candidate.child_by_field_name("function") else {
            return;
        };
        let Some(arguments) = candidate.child_by_field_name("arguments") else {
            return;
        };
        let Ok(Some(name)) = direct_c_call_name(function, source) else {
            return;
        };

        let mut cursor = arguments.walk();
        let arity = arguments.named_children(&mut cursor).count();
        call_arities
            .entry(name.trim().to_string())
            .or_default()
            .insert(arity);
    };
    visit_tree(node, &mut callback);
    Ok(())
}

pub(crate) fn collect_cpp_braced_call_arities(
    node: Node<'_>,
    source: &str,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "compound_literal_expression" {
            return;
        }
        let mut cursor = candidate.walk();
        let children = candidate.named_children(&mut cursor).collect::<Vec<_>>();
        let type_node = candidate
            .child_by_field_name("type")
            .or_else(|| children.first().copied());
        let initializer = candidate.child_by_field_name("value").or_else(|| {
            children
                .iter()
                .copied()
                .find(|child| child.kind() == "initializer_list")
        });
        let (Some(type_node), Some(initializer)) = (type_node, initializer) else {
            return;
        };
        let Ok(Some(name)) = direct_c_call_name(type_node, source) else {
            return;
        };

        let mut initializer_cursor = initializer.walk();
        let arity = initializer.named_children(&mut initializer_cursor).count();
        call_arities
            .entry(name.trim().to_string())
            .or_default()
            .insert(arity);
    };
    visit_tree(node, &mut callback);
    Ok(())
}

pub(crate) fn collect_cpp_new_call_arities(
    node: Node<'_>,
    source: &str,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "new_expression" {
            return;
        }
        let (Some(type_node), Some(arguments)) = (
            candidate.child_by_field_name("type"),
            candidate.child_by_field_name("arguments"),
        ) else {
            return;
        };
        let Ok(Some(name)) = direct_c_call_name(type_node, source) else {
            return;
        };

        let mut cursor = arguments.walk();
        let arity = arguments.named_children(&mut cursor).count();
        call_arities
            .entry(name.trim().to_string())
            .or_default()
            .insert(arity);
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn direct_c_call_name(function: Node<'_>, source: &str) -> Result<Option<String>> {
    match function.kind() {
        "identifier" | "type_identifier" | "template_type" => {
            Ok(Some(node_text(function, source)?.trim().to_string()))
        }
        "qualified_identifier" => qualified_c_call_name(function, source),
        "template_function" => template_function_name(function, source),
        _ => Ok(None),
    }
}

fn qualified_c_call_name(function: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut cursor = function.walk();
    let template_function = function
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "template_function")
        .last();
    let Some(template_function) = template_function else {
        return Ok(Some(node_text(function, source)?.trim().to_string()));
    };
    let Some(name) = template_function_name(template_function, source)? else {
        return Ok(None);
    };

    let prefix = source[function.start_byte()..template_function.start_byte()].trim_end();
    let prefix = prefix.strip_suffix("template").unwrap_or(prefix).trim_end();
    Ok(Some(format!("{prefix}{name}")))
}

fn template_function_name(function: Node<'_>, source: &str) -> Result<Option<String>> {
    function
        .child_by_field_name("name")
        .map(|name| node_text(name, source).map(|name| name.trim().to_string()))
        .transpose()
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

fn is_c_enumerator_name(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        parent.kind() == "enumerator"
            && parent
                .child_by_field_name("name")
                .is_some_and(|name| name == node)
    })
}
