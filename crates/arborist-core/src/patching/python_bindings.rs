use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::python_patterns::{python_enclosing_case_clause, python_match_capture_names};
use super::python_references::{
    is_python_parameter_symbol_name, is_python_with_target_name, python_enclosing_except_clause,
    python_nearest_scope_node,
};
use super::python_visibility::{python_comprehension_part_index, python_enclosing_comprehension};
use crate::language::{node_text, normalize_path, visit_tree};
use crate::model::{SymbolSummary, SymbolSummaryInit};
use crate::semantic::{
    python_display_byte_range, python_display_header, python_docstring, python_parameters,
    python_return_type, semantic_parent_path, semantic_path,
};

#[derive(Debug, Clone)]
pub(super) struct PythonAccessibleSymbol {
    pub(super) name: String,
    pub(super) summary: SymbolSummary,
    pub(super) rank: usize,
    pub(super) visibility: PythonSymbolVisibility,
}

#[derive(Debug, Clone)]
pub(super) enum PythonSymbolVisibility {
    Always,
    ClassBodyLocal {
        class_range: (usize, usize),
    },
    NamedExpression {
        expression_range: (usize, usize),
        comprehension_range: Option<(usize, usize)>,
        comprehension_part_index: Option<usize>,
    },
    ComprehensionTarget {
        comprehension_range: (usize, usize),
        clause_index: usize,
    },
    ExceptTarget {
        except_clause_range: (usize, usize),
    },
    MatchCapture {
        case_clause_range: (usize, usize),
        match_statement_end: usize,
    },
}

struct PythonTargetCollection<'a> {
    source: &'a str,
    normalized_path: &'a str,
    scope_path: Option<&'a str>,
    origin_type: &'a str,
    node_kind: &'a str,
    rank: usize,
    visibility: PythonSymbolVisibility,
}

pub(super) fn collect_python_scope_symbols(
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

fn collect_python_statement_symbols(
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

pub(super) fn python_symbol_summary(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    origin_type: &str,
) -> Result<Option<SymbolSummary>> {
    let Some(node) = python_symbol_node(node) else {
        return Ok(None);
    };

    let semantic_path = semantic_path(node, source)?;
    let scope_path = semantic_parent_path(&semantic_path);
    let signature = Some(python_display_header(node, source)?);
    let parameters = python_parameters(node, source)?;
    let return_type = python_return_type(node, source)?;
    let docstring = python_docstring(node, source)?;

    Ok(Some(SymbolSummary::new(SymbolSummaryInit {
        symbol_id: semantic_path.clone(),
        semantic_path,
        scope_path,
        file_path: normalized_path.to_string(),
        node_kind: node.kind().to_string(),
        origin_type: origin_type.to_string(),
        byte_range: python_display_byte_range(node),
        signature,
        parameters,
        return_type,
        docstring,
    })))
}

fn python_symbol_node(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "function_definition" | "class_definition" => Some(node),
        "decorated_definition" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| matches!(child.kind(), "function_definition" | "class_definition"))
        }
        _ => None,
    }
}

fn collect_python_parameter_symbols(
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

fn collect_python_target_symbols(
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

fn collect_python_with_target_symbols(
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

fn collect_python_except_target_symbols(
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

fn collect_python_match_target_symbols(
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

fn collect_python_import_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "identifier" {
            return;
        }
        if candidate
            .parent()
            .is_some_and(|parent| parent.kind() == "dotted_name")
        {
            return;
        }

        if let Ok(name) = node_text(candidate, source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.trim().to_string(),
                summary: python_synthetic_symbol_summary(
                    normalized_path,
                    scope_path,
                    name.trim(),
                    "import",
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

fn python_synthetic_symbol_summary(
    normalized_path: &str,
    scope_path: Option<&str>,
    name: &str,
    node_kind: &str,
    origin_type: &str,
    byte_range: (usize, usize),
) -> SymbolSummary {
    let scope_fragment = scope_path.unwrap_or("<module>");
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!("{normalized_path}::python::{scope_fragment}::{node_kind}::{name}"),
        semantic_path: name.to_string(),
        scope_path: scope_path.map(str::to_string),
        file_path: normalized_path.to_string(),
        node_kind: node_kind.to_string(),
        origin_type: origin_type.to_string(),
        byte_range,
        signature: None,
        parameters: Vec::new(),
        return_type: None,
        docstring: None,
    })
}

pub(super) fn collect_python_local_bindings(
    current_path: &Path,
    node: Node<'_>,
    source: &str,
) -> Result<Vec<PythonAccessibleSymbol>> {
    let normalized_path = normalize_path(current_path);
    let scope_path = python_binding_scope_path(node, source)?;
    let origin_type = if node.kind() == "module" {
        "module_scope"
    } else {
        "local_scope"
    };

    let mut symbols = Vec::new();
    if node.kind() == "lambda" {
        if node.child_by_field_name("body").is_none() {
            return Ok(Vec::new());
        }
        collect_python_scope_symbols(node, source, &normalized_path, 0, &mut symbols)?;
        return Ok(symbols);
    }

    let body_node = if node.kind() == "module" {
        node
    } else if let Some(body) = node.child_by_field_name("body") {
        body
    } else {
        return Ok(Vec::new());
    };

    let class_visibility =
        (node.kind() == "class_definition").then_some((node.start_byte(), node.end_byte()));
    let mut cursor = body_node.walk();
    for statement in body_node.named_children(&mut cursor) {
        let mut statement_symbols = Vec::new();
        collect_python_statement_symbols(
            statement,
            source,
            &normalized_path,
            scope_path.as_deref(),
            origin_type,
            0,
            &mut statement_symbols,
        )?;
        if let Some(class_range) = class_visibility {
            for symbol in &mut statement_symbols {
                if matches!(symbol.visibility, PythonSymbolVisibility::Always) {
                    symbol.visibility = PythonSymbolVisibility::ClassBodyLocal { class_range };
                }
            }
        }
        symbols.extend(statement_symbols);
    }

    let external_bindings = collect_python_external_binding_names(body_node, source)?;
    if !external_bindings.is_empty() {
        symbols.retain(|symbol| !external_bindings.contains(&symbol.name));
    }
    Ok(symbols)
}

fn collect_python_external_binding_names(
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

pub(super) fn python_binding_scope_path(
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

pub(super) fn python_scope_declares_external_name(
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
