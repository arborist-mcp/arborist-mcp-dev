use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::language::{node_text, visit_tree, visit_tree_with_deadline};

pub(in super::super) fn collect_c_local_definitions(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    collect_c_local_definitions_with_deadline(node, source, names, None)
}

pub(in super::super) fn collect_c_local_definitions_with_deadline(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    collect_c_local_definitions_in_node(node, source, names, deadline)?;
    collect_cpp_template_parameter_definitions(node, source, names, deadline)
}

fn collect_c_local_definitions_in_node(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
    deadline: Option<&dyn DeadlineCheck>,
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
    match deadline {
        Some(deadline) => visit_tree_with_deadline(node, &mut callback, deadline)?,
        None => visit_tree(node, &mut callback),
    }
    Ok(())
}

pub(in super::super) fn collect_c_scope_escaped_local_definition_names(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    collect_c_scope_escaped_local_definition_names_with_deadline(node, source, names, None)
}

pub(in super::super) fn collect_c_scope_escaped_local_definition_names_with_deadline(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut definitions = BTreeMap::new();
    let mut collect_definition = |candidate: Node<'_>| {
        if let Some(parent) = candidate.parent()
            && candidate.kind() == "identifier"
            && is_c_local_definition_parent(parent.kind())
            && let Some(scope) = c_local_definition_scope(candidate)
            && let Ok(name) = node_text(candidate, source)
        {
            let name = name.trim();
            if !name.is_empty() {
                definitions
                    .entry(name.to_string())
                    .or_insert_with(Vec::new)
                    .push(CScopedLocalDefinition {
                        declaration_start: candidate.start_byte(),
                        scope_range: (scope.start_byte(), scope.end_byte()),
                    });
            }
        }
    };
    match deadline {
        Some(deadline) => visit_tree_with_deadline(node, &mut collect_definition, deadline)?,
        None => visit_tree(node, &mut collect_definition),
    }

    if definitions.is_empty() {
        return Ok(());
    }

    let mut collect_scope_escapes = |candidate: Node<'_>| {
        if candidate.kind() != "identifier"
            || is_c_enumerator_name(candidate)
            || candidate
                .parent()
                .is_some_and(|parent| is_c_local_definition_parent(parent.kind()))
        {
            return;
        }
        let Ok(name) = node_text(candidate, source) else {
            return;
        };
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let Some(name_definitions) = definitions.get(name) else {
            return;
        };
        let reference_start = candidate.start_byte();
        let reference_end = candidate.end_byte();
        let is_visible = name_definitions.iter().any(|definition| {
            definition.declaration_start <= reference_start
                && definition.scope_range.0 <= reference_start
                && definition.scope_range.1 >= reference_end
        });
        if !is_visible {
            names.insert(name.to_string());
        }
    };
    match deadline {
        Some(deadline) => visit_tree_with_deadline(node, &mut collect_scope_escapes, deadline),
        None => {
            visit_tree(node, &mut collect_scope_escapes);
            Ok(())
        }
    }
}

struct CScopedLocalDefinition {
    declaration_start: usize,
    scope_range: (usize, usize),
}

fn is_c_local_definition_parent(kind: &str) -> bool {
    matches!(
        kind,
        "declaration"
            | "init_declarator"
            | "parameter_declaration"
            | "optional_parameter_declaration"
            | "variadic_parameter_declaration"
            | "variadic_declarator"
            | "pointer_declarator"
            | "array_declarator"
    )
}

fn c_local_definition_scope(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "for_statement"
                | "for_range_loop"
                | "if_statement"
                | "switch_statement"
                | "while_statement"
                | "catch_clause"
                | "compound_statement"
        ) {
            return Some(candidate);
        }
        if candidate.kind() == "function_definition" {
            return candidate.child_by_field_name("body");
        }
        current = candidate.parent();
    }
    None
}

fn collect_cpp_template_parameter_definitions(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if let Some(deadline) = deadline {
            deadline.check("collecting C/C++ template parameters")?;
        }
        if candidate.kind() == "template_declaration" {
            let mut cursor = candidate.walk();
            for child in candidate.named_children(&mut cursor) {
                if child.kind() == "template_parameter_list" {
                    collect_c_local_definitions_in_node(child, source, names, deadline)?;
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
    collect_c_references_with_options(node, source, references, false, None)
}

pub(crate) fn collect_c_graph_references(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    collect_c_references_with_options(node, source, references, true, None)
}

pub(crate) fn collect_c_references_with_deadline(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    collect_c_references_with_options(node, source, references, false, deadline)
}

pub(crate) fn collect_c_graph_references_with_deadline(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
    deadline: &dyn DeadlineCheck,
) -> Result<()> {
    collect_c_references_with_options(node, source, references, true, Some(deadline))
}

fn collect_c_references_with_options(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
    suppress_direct_qualified_call_components: bool,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut template_parameters = BTreeSet::new();
    collect_cpp_template_parameter_definitions(node, source, &mut template_parameters, deadline)?;
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
    match deadline {
        Some(deadline) => visit_tree_with_deadline(node, &mut callback, deadline)?,
        None => visit_tree(node, &mut callback),
    }
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
