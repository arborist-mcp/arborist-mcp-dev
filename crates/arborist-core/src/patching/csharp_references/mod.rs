mod scope;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{ReferenceValidation, resolved_binding_decision, unresolved_binding_decision};
use crate::deadline::DeadlineCheck;
use crate::language::{ParsedDocument, node_text, normalize_path};
use crate::model::{SymbolSummary, SymbolSummaryInit, ValidationBinding};
use crate::semantic::csharp::{
    csharp_parameters, csharp_return_type, csharp_semantic_path, csharp_signature,
    csharp_symbol_name,
};

use scope::CSharpBinding;
use scope::scan_csharp_symbol_scope;

pub(crate) fn collect_csharp_reference_validation_with_deadline(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<ReferenceValidation> {
    let normalized_path = normalize_path(path);
    let scope_scan = scan_csharp_symbol_scope(symbol_node, source, deadline)?;
    let mut file_items = BTreeMap::new();
    collect_csharp_file_items(document.tree.root_node(), source, &mut file_items, deadline)?;
    let scope_path = csharp_symbol_scope_path(document.tree.root_node(), symbol_node, source)?;

    let mut validation = ReferenceValidation::default();
    for name in &scope_scan.local_references {
        let Some(binding) = scope_scan
            .local_bindings
            .iter()
            .find(|binding| &binding.name == name)
        else {
            continue;
        };
        let summary = csharp_local_symbol_summary(&normalized_path, &scope_path, binding);
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
            deadline.check("validating C# references")?;
        }
        // A name that is also visible as a local binding anywhere in the symbol
        // resolves locally; a reference site outside that binding's scope is
        // conservatively treated as the same visible binding rather than being
        // reported unresolved.
        if scope_scan.local_references.contains(name) {
            continue;
        }
        if CSHARP_PREDECLARED_NAMES.contains(&name.as_str()) {
            continue;
        }
        match file_items.get(name) {
            Some(item) => {
                let summary = csharp_item_symbol_summary(&normalized_path, source, item);
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

fn csharp_symbol_scope_path(
    root: Node<'_>,
    symbol_node: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    match csharp_symbol_name(symbol_node, source)? {
        Some(name) => csharp_semantic_path(root, symbol_node, source, &name),
        None => Ok(None),
    }
}

fn csharp_local_symbol_summary(
    normalized_path: &str,
    scope_path: &Option<String>,
    binding: &CSharpBinding,
) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::csharp::{}::{}::{}",
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

fn semantic_parent_path(semantic_path: &Option<String>) -> Option<String> {
    let path = semantic_path.as_deref()?;
    let mut parts: Vec<&str> = path.split("::").collect();
    parts.pop();
    (!parts.is_empty()).then(|| parts.join("::"))
}

fn csharp_item_symbol_summary(
    normalized_path: &str,
    source: &str,
    item: &CSharpFileItem<'_>,
) -> SymbolSummary {
    let is_function = item.node_kind == "method_declaration";
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::csharp::{}::{}::{}",
            normalized_path,
            item.parent_path.as_deref().unwrap_or("<module>"),
            item.node_kind,
            item.name
        ),
        semantic_path: item
            .semantic_path
            .clone()
            .unwrap_or_else(|| item.name.clone()),
        scope_path: item.parent_path.clone(),
        file_path: normalized_path.to_string(),
        node_kind: item.node_kind.to_string(),
        origin_type: item.origin_type.to_string(),
        byte_range: (item.node.start_byte(), item.node.end_byte()),
        signature: if is_function {
            csharp_signature(item.node, source)
        } else {
            None
        },
        parameters: if is_function {
            csharp_parameters(item.node, source)
        } else {
            Vec::new()
        },
        return_type: if is_function {
            csharp_return_type(item.node, source)
        } else {
            None
        },
        docstring: None,
    })
}

struct CSharpFileItem<'tree> {
    name: String,
    node_kind: &'static str,
    node: Node<'tree>,
    origin_type: &'static str,
    parent_path: Option<String>,
    semantic_path: Option<String>,
}

fn collect_csharp_file_items<'tree>(
    root: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, CSharpFileItem<'tree>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    walk_csharp_file_items(root, root, source, items, deadline)
}

fn walk_csharp_file_items<'tree>(
    root: Node<'tree>,
    node: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, CSharpFileItem<'tree>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning C# file items")?;
    }
    match node.kind() {
        "class_declaration"
        | "struct_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration" => {
            insert_csharp_declaration_item(root, node, source, node.kind(), items)?;
            if node.kind() == "record_declaration" {
                collect_csharp_record_component_items(root, node, source, items)?;
            }
            if let Some(body) = node.child_by_field_name("body") {
                walk_csharp_file_items(root, body, source, items, deadline)?;
            }
        }
        "method_declaration" => {
            insert_csharp_declaration_item(root, node, source, "method_declaration", items)?
        }
        "property_declaration" | "event_declaration" => {
            insert_csharp_declaration_item(root, node, source, node.kind(), items)?
        }
        "field_declaration" | "event_field_declaration" => {
            collect_csharp_declarator_items(root, node, source, node.kind(), items)?
        }
        "enum_member_declaration" => {
            insert_csharp_declaration_item(root, node, source, "enum_member_declaration", items)?
        }
        "using_directive" => insert_csharp_using_alias_item(root, node, source, items)?,
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                walk_csharp_file_items(root, child, source, items, deadline)?;
            }
        }
    }
    Ok(())
}

