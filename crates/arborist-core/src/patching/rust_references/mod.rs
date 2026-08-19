mod scope;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{ReferenceValidation, resolved_binding_decision, unresolved_binding_decision};
use crate::deadline::DeadlineCheck;
use crate::language::{ParsedDocument, node_text, normalize_path};
use crate::model::{SymbolSummary, SymbolSummaryInit, ValidationBinding};
use crate::semantic::rust::{
    rust_parameters, rust_return_type, rust_semantic_path, rust_signature, rust_symbol_name,
};

use scope::RustBinding;
use scope::scan_rust_symbol_scope;

pub(crate) fn collect_rust_reference_validation_with_deadline(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<ReferenceValidation> {
    let normalized_path = normalize_path(path);
    let scope_scan = scan_rust_symbol_scope(symbol_node, source, deadline)?;
    let mut file_items = BTreeMap::new();
    collect_rust_file_items(document.tree.root_node(), source, &mut file_items, deadline)?;
    let scope_path = rust_symbol_scope_path(symbol_node, source)?;

    let mut validation = ReferenceValidation::default();
    for name in &scope_scan.local_references {
        let Some(binding) = scope_scan
            .local_bindings
            .iter()
            .find(|binding| &binding.name == name)
        else {
            continue;
        };
        let summary = rust_local_symbol_summary(&normalized_path, &scope_path, binding);
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
            deadline.check("validating Rust references")?;
        }
        if RUST_PRELUDE_NAMES.contains(&name.as_str()) {
            continue;
        }
        // Item declarations are hoisted inside a block, so a nested item name
        // resolves even when the reference precedes the declaration text.
        if let Some(binding) = scope_scan
            .local_bindings
            .iter()
            .find(|binding| &binding.name == name && is_rust_nested_item_kind(binding.node_kind))
        {
            let summary = rust_local_symbol_summary(&normalized_path, &scope_path, binding);
            validation
                .binding_decisions
                .push(resolved_binding_decision(name, &summary));
            validation.resolved_identifiers.push(ValidationBinding {
                name: name.clone(),
                symbol: summary,
            });
            continue;
        }
        match file_items.get(name) {
            Some(item) => {
                let summary = rust_item_symbol_summary(&normalized_path, source, item);
                validation
                    .binding_decisions
                    .push(resolved_binding_decision(name, &summary));
                validation.resolved_identifiers.push(ValidationBinding {
                    name: name.clone(),
                    symbol: summary,
                });
            }
            None => {
                validation
                    .binding_decisions
                    .push(unresolved_binding_decision(name));
                validation.unresolved_identifiers.push(name.clone());
            }
        }
    }
    Ok(validation)
}

fn rust_symbol_scope_path(symbol_node: Node<'_>, source: &str) -> Result<Option<String>> {
    match rust_symbol_name(symbol_node, source)? {
        Some(name) => rust_semantic_path(symbol_node, source, &name),
        None => Ok(None),
    }
}

fn rust_local_symbol_summary(
    normalized_path: &str,
    scope_path: &Option<String>,
    binding: &RustBinding,
) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::rust::{}::{}::{}",
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

fn rust_item_symbol_summary(
    normalized_path: &str,
    source: &str,
    item: &RustFileItem<'_>,
) -> SymbolSummary {
    let semantic_path = item.name.clone();
    let is_function = matches!(
        item.node.kind(),
        "function_item" | "function_signature_item"
    );
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::rust::<module>::{}::{}",
            normalized_path, item.node_kind, semantic_path
        ),
        semantic_path,
        scope_path: None,
        file_path: normalized_path.to_string(),
        node_kind: item.node_kind.to_string(),
        origin_type: item.origin_type.to_string(),
        byte_range: (item.node.start_byte(), item.node.end_byte()),
        signature: if is_function {
            rust_signature(item.node, source)
        } else {
            None
        },
        parameters: if is_function {
            rust_parameters(item.node, source)
        } else {
            Vec::new()
        },
        return_type: if is_function {
            rust_return_type(item.node, source)
        } else {
            None
        },
        docstring: None,
    })
}

