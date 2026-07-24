use super::types::*;
use crate::language::node_text;
use crate::model::{SymbolSummary, SymbolSummaryInit};
use anyhow::Result;
use tree_sitter::Node;

pub(super) fn collect_python_import_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    match node.kind() {
        "import_statement" => collect_python_import_statement_symbols(
            node,
            source,
            normalized_path,
            scope_path,
            origin_type,
            rank,
            symbols,
        )?,
        "import_from_statement" => collect_python_import_from_statement_symbols(
            node,
            source,
            normalized_path,
            scope_path,
            origin_type,
            rank,
            symbols,
        )?,
        _ => {}
    }
    Ok(())
}

fn collect_python_import_statement_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "aliased_import" => {
                let mut alias_cursor = child.walk();
                let children = child.named_children(&mut alias_cursor).collect::<Vec<_>>();
                if let Some(alias_node) = children.last().copied()
                    && children.len() >= 2
                {
                    push_python_import_symbol(
                        alias_node,
                        source,
                        normalized_path,
                        scope_path,
                        origin_type,
                        rank,
                        symbols,
                    )?;
                }
            }
            "dotted_name" | "identifier" => {
                let module_name = node_text(child, source)?.trim();
                let binding_name = module_name.split('.').next().unwrap_or(module_name);
                push_python_named_import_symbol(
                    child,
                    binding_name,
                    normalized_path,
                    scope_path,
                    origin_type,
                    rank,
                    symbols,
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_python_import_from_statement_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    for child in children.into_iter().skip(1) {
        match child.kind() {
            "aliased_import" => {
                let mut alias_cursor = child.walk();
                let alias_children = child.named_children(&mut alias_cursor).collect::<Vec<_>>();
                if let Some(alias_node) = alias_children.last().copied()
                    && alias_children.len() >= 2
                {
                    push_python_import_symbol(
                        alias_node,
                        source,
                        normalized_path,
                        scope_path,
                        origin_type,
                        rank,
                        symbols,
                    )?;
                }
            }
            "dotted_name" | "identifier" => {
                let imported_name = node_text(child, source)?.trim();
                let binding_name = imported_name.rsplit('.').next().unwrap_or(imported_name);
                push_python_named_import_symbol(
                    child,
                    binding_name,
                    normalized_path,
                    scope_path,
                    origin_type,
                    rank,
                    symbols,
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn push_python_import_symbol(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let name = node_text(node, source)?.trim().to_string();
    push_python_named_import_symbol(
        node,
        &name,
        normalized_path,
        scope_path,
        origin_type,
        rank,
        symbols,
    );
    Ok(())
}

fn push_python_named_import_symbol(
    node: Node<'_>,
    name: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) {
    symbols.push(PythonAccessibleSymbol {
        name: name.to_string(),
        summary: python_synthetic_symbol_summary(
            normalized_path,
            scope_path,
            name,
            "import",
            origin_type,
            (node.start_byte(), node.end_byte()),
        ),
        rank: rank + node.start_byte(),
        visibility: PythonSymbolVisibility::Always,
    });
}

pub(super) fn python_synthetic_symbol_summary(
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
