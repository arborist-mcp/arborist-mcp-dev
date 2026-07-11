use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::PythonImportBinding;
use super::python_bindings::python_symbol_summary;
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

pub(crate) fn resolve_local_python_module_path(
    current_path: &Path,
    module_name: &str,
) -> Option<std::path::PathBuf> {
    let parent = current_path.parent()?;
    let (relative_levels, module_parts) = split_python_module_reference(module_name);
    if relative_levels > 0 {
        let mut candidate = parent.to_path_buf();
        for _ in 0..relative_levels.saturating_sub(1) {
            candidate = candidate.parent()?.to_path_buf();
        }
        return resolve_python_module_candidate(candidate, &module_parts);
    }

    let mut search_root = Some(parent);
    while let Some(root) = search_root {
        if let Some(candidate) = resolve_python_module_candidate(root.to_path_buf(), &module_parts)
        {
            return Some(candidate);
        }
        search_root = root.parent();
    }

    None
}

fn split_python_module_reference(module_name: &str) -> (usize, Vec<&str>) {
    let relative_levels = module_name.chars().take_while(|ch| *ch == '.').count();
    let trimmed = module_name.trim_start_matches('.');
    let parts = if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed
            .split('.')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
    };
    (relative_levels, parts)
}

fn resolve_python_module_candidate(
    mut base_dir: std::path::PathBuf,
    module_parts: &[&str],
) -> Option<std::path::PathBuf> {
    for part in module_parts {
        base_dir.push(part);
    }

    let file_candidate = base_dir.with_extension("py");
    if file_candidate.exists() {
        return Some(file_candidate);
    }

    let package_candidate = base_dir.join("__init__.py");
    package_candidate.exists().then_some(package_candidate)
}

pub(super) fn collect_visible_python_import_bindings(
    current_path: &Path,
    node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, PythonImportBinding>> {
    let mut scopes = Vec::new();
    let mut current = Some(node);
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "module" | "function_definition" | "class_definition" | "lambda"
        ) {
            scopes.push(candidate);
        }
        current = candidate.parent();
    }
    scopes.reverse();

    let mut bindings = BTreeMap::new();
    for scope in scopes {
        collect_python_scope_import_bindings(current_path, scope, source, &mut bindings)?;
    }

    Ok(bindings)
}

fn collect_python_scope_import_bindings(
    current_path: &Path,
    scope_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeMap<String, PythonImportBinding>,
) -> Result<()> {
    let body_node = if scope_node.kind() == "module" {
        scope_node
    } else if let Some(body) = scope_node.child_by_field_name("body") {
        body
    } else {
        return Ok(());
    };

    let mut cursor = body_node.walk();
    for child in body_node.named_children(&mut cursor) {
        collect_python_import_bindings_from_statement(current_path, child, source, bindings)?;
    }
    Ok(())
}

fn collect_python_import_bindings_from_statement(
    current_path: &Path,
    statement_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeMap<String, PythonImportBinding>,
) -> Result<()> {
    match statement_node.kind() {
        "import_statement" => {
            collect_python_import_statement_bindings(statement_node, source, bindings)
        }
        "import_from_statement" => collect_python_import_from_statement_bindings(
            current_path,
            statement_node,
            source,
            bindings,
        ),
        "expression_statement" => {
            let mut cursor = statement_node.walk();
            for child in statement_node.named_children(&mut cursor) {
                collect_python_import_bindings_from_statement(
                    current_path,
                    child,
                    source,
                    bindings,
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn collect_python_import_statement_bindings(
    statement_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeMap<String, PythonImportBinding>,
) -> Result<()> {
    let mut cursor = statement_node.walk();
    for child in statement_node.named_children(&mut cursor) {
        match child.kind() {
            "aliased_import" => {
                let mut alias_cursor = child.walk();
                let named_children = child.named_children(&mut alias_cursor).collect::<Vec<_>>();
                if named_children.len() >= 2 {
                    let module_name = node_text(named_children[0], source)?.trim().to_string();
                    let Some(alias_node) = named_children.last().copied() else {
                        continue;
                    };
                    let alias_name = node_text(alias_node, source)?.trim().to_string();
                    bindings.insert(alias_name, PythonImportBinding::Module { module_name });
                }
            }
            "dotted_name" | "identifier" => {
                let module_name = node_text(child, source)?.trim().to_string();
                let binding_name = python_import_statement_binding_name(&module_name);
                bindings.insert(binding_name, PythonImportBinding::Module { module_name });
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_python_import_from_statement_bindings(
    current_path: &Path,
    statement_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeMap<String, PythonImportBinding>,
) -> Result<()> {
    let mut cursor = statement_node.walk();
    let named_children = statement_node
        .named_children(&mut cursor)
        .collect::<Vec<_>>();
    let Some(module_node) = named_children.first() else {
        return Ok(());
    };
    let module_name = node_text(*module_node, source)?.trim().to_string();

    for child in named_children.into_iter().skip(1) {
        match child.kind() {
            "aliased_import" => {
                let mut alias_cursor = child.walk();
                let alias_children = child.named_children(&mut alias_cursor).collect::<Vec<_>>();
                if alias_children.len() >= 2 {
                    let imported_name = node_text(alias_children[0], source)?.trim().to_string();
                    let Some(alias_node) = alias_children.last().copied() else {
                        continue;
                    };
                    let alias_name = node_text(alias_node, source)?.trim().to_string();
                    bindings.insert(
                        alias_name,
                        python_import_from_binding(current_path, &module_name, &imported_name),
                    );
                }
            }
            "dotted_name" | "identifier" => {
                let imported_name = node_text(child, source)?.trim().to_string();
                let binding_name = python_imported_symbol_name(&imported_name);
                bindings.insert(
                    binding_name.clone(),
                    python_import_from_binding(current_path, &module_name, &imported_name),
                );
            }
            _ => {}
        }
    }

    Ok(())
}

fn python_import_from_binding(
    current_path: &Path,
    module_name: &str,
    imported_name: &str,
) -> PythonImportBinding {
    let imported_symbol_name = python_imported_symbol_name(imported_name);
    let module_candidate = join_python_module_reference(module_name, imported_name);
    if resolve_local_python_module_path(current_path, &module_candidate).is_some() {
        PythonImportBinding::Module {
            module_name: module_candidate,
        }
    } else {
        PythonImportBinding::Symbol {
            module_name: Some(module_name.to_string()),
            symbol_name: imported_symbol_name,
        }
    }
}

fn python_import_statement_binding_name(module_name: &str) -> String {
    module_name
        .split('.')
        .next()
        .unwrap_or(module_name)
        .to_string()
}

fn python_imported_symbol_name(imported_name: &str) -> String {
    imported_name
        .rsplit('.')
        .next()
        .unwrap_or(imported_name)
        .to_string()
}

fn join_python_module_reference(module_name: &str, imported_name: &str) -> String {
    if module_name.is_empty() {
        imported_name.to_string()
    } else if module_name.ends_with('.') {
        format!("{module_name}{imported_name}")
    } else {
        format!("{module_name}.{imported_name}")
    }
}
