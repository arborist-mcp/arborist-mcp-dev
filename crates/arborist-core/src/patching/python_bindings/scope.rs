use super::super::python_visibility::{
    python_comprehension_part_index, python_enclosing_comprehension,
};
use super::imports::*;
use super::path::*;
use super::summary::*;
use super::targets::*;
use super::types::*;
use crate::language::{node_text, visit_tree};
use anyhow::Result;
use tree_sitter::Node;

pub(in super::super) fn collect_python_scope_symbols(
    scope_node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let scope_path = python_binding_scope_path(scope_node, source)?;
    let origin_type = if scope_node.kind() == "module" {
        "module_scope"
    } else {
        "local_scope"
    };

    if matches!(scope_node.kind(), "function_definition" | "lambda") {
        collect_python_parameter_symbols(
            scope_node,
            source,
            normalized_path,
            scope_path.as_deref(),
            origin_type,
            scope_rank + 500_000,
            symbols,
        )?;
    }

    if scope_node.kind() == "lambda" {
        let Some(body_node) = scope_node.child_by_field_name("body") else {
            return Ok(());
        };
        collect_python_statement_symbols(
            body_node,
            source,
            normalized_path,
            scope_path.as_deref(),
            origin_type,
            scope_rank,
            symbols,
        )?;
        return Ok(());
    }

    let class_visibility = (scope_node.kind() == "class_definition")
        .then_some((scope_node.start_byte(), scope_node.end_byte()));
    let body_node = if scope_node.kind() == "module" {
        scope_node
    } else if let Some(body) = scope_node.child_by_field_name("body") {
        body
    } else {
        return Ok(());
    };

    let external_bindings = collect_python_external_binding_names(body_node, source)?;
    let mut cursor = body_node.walk();
    for child in body_node.named_children(&mut cursor) {
        let mut statement_symbols = Vec::new();
        collect_python_statement_symbols(
            child,
            source,
            normalized_path,
            scope_path.as_deref(),
            origin_type,
            scope_rank,
            &mut statement_symbols,
        )?;
        if let Some(class_range) = class_visibility {
            for symbol in &mut statement_symbols {
                if matches!(symbol.visibility, PythonSymbolVisibility::Always) {
                    symbol.visibility = PythonSymbolVisibility::ClassBodyLocal { class_range };
                }
            }
        }
        if scope_node.kind() != "module" && !external_bindings.is_empty() {
            statement_symbols.retain(|symbol| !external_bindings.contains(&symbol.name));
        }
        symbols.extend(statement_symbols);
    }

    Ok(())
}

