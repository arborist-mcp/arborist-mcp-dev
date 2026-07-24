use crate::language::node_text;
use crate::semantic::semantic_path;
use anyhow::Result;
use std::collections::BTreeSet;
use tree_sitter::Node;

pub(super) fn collect_python_external_binding_names(
    body_node: Node<'_>,
    source: &str,
) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    collect_python_external_binding_names_in_scope(body_node, source, &mut names)?;
    Ok(names)
}

fn collect_python_external_binding_names_in_scope(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    if matches!(
        node.kind(),
        "function_definition" | "class_definition" | "lambda"
    ) {
        return Ok(());
    }

    if matches!(node.kind(), "global_statement" | "nonlocal_statement") {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "identifier" {
                continue;
            }
            if let Ok(name) = node_text(child, source) {
                names.insert(name.trim().to_string());
            }
        }
        return Ok(());
    }

    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            collect_python_external_binding_names_in_scope(child, source, names)?;
        }
    }

    Ok(())
}

pub(in super::super) fn python_binding_scope_path(
    scope_node: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    if scope_node.kind() == "module" {
        return Ok(None);
    }

    if matches!(
        scope_node.kind(),
        "function_definition" | "class_definition"
    ) {
        return Ok(Some(semantic_path(scope_node, source)?));
    }

    if scope_node.kind() == "lambda" {
        let mut current = scope_node.parent();
        while let Some(candidate) = current {
            if candidate.kind() == "module" {
                return Ok(None);
            }
            if matches!(candidate.kind(), "function_definition" | "class_definition") {
                return Ok(Some(semantic_path(candidate, source)?));
            }
            current = candidate.parent();
        }
    }

    Ok(None)
}

pub(in super::super) fn python_scope_declares_external_name(
    scope_node: Node<'_>,
    source: &str,
    name: &str,
    statement_kind: &str,
) -> bool {
    let body_node = if scope_node.kind() == "module" {
        scope_node
    } else if let Some(body) = scope_node.child_by_field_name("body") {
        body
    } else {
        return false;
    };

    python_scope_declares_external_name_in_scope(body_node, source, name, statement_kind)
}

fn python_scope_declares_external_name_in_scope(
    node: Node<'_>,
    source: &str,
    name: &str,
    statement_kind: &str,
) -> bool {
    if matches!(
        node.kind(),
        "function_definition" | "class_definition" | "lambda"
    ) {
        return false;
    }

    if node.kind() == statement_kind {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "identifier" {
                continue;
            }
            if node_text(child, source)
                .ok()
                .is_some_and(|text| text.trim() == name)
            {
                return true;
            }
        }
        return false;
    }

    for index in 0..node.child_count() {
        if let Some(child) = node.child(index)
            && python_scope_declares_external_name_in_scope(child, source, name, statement_kind)
        {
            return true;
        }
    }

    false
}
