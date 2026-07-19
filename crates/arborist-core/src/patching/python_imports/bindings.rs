use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::module_path::resolve_local_python_module_path;
use crate::language::node_text;

#[derive(Debug, Clone)]
pub(crate) enum PythonImportBinding {
    Module {
        module_name: String,
    },
    Symbol {
        module_name: Option<String>,
        symbol_name: String,
    },
}

pub(crate) fn collect_visible_python_import_bindings(
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

pub(super) fn python_import_from_binding(
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

pub(super) fn python_imported_symbol_name(imported_name: &str) -> String {
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
