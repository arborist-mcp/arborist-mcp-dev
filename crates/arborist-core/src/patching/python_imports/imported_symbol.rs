use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::super::{PythonImportBinding, python_bindings::python_symbol_summary};
use super::bindings::{python_import_from_binding, python_imported_symbol_name};
use super::module_path::resolve_local_python_module_path;
use crate::language::{node_text, normalize_path, parse_document, read_source};
use crate::model::{LanguageId, SymbolSummary};

pub(crate) fn resolve_local_python_imported_symbol(
    current_path: &Path,
    module_name: &str,
    symbol_name: &str,
) -> Result<Option<SymbolSummary>> {
    let mut visited = BTreeSet::new();
    resolve_local_python_imported_symbol_inner(current_path, module_name, symbol_name, &mut visited)
}

fn resolve_local_python_imported_symbol_inner(
    current_path: &Path,
    module_name: &str,
    symbol_name: &str,
    visited: &mut BTreeSet<String>,
) -> Result<Option<SymbolSummary>> {
    let Some(module_path) = resolve_local_python_module_path(current_path, module_name) else {
        return Ok(None);
    };

    let visit_key = format!("{}::{symbol_name}", normalize_path(&module_path));
    if !visited.insert(visit_key) {
        return Ok(None);
    }

    let module_source = read_source(&module_path)?;
    let document = parse_document(&module_path, &module_source)?;
    if document.language_id != LanguageId::Python {
        return Ok(None);
    }

    let normalized_module_path = normalize_path(&module_path);
    let root = document.tree.root_node();
    let mut cursor = root.walk();
    let children = root.named_children(&mut cursor).collect::<Vec<_>>();

    for child in &children {
        let Some(summary) = python_symbol_summary(
            *child,
            &module_source,
            &normalized_module_path,
            "imported_module",
        )?
        else {
            continue;
        };

        if summary.semantic_path == symbol_name {
            return Ok(Some(summary));
        }
    }

    for child in children {
        let Some(binding) =
            python_reexport_binding_for_name(&module_path, child, &module_source, symbol_name)?
        else {
            continue;
        };

        let PythonImportBinding::Symbol {
            module_name: Some(reexport_module),
            symbol_name: reexported_symbol,
        } = binding
        else {
            continue;
        };

        if let Some(summary) = resolve_local_python_imported_symbol_inner(
            &module_path,
            &reexport_module,
            &reexported_symbol,
            visited,
        )? {
            return Ok(Some(summary));
        }
    }

    Ok(None)
}

fn python_reexport_binding_for_name(
    current_path: &Path,
    statement_node: Node<'_>,
    source: &str,
    symbol_name: &str,
) -> Result<Option<PythonImportBinding>> {
    if statement_node.kind() != "import_from_statement" {
        return Ok(None);
    }

    let mut cursor = statement_node.walk();
    let named_children = statement_node
        .named_children(&mut cursor)
        .collect::<Vec<_>>();
    let Some(module_node) = named_children.first() else {
        return Ok(None);
    };
    let module_name = node_text(*module_node, source)?.trim().to_string();

    for child in named_children.into_iter().skip(1) {
        match child.kind() {
            "aliased_import" => {
                let mut alias_cursor = child.walk();
                let alias_children = child.named_children(&mut alias_cursor).collect::<Vec<_>>();
                if alias_children.len() < 2 {
                    continue;
                }

                let imported_name = node_text(alias_children[0], source)?.trim().to_string();
                let Some(alias_node) = alias_children.last().copied() else {
                    continue;
                };
                let alias_name = node_text(alias_node, source)?.trim().to_string();
                if alias_name == symbol_name {
                    return Ok(Some(python_import_from_binding(
                        current_path,
                        &module_name,
                        &imported_name,
                    )));
                }
            }
            "dotted_name" | "identifier" => {
                let imported_name = node_text(child, source)?.trim().to_string();
                let binding_name = python_imported_symbol_name(&imported_name);
                if binding_name == symbol_name {
                    return Ok(Some(python_import_from_binding(
                        current_path,
                        &module_name,
                        &imported_name,
                    )));
                }
            }
            _ => {}
        }
    }

    Ok(None)
}
