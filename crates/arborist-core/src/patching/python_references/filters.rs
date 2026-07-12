use tree_sitter::Node;

use super::super::python_patterns::{
    is_python_as_pattern_alias, is_python_match_capture_name, is_python_match_keyword_name,
};
use crate::language::{contains_node, is_field_node};

pub(super) fn should_count_python_reference(node: Node<'_>, source: &str) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    if is_field_node(parent, "name", node)
        && matches!(
            parent.kind(),
            "function_definition" | "class_definition" | "keyword_argument"
        )
    {
        return false;
    }

    if is_field_node(parent, "attribute", node) && parent.kind() == "attribute" {
        return false;
    }

    if is_python_with_target_name(node, source) {
        return false;
    }

    if is_python_except_target_name(node, source) {
        return false;
    }

    if is_python_match_capture_name(node, source) {
        return false;
    }

    if is_python_match_keyword_name(node) {
        return false;
    }

    if matches!(parent.kind(), "import_statement" | "import_from_statement") {
        return false;
    }

    if has_python_type_annotation_ancestor(node) {
        return false;
    }

    if is_python_parameter_name(node) {
        return false;
    }

    if is_python_parameter_declaration_node(node) {
        return false;
    }

    if is_python_named_expression_target(node) {
        return false;
    }

    if matches!(
        parent.kind(),
        "list_splat_pattern" | "dictionary_splat_pattern" | "tuple_pattern"
    ) {
        return false;
    }

    if let Some(left) = parent.child_by_field_name("left")
        && matches!(
            parent.kind(),
            "assignment" | "augmented_assignment" | "for_statement" | "for_in_clause"
        )
        && contains_node(left, node)
    {
        return false;
    }

    true
}

fn has_python_type_annotation_ancestor(node: Node<'_>) -> bool {
    let mut current = node.parent();

    while let Some(candidate) = current {
        if candidate.kind() == "type" {
            return true;
        }

        if matches!(
            candidate.kind(),
            "function_definition" | "class_definition" | "module"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

fn is_python_parameter_name(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if !PYTHON_PARAMETER_KINDS.contains(&parent.kind()) {
        return false;
    }

    parent
        .child_by_field_name("name")
        .is_some_and(|candidate| candidate.id() == node.id())
}

pub(in crate::patching) fn is_python_parameter_symbol_name(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if !PYTHON_PARAMETER_KINDS.contains(&parent.kind()) {
        return false;
    }

    parent
        .child_by_field_name("value")
        .is_none_or(|value| !contains_node(value, node))
        && !has_python_type_annotation_ancestor(node)
}

fn is_python_parameter_declaration_node(node: Node<'_>) -> bool {
    let mut current = node.parent();

    while let Some(candidate) = current {
        if candidate.kind() == "default_parameter" || candidate.kind() == "typed_default_parameter"
        {
            if let Some(value) = candidate.child_by_field_name("value") {
                return !contains_node(value, node);
            }
            return true;
        }

        if matches!(candidate.kind(), "parameters" | "lambda_parameters") {
            return true;
        }

        if matches!(
            candidate.kind(),
            "function_definition" | "class_definition" | "module"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

fn is_python_named_expression_target(node: Node<'_>) -> bool {
    let mut current = node.parent();

    while let Some(candidate) = current {
        if candidate.kind() == "named_expression" {
            return candidate
                .child_by_field_name("name")
                .is_some_and(|left| contains_node(left, node));
        }

        if matches!(
            candidate.kind(),
            "function_definition" | "class_definition" | "module"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

pub(in crate::patching) fn is_python_with_target_name(node: Node<'_>, source: &str) -> bool {
    let mut current = node.parent();

    while let Some(candidate) = current {
        if matches!(candidate.kind(), "as_pattern" | "with_item") {
            if let Some(alias) = candidate.child_by_field_name("alias") {
                return contains_node(alias, node);
            }
            if source
                .get(candidate.start_byte()..node.start_byte())
                .is_some_and(|prefix| prefix.contains(" as "))
            {
                return true;
            }
        }

        if matches!(
            candidate.kind(),
            "with_statement" | "function_definition" | "class_definition" | "module"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

fn is_python_except_target_name(node: Node<'_>, source: &str) -> bool {
    let mut current = node.parent();

    while let Some(candidate) = current {
        if candidate.kind() == "except_clause" {
            if node.kind() == "as_pattern_target" {
                return true;
            }
            return is_python_as_pattern_alias(node, candidate, source);
        }

        if matches!(
            candidate.kind(),
            "try_statement" | "function_definition" | "class_definition" | "module"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

pub(in crate::patching) fn python_enclosing_except_clause(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "except_clause" {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

pub(in crate::patching) fn python_nearest_scope_node(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "function_definition" | "class_definition" | "module" | "lambda"
        ) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

const PYTHON_PARAMETER_KINDS: &[&str] = &[
    "parameters",
    "lambda_parameters",
    "typed_parameter",
    "default_parameter",
    "typed_default_parameter",
];
