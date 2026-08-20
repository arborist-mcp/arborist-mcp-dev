mod scope;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{ReferenceValidation, resolved_binding_decision, unresolved_binding_decision};
use crate::deadline::DeadlineCheck;
use crate::language::{
    ParsedDocument, go_local_import_binding_statuses, node_text, normalize_path,
};
use crate::model::{SymbolSummary, SymbolSummaryInit, ValidationBinding};
use crate::semantic::go::{
    go_parameters, go_return_type, go_semantic_path, go_signature, go_symbol_name,
};

use scope::GoBinding;
use scope::scan_go_symbol_scope;

pub(crate) fn collect_go_reference_validation_with_deadline(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<ReferenceValidation> {
    let normalized_path = normalize_path(path);
    let scope_scan = scan_go_symbol_scope(symbol_node, source, deadline)?;
    let mut file_items = BTreeMap::new();
    let mut ambiguous_file_item_names = BTreeSet::new();
    collect_go_file_items(
        document.tree.root_node(),
        source,
        &mut file_items,
        &mut ambiguous_file_item_names,
        deadline,
    )?;
    let import_statuses =
        go_local_import_binding_statuses(path, document.tree.root_node(), source, deadline)?;
    let scope_path = go_symbol_scope_path(symbol_node, source)?;

    let mut validation = ReferenceValidation::default();
    for name in &scope_scan.local_references {
        let Some(binding) = scope_scan
            .local_bindings
            .iter()
            .find(|binding| &binding.name == name)
        else {
            continue;
        };
        let summary = go_local_symbol_summary(&normalized_path, &scope_path, binding);
        validation
            .binding_decisions
            .push(resolved_binding_decision(name, &summary));
        validation.resolved_identifiers.push(ValidationBinding {
            name: name.clone(),
            symbol: summary,
        });
    }
    for name in &scope_scan.external_references {
        if let Some(deadline) = deadline {
            deadline.check("validating Go references")?;
        }
        if GO_PREDECLARED_NAMES.contains(&name.as_str()) {
            continue;
        }
        if ambiguous_file_item_names.contains(name)
            || (import_statuses.local_names.contains(name)
                && !import_statuses.resolved_names.contains(name))
        {
            validation
                .binding_decisions
                .push(unresolved_binding_decision(name));
            validation.unresolved_identifiers.push(name.clone());
            continue;
        }
        if let Some(item) = file_items.get(name) {
            let summary = go_item_symbol_summary(&normalized_path, source, item);
            validation
                .binding_decisions
                .push(resolved_binding_decision(name, &summary));
            validation.resolved_identifiers.push(ValidationBinding {
                name: name.clone(),
                symbol: summary,
            });
        } else if import_statuses.resolved_names.contains(name) {
            let summary = go_resolved_local_import_symbol_summary(
                &normalized_path,
                name,
                import_statuses
                    .resolved_ranges
                    .get(name)
                    .copied()
                    .unwrap_or((0, 0)),
            );
            validation
                .binding_decisions
                .push(resolved_binding_decision(name, &summary));
            validation.resolved_identifiers.push(ValidationBinding {
                name: name.clone(),
                symbol: summary,
            });
        } else {
            validation
                .binding_decisions
                .push(unresolved_binding_decision(name));
            validation.unresolved_identifiers.push(name.clone());
        }
    }
    Ok(validation)
}

fn go_symbol_scope_path(symbol_node: Node<'_>, source: &str) -> Result<Option<String>> {
    match go_symbol_name(symbol_node, source)? {
        Some(name) => go_semantic_path(symbol_node, source, &name),
        None => Ok(None),
    }
}

fn go_resolved_local_import_symbol_summary(
    normalized_path: &str,
    name: &str,
    byte_range: (usize, usize),
) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!("{normalized_path}::go::<module>::import_spec::{name}"),
        semantic_path: name.to_string(),
        scope_path: None,
        file_path: normalized_path.to_string(),
        node_kind: "import_spec".to_string(),
        origin_type: "imported_module".to_string(),
        byte_range,
        signature: None,
        parameters: Vec::new(),
        return_type: None,
        docstring: None,
    })
}

