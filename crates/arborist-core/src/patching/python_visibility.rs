use tree_sitter::Node;

use super::is_python_class_header_expression;
use super::is_python_default_parameter_value;
use super::python_bindings::{PythonAccessibleSymbol, PythonSymbolVisibility};
use crate::language::contains_node;

fn python_reference_uses_direct_class_scope(
    reference_node: Node<'_>,
    class_range: (usize, usize),
) -> bool {
    let mut skipped_current_class_scope = false;
    let mut skipped_current_function_scope = false;
    let skip_current_class_scope = is_python_class_header_expression(reference_node);
    let skip_current_function_scope = is_python_default_parameter_value(reference_node);
    let mut current = Some(reference_node);
    while let Some(candidate) = current {
        if skip_current_class_scope
            && !skipped_current_class_scope
            && candidate.kind() == "class_definition"
        {
            skipped_current_class_scope = true;
            current = candidate.parent();
            continue;
        }

        if candidate.kind() == "class_definition"
            && (candidate.start_byte(), candidate.end_byte()) == class_range
        {
            return true;
        }

        if skip_current_function_scope
            && !skipped_current_function_scope
            && matches!(candidate.kind(), "function_definition" | "lambda")
        {
            skipped_current_function_scope = true;
            current = candidate.parent();
            continue;
        }

        if matches!(
            candidate.kind(),
            "function_definition"
                | "lambda"
                | "list_comprehension"
                | "set_comprehension"
                | "dictionary_comprehension"
                | "generator_expression"
                | "class_definition"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

pub(super) fn python_enclosing_comprehension(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "list_comprehension"
                | "set_comprehension"
                | "dictionary_comprehension"
                | "generator_expression"
        ) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn python_comprehension_visible_clause_count(
    comprehension: Node<'_>,
    reference_node: Node<'_>,
) -> Option<usize> {
    if !contains_node(comprehension, reference_node) {
        return None;
    }

    if comprehension
        .child_by_field_name("body")
        .is_some_and(|body| contains_node(body, reference_node))
    {
        let mut total_for_clauses = 0usize;
        let mut cursor = comprehension.walk();
        for child in comprehension.named_children(&mut cursor) {
            if child.kind() == "for_in_clause" {
                total_for_clauses += 1;
            }
        }
        return Some(total_for_clauses);
    }

    let mut completed_for_clauses = 0usize;
    let mut cursor = comprehension.walk();
    for child in comprehension.named_children(&mut cursor) {
        if child.kind() == "for_in_clause" {
            if contains_node(child, reference_node) {
                return Some(completed_for_clauses);
            }
            completed_for_clauses += 1;
            continue;
        }

        if child.kind() == "if_clause" && contains_node(child, reference_node) {
            return Some(completed_for_clauses);
        }
    }

    None
}

pub(super) fn python_comprehension_part_index(
    comprehension: Node<'_>,
    node: Node<'_>,
) -> Option<usize> {
    if !contains_node(comprehension, node) {
        return None;
    }

    let mut part_index = 0usize;
    let mut cursor = comprehension.walk();
    for child in comprehension.named_children(&mut cursor) {
        if child.kind() == "for_in_clause" {
            if child
                .child_by_field_name("right")
                .is_some_and(|right| contains_node(right, node))
            {
                return Some(part_index);
            }
            part_index += 1;
            continue;
        }

        if child.kind() == "if_clause" {
            if contains_node(child, node) {
                return Some(part_index);
            }
            part_index += 1;
            continue;
        }
    }

    comprehension
        .child_by_field_name("body")
        .filter(|body| contains_node(*body, node))
        .map(|_| part_index)
}

pub(super) fn python_accessible_symbol_resolves_at(
    symbol: &PythonAccessibleSymbol,
    reference_node: Node<'_>,
) -> bool {
    match symbol.visibility {
        PythonSymbolVisibility::Always => true,
        PythonSymbolVisibility::ClassBodyLocal { class_range } => {
            python_reference_uses_direct_class_scope(reference_node, class_range)
        }
        PythonSymbolVisibility::NamedExpression {
            expression_range,
            comprehension_range,
            comprehension_part_index,
        } => {
            if let (Some(expected_range), Some(named_part_index)) =
                (comprehension_range, comprehension_part_index)
                && python_enclosing_comprehension(reference_node).is_some_and(|comprehension| {
                    (comprehension.start_byte(), comprehension.end_byte()) == expected_range
                        && python_comprehension_part_index(comprehension, reference_node)
                            .is_some_and(|reference_part_index| {
                                reference_part_index > named_part_index
                                    || (reference_part_index == named_part_index
                                        && reference_node.start_byte() > expression_range.1)
                            })
                })
            {
                return true;
            }

            reference_node.start_byte() > expression_range.1
        }
        PythonSymbolVisibility::ComprehensionTarget {
            comprehension_range,
            clause_index,
        } => python_enclosing_comprehension(reference_node).is_some_and(|comprehension| {
            (comprehension.start_byte(), comprehension.end_byte()) == comprehension_range
                && python_comprehension_visible_clause_count(comprehension, reference_node)
                    .is_some_and(|visible_clause_count| clause_index < visible_clause_count)
        }),
        PythonSymbolVisibility::ExceptTarget {
            except_clause_range,
            ..
        } => {
            let start = reference_node.start_byte();
            let end = reference_node.end_byte();
            start >= except_clause_range.0 && end <= except_clause_range.1
        }
        PythonSymbolVisibility::MatchCapture {
            case_clause_range,
            match_statement_end,
        } => {
            let start = reference_node.start_byte();
            let end = reference_node.end_byte();
            (start >= case_clause_range.0 && end <= case_clause_range.1)
                || start > match_statement_end
        }
    }
}

pub(super) fn python_accessible_symbol_suppresses_at(
    symbol: &PythonAccessibleSymbol,
    reference_node: Node<'_>,
) -> bool {
    match symbol.visibility {
        PythonSymbolVisibility::Always => true,
        PythonSymbolVisibility::ClassBodyLocal { class_range } => {
            python_reference_uses_direct_class_scope(reference_node, class_range)
        }
        PythonSymbolVisibility::NamedExpression { .. } => true,
        PythonSymbolVisibility::ComprehensionTarget { .. } => {
            python_accessible_symbol_resolves_at(symbol, reference_node)
        }
        PythonSymbolVisibility::ExceptTarget {
            except_clause_range: _,
        } => true,
        PythonSymbolVisibility::MatchCapture { .. } => true,
    }
}

pub(super) fn python_local_binding_visible(
    local_bindings: &[PythonAccessibleSymbol],
    name: &str,
    reference_node: Node<'_>,
) -> bool {
    local_bindings.iter().any(|binding| {
        binding.name == name && python_accessible_symbol_suppresses_at(binding, reference_node)
    })
}
