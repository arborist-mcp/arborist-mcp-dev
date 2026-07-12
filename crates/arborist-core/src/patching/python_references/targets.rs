use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::super::PythonImportBinding;
use super::super::is_python_default_parameter_value;
use super::super::python_bindings::PythonAccessibleSymbol;
use super::super::python_visibility::python_local_binding_visible;
use super::PythonReferenceTarget;
use super::candidates::python_enclosing_local_binding_should_suppress_reference;
use super::filters::should_count_python_reference;
use crate::language::node_text;

pub(super) fn collect_python_reference_targets<'tree>(
    symbol_node: Node<'tree>,
    source: &str,
    bindings: &BTreeMap<String, PythonImportBinding>,
) -> Result<Vec<PythonReferenceTarget<'tree>>> {
    let mut references = Vec::new();
    collect_python_reference_targets_inner(symbol_node, source, bindings, &mut references)?;
    Ok(references)
}

fn collect_python_reference_targets_inner<'tree>(
    node: Node<'tree>,
    source: &str,
    bindings: &BTreeMap<String, PythonImportBinding>,
    references: &mut Vec<PythonReferenceTarget<'tree>>,
) -> Result<()> {
    if node.kind() == "attribute"
        && let (Some(object_node), Some(attribute_node)) = (
            node.child_by_field_name("object"),
            node.child_by_field_name("attribute"),
        )
    {
        if object_node.kind() == "identifier" && attribute_node.kind() == "identifier" {
            let object_name = node_text(object_node, source)?.trim().to_string();
            let attribute_name = node_text(attribute_node, source)?.trim().to_string();
            if let Some(PythonImportBinding::Module { module_name }) = bindings.get(&object_name) {
                let display_name = format!("{object_name}.{attribute_name}");
                references.push(PythonReferenceTarget {
                    name: display_name,
                    node,
                    imported_symbol: Some((module_name.clone(), attribute_name)),
                    import_fallback_name: Some(object_name),
                });
                return Ok(());
            }
        }

        collect_python_reference_targets_inner(object_node, source, bindings, references)?;
        return Ok(());
    }

    if node.kind() == "identifier" && should_count_python_reference(node, source) {
        let name = node_text(node, source)?.trim().to_string();
        let imported_symbol = match bindings.get(&name) {
            Some(PythonImportBinding::Symbol {
                module_name: Some(module_name),
                symbol_name,
            }) => Some((module_name.clone(), symbol_name.clone())),
            _ => None,
        };
        references.push(PythonReferenceTarget {
            name,
            node,
            imported_symbol,
            import_fallback_name: None,
        });
        return Ok(());
    }

    let child_count = node.child_count();
    for index in 0..child_count {
        if let Some(child) = node.child(index) {
            collect_python_reference_targets_inner(child, source, bindings, references)?;
        }
    }

    Ok(())
}

pub(super) fn collect_python_reference_entries(
    current_path: &Path,
    node: Node<'_>,
    source: &str,
    bindings: &BTreeMap<String, PythonImportBinding>,
    local_bindings: &[PythonAccessibleSymbol],
    instance_bindings: &BTreeMap<String, String>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if node.kind() == "attribute"
        && let (Some(object_node), Some(attribute_node)) = (
            node.child_by_field_name("object"),
            node.child_by_field_name("attribute"),
        )
    {
        if object_node.kind() == "identifier" && attribute_node.kind() == "identifier" {
            let object_name = node_text(object_node, source)?.trim().to_string();
            let attribute_name = node_text(attribute_node, source)?.trim().to_string();
            if let Some(PythonImportBinding::Module { module_name }) = bindings.get(&object_name) {
                references.insert(format!("{module_name}.{attribute_name}"));
                return Ok(());
            }
            if let Some(type_name) = instance_bindings.get(&object_name) {
                references.insert(format!("{type_name}.{attribute_name}"));
                return Ok(());
            }
        }

        collect_python_reference_entries(
            current_path,
            object_node,
            source,
            bindings,
            local_bindings,
            instance_bindings,
            references,
        )?;
        return Ok(());
    }

    if node.kind() == "identifier" && should_count_python_reference(node, source) {
        let name = node_text(node, source)?.trim().to_string();
        if let Some(binding) = bindings.get(&name) {
            match binding {
                PythonImportBinding::Module { .. } => {
                    references.insert(name);
                }
                PythonImportBinding::Symbol {
                    module_name,
                    symbol_name,
                } => {
                    if let Some(module_name) = module_name {
                        references.insert(format!("{module_name}.{symbol_name}"));
                    } else {
                        references.insert(symbol_name.clone());
                    }
                }
            }
        } else if (!is_python_default_parameter_value(node)
            && python_local_binding_visible(local_bindings, &name, node))
            || python_enclosing_local_binding_should_suppress_reference(
                current_path,
                node,
                source,
                &name,
            )?
        {
            return Ok(());
        } else {
            references.insert(name);
        }
        return Ok(());
    }

    let child_count = node.child_count();
    for index in 0..child_count {
        if let Some(child) = node.child(index) {
            collect_python_reference_entries(
                current_path,
                child,
                source,
                bindings,
                local_bindings,
                instance_bindings,
                references,
            )?;
        }
    }

    Ok(())
}

pub(super) fn collect_python_instance_type_bindings(
    node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, String>> {
    let mut bindings = BTreeMap::new();
    collect_python_instance_type_bindings_inner(node, source, &mut bindings)?;
    Ok(bindings)
}

fn collect_python_instance_type_bindings_inner(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeMap<String, String>,
) -> Result<()> {
    if node.kind() == "assignment"
        && let (Some(left), Some(right)) = (
            node.child_by_field_name("left"),
            node.child_by_field_name("right"),
        )
        && left.kind() == "identifier"
        && matches!(right.kind(), "call" | "call_expression")
        && let Some(function) = right.child_by_field_name("function")
        && function.kind() == "identifier"
    {
        let variable_name = node_text(left, source)?.trim().to_string();
        let type_name = node_text(function, source)?.trim().to_string();
        if !variable_name.is_empty() && !type_name.is_empty() {
            bindings.insert(variable_name, type_name);
        }
    }

    let child_count = node.child_count();
    for index in 0..child_count {
        if let Some(child) = node.child(index) {
            collect_python_instance_type_bindings_inner(child, source, bindings)?;
        }
    }

    Ok(())
}