fn go_local_symbol_summary(
    normalized_path: &str,
    scope_path: &Option<String>,
    binding: &GoBinding,
) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::go::{}::{}::{}",
            normalized_path,
            scope_path.as_deref().unwrap_or("<symbol>"),
            binding.node_kind,
            binding.name
        ),
        semantic_path: binding.name.clone(),
        scope_path: scope_path.clone(),
        file_path: normalized_path.to_string(),
        node_kind: binding.node_kind.to_string(),
        origin_type: "local_scope".to_string(),
        byte_range: (binding.start_byte, binding.end_byte),
        signature: None,
        parameters: Vec::new(),
        return_type: None,
        docstring: None,
    })
}

fn go_item_symbol_summary(
    normalized_path: &str,
    source: &str,
    item: &GoFileItem<'_>,
) -> SymbolSummary {
    let semantic_path = item.name.clone();
    let is_function = item.node.kind() == "function_declaration";
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::go::<module>::{}::{}",
            normalized_path, item.node_kind, semantic_path
        ),
        semantic_path,
        scope_path: None,
        file_path: normalized_path.to_string(),
        node_kind: item.node_kind.to_string(),
        origin_type: item.origin_type.to_string(),
        byte_range: (item.node.start_byte(), item.node.end_byte()),
        signature: if is_function {
            go_signature(item.node, source)
        } else {
            None
        },
        parameters: if is_function {
            go_parameters(item.node, source)
        } else {
            Vec::new()
        },
        return_type: if is_function {
            go_return_type(item.node, source)
        } else {
            None
        },
        docstring: None,
    })
}

struct GoFileItem<'tree> {
    name: String,
    node_kind: &'static str,
    node: Node<'tree>,
    origin_type: &'static str,
}

fn collect_go_file_items<'tree>(
    root: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, GoFileItem<'tree>>,
    ambiguous_names: &mut BTreeSet<String>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if let Some(deadline) = deadline {
            deadline.check("scanning Go file items")?;
        }
        match child.kind() {
            "function_declaration" => insert_go_declaration_item(
                child,
                source,
                "function_declaration",
                items,
                ambiguous_names,
            )?,
            "type_declaration" => collect_go_type_names(child, source, items, ambiguous_names)?,
            "import_declaration" => collect_go_import_names(child, source, items, ambiguous_names)?,
            "var_declaration" | "const_declaration" => {
                collect_go_spec_names(child, source, items, ambiguous_names)?
            }
            _ => {}
        }
    }
    Ok(())
}

fn insert_go_declaration_item<'tree>(
    node: Node<'tree>,
    source: &str,
    node_kind: &'static str,
    items: &mut BTreeMap<String, GoFileItem<'tree>>,
    ambiguous_names: &mut BTreeSet<String>,
) -> Result<()> {
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(());
    };
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() {
        return Ok(());
    }
    insert_go_file_item(
        items,
        ambiguous_names,
        GoFileItem {
            name,
            node_kind,
            node,
            origin_type: "module_scope",
        },
    );
    Ok(())
}

fn collect_go_spec_names<'tree>(
    node: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, GoFileItem<'tree>>,
    ambiguous_names: &mut BTreeSet<String>,
) -> Result<()> {
    if matches!(node.kind(), "var_spec" | "const_spec") {
        let mut cursor = node.walk();
        for name in node.children_by_field_name("name", &mut cursor) {
            let name = node_text(name, source)?.trim().to_string();
            if !name.is_empty() {
                insert_go_file_item(
                    items,
                    ambiguous_names,
                    GoFileItem {
                        name,
                        node_kind: node.kind(),
                        node,
                        origin_type: "module_scope",
                    },
                );
            }
        }
        return Ok(());
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_go_spec_names(child, source, items, ambiguous_names)?;
    }
    Ok(())
}