struct RustFileItem<'tree> {
    name: String,
    node_kind: &'static str,
    node: Node<'tree>,
    origin_type: &'static str,
}

fn collect_rust_file_items<'tree>(
    root: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, RustFileItem<'tree>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if let Some(deadline) = deadline {
            deadline.check("scanning Rust file items")?;
        }
        if child.kind() == "use_declaration" {
            if let Some(argument) = child.child_by_field_name("argument") {
                collect_rust_use_names(argument, source, child, items)?;
            }
            continue;
        }
        if is_rust_declaration_item(child.kind())
            && let Some(name_node) = child.child_by_field_name("name")
            && matches!(name_node.kind(), "identifier" | "type_identifier")
        {
            let name = node_text(name_node, source)?.trim().to_string();
            if !name.is_empty() {
                items.insert(
                    name.clone(),
                    RustFileItem {
                        name,
                        node_kind: child.kind(),
                        node: child,
                        origin_type: "module_scope",
                    },
                );
            }
        }
    }
    Ok(())
}

fn collect_rust_use_names<'tree>(
    node: Node<'tree>,
    source: &str,
    use_node: Node<'tree>,
    items: &mut BTreeMap<String, RustFileItem<'tree>>,
) -> Result<()> {
    match node.kind() {
        "identifier" => {
            insert_rust_use_name(node, source, use_node, items);
            Ok(())
        }
        "use_as_clause" => {
            if let Some(alias) = node.child_by_field_name("alias") {
                insert_rust_use_name(alias, source, use_node, items);
            }
            Ok(())
        }
        "scoped_identifier" => {
            if let Some(name) = node.child_by_field_name("name") {
                insert_rust_use_name(name, source, use_node, items);
            }
            Ok(())
        }
        "use_list" | "scoped_use_list" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_rust_use_names(child, source, use_node, items)?;
            }
            Ok(())
        }
        "use_wildcard" | "self" | "super" | "crate" | "_" => Ok(()),
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_rust_use_names(child, source, use_node, items)?;
            }
            Ok(())
        }
    }
}

fn insert_rust_use_name<'tree>(
    node: Node<'tree>,
    source: &str,
    use_node: Node<'tree>,
    items: &mut BTreeMap<String, RustFileItem<'tree>>,
) {
    if node.kind() != "identifier" {
        return;
    }
    let Ok(name) = node_text(node, source) else {
        return;
    };
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    items.insert(
        name.to_string(),
        RustFileItem {
            name: name.to_string(),
            node_kind: "use_declaration",
            node: use_node,
            origin_type: "imported_module",
        },
    );
}

fn is_rust_nested_item_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "function_signature_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "type_item"
            | "const_item"
            | "static_item"
            | "mod_item"
            | "union_item"
    )
}

fn is_rust_declaration_item(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "function_signature_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "type_item"
            | "const_item"
            | "static_item"
            | "mod_item"
            | "union_item"
            | "macro_definition"
    )
}

const RUST_PRELUDE_NAMES: &[&str] = &[
    "Some",
    "None",
    "Ok",
    "Err",
    "String",
    "Vec",
    "Box",
    "Option",
    "Result",
    "Drop",
    "Clone",
    "Copy",
    "Default",
    "Eq",
    "Ord",
    "PartialEq",
    "PartialOrd",
    "Into",
    "From",
    "TryFrom",
    "TryInto",
    "ToOwned",
    "AsRef",
    "AsMut",
    "Send",
    "Sync",
    "Sized",
    "ToString",
    "Iterator",
    "IntoIterator",
    "ExactSizeIterator",
    "DoubleEndedIterator",
    "Extend",
    "Fn",
    "FnMut",
    "FnOnce",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "f32",
    "f64",
    "bool",
    "char",
    "str",
];
