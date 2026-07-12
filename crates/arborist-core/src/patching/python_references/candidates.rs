use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::super::python_bindings::{
    PythonAccessibleSymbol, PythonSymbolVisibility, collect_python_scope_symbols,
    python_scope_declares_external_name,
};
use super::super::python_visibility::{
    python_accessible_symbol_resolves_at, python_accessible_symbol_suppresses_at,
};
use super::super::{
    is_python_class_header_expression, is_python_default_parameter_value,
    resolve_local_python_imported_symbol,
};
use super::{PythonReferenceTarget, python_nearest_scope_node};
use crate::language::normalize_path;

pub(super) fn python_binding_candidates_for_reference(
    path: &Path,
    source: &str,
    normalized_path: &str,
    reference_target: &PythonReferenceTarget<'_>,
) -> Result<Vec<PythonAccessibleSymbol>> {
    if let Some((module_name, symbol_name)) = &reference_target.imported_symbol
        && let Some(summary) = resolve_local_python_imported_symbol(path, module_name, symbol_name)?
    {
        return Ok(vec![PythonAccessibleSymbol {
            name: reference_target.name.clone(),
            summary,
            rank: 4_000_000,
            visibility: PythonSymbolVisibility::Always,
        }]);
    }

    if let Some(fallback_name) = &reference_target.import_fallback_name {
        let fallback = PythonReferenceTarget {
            name: fallback_name.clone(),
            node: reference_target.node,
            imported_symbol: None,
            import_fallback_name: None,
        };
        let fallback_candidates =
            python_binding_candidates_for_reference(path, source, normalized_path, &fallback)?;
        if !fallback_candidates.is_empty() {
            return Ok(fallback_candidates);
        }
    }

    let name = if let Some((_, symbol_name)) = &reference_target.imported_symbol {
        symbol_name.clone()
    } else {
        reference_target.name.clone()
    };
    let force_module_scope =
        python_reference_is_global_declared(reference_target.node, source, &name);
    let mut candidates = Vec::new();
    let mut seen_function_scope = false;
    let mut skipped_current_class_scope = false;
    let mut skipped_current_function_scope = false;
    let mut scope_rank = 3_000_000usize;
    let mut current = Some(reference_target.node);
    let skip_current_function_scope = is_python_default_parameter_value(reference_target.node);
    let skip_current_class_scope = is_python_class_header_expression(reference_target.node);

    while let Some(node) = current {
        let include_scope = if force_module_scope {
            node.kind() == "module"
        } else {
            match node.kind() {
                "function_definition" | "lambda" => {
                    if skip_current_function_scope && !skipped_current_function_scope {
                        skipped_current_function_scope = true;
                        false
                    } else {
                        seen_function_scope = true;
                        true
                    }
                }
                "list_comprehension"
                | "set_comprehension"
                | "dictionary_comprehension"
                | "generator_expression" => {
                    seen_function_scope = true;
                    false
                }
                "class_definition" => {
                    if skip_current_class_scope && !skipped_current_class_scope {
                        skipped_current_class_scope = true;
                        false
                    } else {
                        !seen_function_scope
                    }
                }
                "module" => true,
                _ => false,
            }
        };

        if include_scope {
            collect_python_scope_symbols(
                node,
                source,
                normalized_path,
                scope_rank,
                &mut candidates,
            )?;
            scope_rank = scope_rank.saturating_sub(1_000_000);
        }

        current = node.parent();
    }

    candidates.retain(|candidate| candidate.name == name);
    let mut resolving_candidates = candidates
        .iter()
        .filter(|candidate| python_accessible_symbol_resolves_at(candidate, reference_target.node))
        .cloned()
        .collect::<Vec<_>>();
    let mut suppressing_candidates = candidates
        .iter()
        .filter(|candidate| {
            python_accessible_symbol_suppresses_at(candidate, reference_target.node)
        })
        .cloned()
        .collect::<Vec<_>>();

    resolving_candidates.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| left.summary.symbol_id.cmp(&right.summary.symbol_id))
    });

    suppressing_candidates.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| left.summary.symbol_id.cmp(&right.summary.symbol_id))
    });

    let best_suppressing_rank = suppressing_candidates
        .first()
        .map(|candidate| candidate.rank);
    let Some(best_rank) = resolving_candidates.first().map(|candidate| candidate.rank) else {
        return Ok(Vec::new());
    };

    if best_suppressing_rank.is_some_and(|rank| rank > best_rank) {
        return Ok(Vec::new());
    }

    Ok(resolving_candidates
        .into_iter()
        .filter(|candidate| candidate.rank == best_rank)
        .collect())
}

pub(super) fn python_enclosing_local_binding_should_suppress_reference(
    current_path: &Path,
    reference_node: Node<'_>,
    source: &str,
    name: &str,
) -> Result<bool> {
    if python_reference_is_global_declared(reference_node, source, name) {
        return Ok(false);
    }

    let normalized_path = normalize_path(current_path);
    let mut candidates = Vec::new();
    let mut seen_scope = false;
    let include_immediate_scope = is_python_decorator_expression(reference_node);
    let mut scope_rank = 2_000_000usize;
    let mut current = reference_node.parent();

    while let Some(node) = current {
        let include_scope = match node.kind() {
            "lambda" => {
                seen_scope = true;
                true
            }
            "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension"
            | "generator_expression" => {
                seen_scope = true;
                false
            }
            "function_definition" | "class_definition" | "module" => {
                if seen_scope {
                    true
                } else {
                    seen_scope = true;
                    include_immediate_scope
                }
            }
            _ => false,
        };

        if include_scope {
            collect_python_scope_symbols(
                node,
                source,
                &normalized_path,
                scope_rank,
                &mut candidates,
            )?;
            scope_rank = scope_rank.saturating_sub(1_000_000);
        }

        current = node.parent();
    }

    candidates.retain(|candidate| {
        candidate.name == name && python_accessible_symbol_suppresses_at(candidate, reference_node)
    });
    candidates.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| left.summary.symbol_id.cmp(&right.summary.symbol_id))
    });

    let Some(best) = candidates.first() else {
        return Ok(false);
    };

    Ok(!matches!(
        best.summary.node_kind.as_str(),
        "function_definition" | "class_definition"
    ))
}

fn is_python_decorator_expression(node: Node<'_>) -> bool {
    let mut current = Some(node);

    while let Some(candidate) = current {
        if candidate.kind() == "decorator" {
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

fn python_reference_is_global_declared(node: Node<'_>, source: &str, name: &str) -> bool {
    python_nearest_scope_node(node).is_some_and(|scope| {
        python_scope_declares_external_name(scope, source, name, "global_statement")
    })
}