fn collect_go_type_names<'tree>(
    node: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, GoFileItem<'tree>>,
    ambiguous_names: &mut BTreeSet<String>,
) -> Result<()> {
    if matches!(node.kind(), "type_spec" | "type_alias") {
        return insert_go_declaration_item(node, source, node.kind(), items, ambiguous_names);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_go_type_names(child, source, items, ambiguous_names)?;
    }
    Ok(())
}

fn collect_go_import_names<'tree>(
    node: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, GoFileItem<'tree>>,
    ambiguous_names: &mut BTreeSet<String>,
) -> Result<()> {
    if node.kind() == "import_spec" {
        insert_go_import_name(node, source, items, ambiguous_names);
        return Ok(());
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_go_import_names(child, source, items, ambiguous_names)?;
    }
    Ok(())
}

fn insert_go_import_name<'tree>(
    node: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, GoFileItem<'tree>>,
    ambiguous_names: &mut BTreeSet<String>,
) {
    if let Some(name_node) = node.child_by_field_name("name") {
        // Dot imports and blank imports do not introduce a usable local name.
        if name_node.kind() == "package_identifier"
            && let Ok(name) = node_text(name_node, source)
            && !name.trim().is_empty()
        {
            let name = name.trim().to_string();
            insert_go_file_item(
                items,
                ambiguous_names,
                GoFileItem {
                    name,
                    node_kind: "import_spec",
                    node,
                    origin_type: "imported_module",
                },
            );
        }
        return;
    }
    let Some(path_node) = node.child_by_field_name("path") else {
        return;
    };
    let Some(name) = go_default_import_name(path_node, source) else {
        return;
    };
    if is_valid_go_identifier(&name) {
        items.insert(
            name.clone(),
            GoFileItem {
                name,
                node_kind: "import_spec",
                node,
                origin_type: "imported_module",
            },
        );
    }
}

fn insert_go_file_item<'tree>(
    items: &mut BTreeMap<String, GoFileItem<'tree>>,
    ambiguous_names: &mut BTreeSet<String>,
    item: GoFileItem<'tree>,
) {
    if ambiguous_names.contains(&item.name) {
        return;
    }
    let name = item.name.clone();
    if items.insert(name.clone(), item).is_some() {
        items.remove(&name);
        ambiguous_names.insert(name);
    }
}

fn go_default_import_name(path_node: Node<'_>, source: &str) -> Option<String> {
    let text = node_text(path_node, source).ok()?.trim();
    let inner = match path_node.kind() {
        "interpreted_string_literal" => text.strip_prefix('"')?.strip_suffix('"')?,
        "raw_string_literal" => text.strip_prefix('`')?.strip_suffix('`')?,
        _ => return None,
    };
    let last = inner.rsplit('/').next()?;
    if last.is_empty() {
        None
    } else {
        Some(last.to_string())
    }
}

fn is_valid_go_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !(first == '_' || first.is_alphabetic()) {
        return false;
    }
    characters.all(|character| character == '_' || character.is_alphanumeric())
}

/// Predeclared Go identifiers (built-in types, constants, and functions).
/// References to these never require a same-file or imported binding.
const GO_PREDECLARED_NAMES: &[&str] = &[
    "bool",
    "byte",
    "complex64",
    "complex128",
    "error",
    "float32",
    "float64",
    "int",
    "int8",
    "int16",
    "int32",
    "int64",
    "rune",
    "string",
    "uint",
    "uint8",
    "uint16",
    "uint32",
    "uint64",
    "uintptr",
    "true",
    "false",
    "iota",
    "nil",
    "append",
    "cap",
    "clear",
    "close",
    "complex",
    "copy",
    "delete",
    "imag",
    "len",
    "make",
    "max",
    "min",
    "new",
    "panic",
    "print",
    "println",
    "real",
    "recover",
];