fn insert_csharp_declaration_item<'tree>(
    root: Node<'tree>,
    node: Node<'tree>,
    source: &str,
    node_kind: &'static str,
    items: &mut BTreeMap<String, CSharpFileItem<'tree>>,
) -> Result<()> {
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(());
    };
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() {
        return Ok(());
    }
    let semantic_path = csharp_semantic_path(root, node, source, &name)?;
    items.insert(
        name.clone(),
        CSharpFileItem {
            name,
            node_kind,
            node,
            origin_type: "module_scope",
            parent_path: semantic_parent_path(&semantic_path),
            semantic_path,
        },
    );
    Ok(())
}

fn collect_csharp_record_component_items<'tree>(
    root: Node<'tree>,
    record: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, CSharpFileItem<'tree>>,
) -> Result<()> {
    let Some(parameters) = csharp_record_parameter_list(record) else {
        return Ok(());
    };
    let mut cursor = parameters.walk();
    for parameter in parameters.named_children(&mut cursor) {
        if !matches!(parameter.kind(), "parameter" | "parameter_array") {
            continue;
        }
        let Some(name_node) = parameter.child_by_field_name("name") else {
            continue;
        };
        let name = node_text(name_node, source)?.trim().to_string();
        if name.is_empty() {
            continue;
        }
        let semantic_path = csharp_semantic_path(root, parameter, source, &name)?;
        items.insert(
            name.clone(),
            CSharpFileItem {
                name,
                node_kind: "record_parameter",
                node: parameter,
                origin_type: "module_scope",
                parent_path: semantic_parent_path(&semantic_path),
                semantic_path,
            },
        );
    }
    Ok(())
}

fn collect_csharp_declarator_items<'tree>(
    root: Node<'tree>,
    declaration: Node<'tree>,
    source: &str,
    node_kind: &'static str,
    items: &mut BTreeMap<String, CSharpFileItem<'tree>>,
) -> Result<()> {
    // A C# field or event field wraps its declarators in a
    // `variable_declaration` node.
    let mut cursor = declaration.walk();
    for child in declaration.named_children(&mut cursor) {
        if child.kind() != "variable_declaration" {
            continue;
        }
        let mut declarator_cursor = child.walk();
        for declarator in child.named_children(&mut declarator_cursor) {
            if declarator.kind() != "variable_declarator" {
                continue;
            }
            let Some(name_node) = declarator.child_by_field_name("name") else {
                continue;
            };
            let name = node_text(name_node, source)?.trim().to_string();
            if name.is_empty() {
                continue;
            }
            let semantic_path = csharp_semantic_path(root, declarator, source, &name)?;
            items.insert(
                name.clone(),
                CSharpFileItem {
                    name,
                    node_kind,
                    node: declarator,
                    origin_type: "module_scope",
                    parent_path: semantic_parent_path(&semantic_path),
                    semantic_path,
                },
            );
        }
    }
    Ok(())
}

/// Returns the positional parameter list of a record declaration. The C#
/// grammar stores it as an unfielded repeated child rather than a named field.
fn csharp_record_parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "parameter_list")
}

fn insert_csharp_using_alias_item<'tree>(
    root: Node<'tree>,
    using_directive: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, CSharpFileItem<'tree>>,
) -> Result<()> {
    // Only the alias form (`using Alias = Namespace.Type;`) introduces a
    // usable simple name. Plain and static using directives do not bind a
    // simple name, and their targets are not modeled, so references to names
    // introduced that way fail closed.
    let _ = root;
    let Some(name_node) = using_directive.child_by_field_name("name") else {
        return Ok(());
    };
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() {
        return Ok(());
    }
    items.insert(
        name.clone(),
        CSharpFileItem {
            name: name.clone(),
            node_kind: "using_directive",
            node: using_directive,
            origin_type: "imported_module",
            parent_path: None,
            semantic_path: Some(name),
        },
    );
    Ok(())
}

/// Predeclared C# names (`System` types and common `Object` members) that are
/// visible without an explicit using directive and therefore never require a
/// same-file or aliased-using binding. Primitive keywords such as `int` and
/// `string` parse as `predefined_type` nodes and are skipped by the walker.
const CSHARP_PREDECLARED_NAMES: &[&str] = &[
    "object",
    "string",
    "bool",
    "char",
    "byte",
    "sbyte",
    "short",
    "ushort",
    "int",
    "uint",
    "long",
    "ulong",
    "float",
    "double",
    "decimal",
    "void",
    "Object",
    "String",
    "Math",
    "Console",
    "Exception",
    "SystemException",
    "ArgumentException",
    "ArgumentNullException",
    "ArgumentOutOfRangeException",
    "InvalidOperationException",
    "NotSupportedException",
    "NotImplementedException",
    "NullReferenceException",
    "IndexOutOfRangeException",
    "Array",
    "ArraySegment",
    "DateTime",
    "TimeSpan",
    "Guid",
    "Environment",
    "Enum",
    "Type",
    "Delegate",
    "ValueType",
    "IComparable",
    "IEquatable",
    "IDisposable",
    "IEnumerable",
    "IEnumerator",
    "Attribute",
    "ToString",
    "GetHashCode",
    "Equals",
    "ReferenceEquals",
    "GetType",
    "MemberwiseClone",
    "Finalize",
];
