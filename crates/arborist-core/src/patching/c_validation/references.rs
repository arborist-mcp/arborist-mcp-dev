use std::collections::BTreeSet;

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
    let mut template_parameters = BTreeSet::new();
    collect_cpp_template_parameter_definitions(node, source, &mut template_parameters)?;
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() == "identifier" && !is_c_enumerator_name(candidate) {
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

fn is_qualified_identifier_component(node: Node<'_>) -> bool {
    node.parent()
        .is_some_and(|parent| parent.kind() == "qualified_identifier")
}

fn is_c_enumerator_name(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        parent.kind() == "enumerator"
            && parent
                .child_by_field_name("name")
                .is_some_and(|name| name == node)
    })
}