pub(super) fn collect_python_statement_symbols(
    statement_node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    scope_rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    collect_python_named_expression_symbols(
        statement_node,
        source,
        normalized_path,
        scope_path,
        origin_type,
        scope_rank + 350_000 + statement_node.start_byte(),
        symbols,
    )?;
    collect_python_comprehension_target_symbols(
        statement_node,
        source,
        normalized_path,
        scope_path,
        origin_type,
        scope_rank + 325_000 + statement_node.start_byte(),
        symbols,
    )?;

    match statement_node.kind() {
        "function_definition" | "class_definition" | "decorated_definition" => {
            if let Some(summary) =
                python_symbol_summary(statement_node, source, normalized_path, origin_type)?
            {
                symbols.push(PythonAccessibleSymbol {
                    name: summary
                        .semantic_path
                        .rsplit('.')
                        .next()
                        .unwrap_or(&summary.semantic_path)
                        .to_string(),
                    summary,
                    rank: scope_rank + 400_000 + statement_node.start_byte(),
                    visibility: PythonSymbolVisibility::Always,
                });
            }
        }
        "assignment" | "augmented_assignment" => {
            if let Some(left) = statement_node.child_by_field_name("left") {
                collect_python_target_symbols(
                    left,
                    PythonTargetCollection {
                        source,
                        normalized_path,
                        scope_path,
                        origin_type,
                        node_kind: "assignment",
                        rank: scope_rank + 300_000 + statement_node.start_byte(),
                        visibility: PythonSymbolVisibility::Always,
                    },
                    symbols,
                )?;
            }
        }
        "for_statement" => {
            if let Some(left) = statement_node.child_by_field_name("left") {
                collect_python_target_symbols(
                    left,
                    PythonTargetCollection {
                        source,
                        normalized_path,
                        scope_path,
                        origin_type,
                        node_kind: "for_target",
                        rank: scope_rank + 300_000 + statement_node.start_byte(),
                        visibility: PythonSymbolVisibility::Always,
                    },
                    symbols,
                )?;
            }
            collect_python_child_block_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
        "with_statement" => {
            collect_python_with_target_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank + 300_000 + statement_node.start_byte(),
                symbols,
            )?;
            collect_python_child_block_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
        "try_statement" => {
            collect_python_except_target_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank + 300_000 + statement_node.start_byte(),
                symbols,
            )?;
            collect_python_child_block_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
        "match_statement" => {
            collect_python_match_target_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank + 300_000 + statement_node.start_byte(),
                symbols,
            )?;
            collect_python_child_block_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
        "if_statement" | "while_statement" => {
            collect_python_child_block_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
        "import_statement" | "import_from_statement" => {
            collect_python_import_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank + 300_000 + statement_node.start_byte(),
                symbols,
            )?;
        }
        "expression_statement" => {
            let mut cursor = statement_node.walk();
            for child in statement_node.named_children(&mut cursor) {
                collect_python_statement_symbols(
                    child,
                    source,
                    normalized_path,
                    scope_path,
                    origin_type,
                    scope_rank,
                    symbols,
                )?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn collect_python_comprehension_target_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if !matches!(
            candidate.kind(),
            "list_comprehension"
                | "set_comprehension"
                | "dictionary_comprehension"
                | "generator_expression"
        ) {
            return;
        }

        let comprehension_range = (candidate.start_byte(), candidate.end_byte());
        let mut clause_index = 0usize;
        let mut cursor = candidate.walk();
        for child in candidate.named_children(&mut cursor) {
            if child.kind() != "for_in_clause" {
                continue;
            }
            let Some(left) = child.child_by_field_name("left") else {
                clause_index += 1;
                continue;
            };
            collect_python_target_symbols(
                left,
                PythonTargetCollection {
                    source,
                    normalized_path,
                    scope_path,
                    origin_type,
                    node_kind: "comprehension_target",
                    rank: rank + child.start_byte(),
                    visibility: PythonSymbolVisibility::ComprehensionTarget {
                        comprehension_range,
                        clause_index,
                    },
                },
                symbols,
            )
            .ok();
            clause_index += 1;
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn collect_python_child_block_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    scope_rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "block" {
            continue;
        }

        let mut block_cursor = child.walk();
        for statement in child.named_children(&mut block_cursor) {
            collect_python_statement_symbols(
                statement,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
    }

    Ok(())
}

fn collect_python_named_expression_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "named_expression" {
            return;
        }
        let Some(left) = candidate.child_by_field_name("name") else {
            return;
        };
        let mut target_callback = |target: Node<'_>| {
            if target.kind() != "identifier" {
                return;
            }
            if let Ok(name) = node_text(target, source) {
                symbols.push(PythonAccessibleSymbol {
                    name: name.trim().to_string(),
                    summary: python_synthetic_symbol_summary(
                        normalized_path,
                        scope_path,
                        name.trim(),
                        "named_expression",
                        origin_type,
                        (target.start_byte(), target.end_byte()),
                    ),
                    rank: rank + target.start_byte(),
                    visibility: PythonSymbolVisibility::NamedExpression {
                        expression_range: (candidate.start_byte(), candidate.end_byte()),
                        comprehension_range: python_enclosing_comprehension(candidate).map(
                            |comprehension| (comprehension.start_byte(), comprehension.end_byte()),
                        ),
                        comprehension_part_index: python_enclosing_comprehension(candidate)
                            .and_then(|comprehension| {
                                python_comprehension_part_index(comprehension, candidate)
                            }),
                    },
                });
            }
        };
        visit_tree(left, &mut target_callback);
    };
    visit_tree(node, &mut callback);
    Ok(())
}
