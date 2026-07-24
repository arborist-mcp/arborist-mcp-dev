use super::super::python_patterns::{python_enclosing_case_clause, python_match_capture_names};
use super::super::python_references::{
    is_python_parameter_symbol_name, is_python_with_target_name, python_enclosing_except_clause,
    python_nearest_scope_node,
};
use super::imports::*;
use super::types::*;
use crate::language::{node_text, visit_tree};
use anyhow::Result;
use tree_sitter::Node;

pub(super) struct PythonTargetCollection<'a> {
    pub(super) source: &'a str,
    pub(super) normalized_path: &'a str,
    pub(super) scope_path: Option<&'a str>,
    pub(super) origin_type: &'a str,
    pub(super) node_kind: &'a str,
    pub(super) rank: usize,
    pub(super) visibility: PythonSymbolVisibility,
}

pub(super) fn collect_python_parameter_symbols(
    function_node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let Some(parameters_node) = function_node.child_by_field_name("parameters") else {
        return Ok(());
    };

    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "identifier" || !is_python_parameter_symbol_name(candidate) {
            return;
        }

        if let Ok(name) = node_text(candidate, source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.trim().to_string(),
                summary: python_synthetic_symbol_summary(
                    normalized_path,
                    scope_path,
                    name.trim(),
                    "parameter",
                    origin_type,
                    (candidate.start_byte(), candidate.end_byte()),
                ),
                rank: rank + candidate.start_byte(),
                visibility: PythonSymbolVisibility::Always,
            });
        }
    };
    visit_tree(parameters_node, &mut callback);
    Ok(())
}

pub(super) fn collect_python_target_symbols(
    node: Node<'_>,
    context: PythonTargetCollection<'_>,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "identifier" {
            return;
        }

        if let Ok(name) = node_text(candidate, context.source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.trim().to_string(),
                summary: python_synthetic_symbol_summary(
                    context.normalized_path,
                    context.scope_path,
                    name.trim(),
                    context.node_kind,
                    context.origin_type,
                    (candidate.start_byte(), candidate.end_byte()),
                ),
                rank: context.rank + candidate.start_byte(),
                visibility: context.visibility.clone(),
            });
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

pub(super) fn collect_python_with_target_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "identifier" || !is_python_with_target_name(candidate, source) {
            return;
        }

        if let Ok(name) = node_text(candidate, source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.trim().to_string(),
                summary: python_synthetic_symbol_summary(
                    normalized_path,
                    scope_path,
                    name.trim(),
                    "with_target",
                    origin_type,
                    (candidate.start_byte(), candidate.end_byte()),
                ),
                rank: rank + candidate.start_byte(),
                visibility: PythonSymbolVisibility::Always,
            });
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

pub(super) fn collect_python_except_target_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "as_pattern_target" {
            return;
        }

        let Some(except_clause) = python_enclosing_except_clause(candidate) else {
            return;
        };
        let Some(_scope_node) = python_nearest_scope_node(candidate) else {
            return;
        };

        if let Ok(name) = node_text(candidate, source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.trim().to_string(),
                summary: python_synthetic_symbol_summary(
                    normalized_path,
                    scope_path,
                    name.trim(),
                    "except_target",
                    origin_type,
                    (candidate.start_byte(), candidate.end_byte()),
                ),
                rank: rank + candidate.start_byte(),
                visibility: PythonSymbolVisibility::ExceptTarget {
                    except_clause_range: (except_clause.start_byte(), except_clause.end_byte()),
                },
            });
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

pub(super) fn collect_python_match_target_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "case_pattern" {
            return;
        }

        let Some(case_clause) = python_enclosing_case_clause(candidate) else {
            return;
        };

        for name in python_match_capture_names(candidate, source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.clone(),
                summary: python_synthetic_symbol_summary(
                    normalized_path,
                    scope_path,
                    &name,
                    "match_capture",
                    origin_type,
                    (candidate.start_byte(), candidate.end_byte()),
                ),
                rank: rank + candidate.start_byte(),
                visibility: PythonSymbolVisibility::MatchCapture {
                    case_clause_range: (case_clause.start_byte(), case_clause.end_byte()),
                    match_statement_end: node.end_byte(),
                },
            });
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}
