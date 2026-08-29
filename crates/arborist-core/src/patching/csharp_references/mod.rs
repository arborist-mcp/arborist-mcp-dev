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
    let mut using_aliases = BTreeMap::new();
    collect_csharp_file_items(
        document.tree.root_node(),
        source,
        &mut file_items,
        &mut using_aliases,
        deadline,
    )?;
    let scope_path = csharp_symbol_scope_path(document.tree.root_node(), symbol_node, source)?;
    let namespace_scope_path =
        csharp_namespace_scope_path(document.tree.root_node(), symbol_node, source)?;

    let mut validation = ReferenceValidation::default();
    for name in &scope_scan.local_references {
        // The validation report has one decision per identifier spelling. When
        // the spelling also appears outside the local binding's scope, validate
        // the external reference instead of accepting every site as local.
        if scope_scan.external_references.contains(name) {
            continue;
        }
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
        if CSHARP_PREDECLARED_NAMES.contains(&name.as_str()) {
            continue;
        }
        let item =
            visible_csharp_file_item(&file_items, name, scope_path.as_deref()).or_else(|| {
                visible_csharp_using_alias(&using_aliases, name, namespace_scope_path.as_deref())
            });
        match item {
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

fn csharp_namespace_scope_path(
    root: Node<'_>,
    symbol_node: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let mut parts = Vec::new();
    let mut root_cursor = root.walk();
    let file_scoped_namespaces = root
        .named_children(&mut root_cursor)
        .filter(|node| node.kind() == "file_scoped_namespace_declaration")
        .filter_map(|node| {
            csharp_symbol_name(node, source)
                .transpose()
                .map(|namespace| namespace.map(|namespace| (node, namespace)))
        })
        .collect::<Result<Vec<_>>>()?;
    // A file-scoped namespace applies only to the root items that follow its
    // declaration. Root-level using aliases before `namespace Demo;` retain
    // file-root scope even though patched symbols after it are in `Demo`.
    if let [(file_scoped_namespace, namespace)] = file_scoped_namespaces.as_slice()
        && file_scoped_namespace.start_byte() < symbol_node.start_byte()
    {
        parts.extend(namespace.split('.').map(str::to_string));
    }

    let mut ancestors = Vec::new();
    let mut current = symbol_node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "namespace_declaration"
            && let Some(namespace) = csharp_symbol_name(candidate, source)?
        {
            ancestors.push(namespace);
        }
        current = candidate.parent();
    }
    ancestors.reverse();
    for namespace in ancestors {
        parts.extend(namespace.split('.').map(str::to_string));
    }
    Ok((!parts.is_empty()).then(|| parts.join("::")))
}

fn csharp_visible_scope_paths(scope_path: Option<&str>) -> Vec<Option<String>> {
    let mut scope_paths = Vec::new();
    let mut current_scope_path = scope_path;
    while let Some(scope_path) = current_scope_path {
        scope_paths.push(Some(scope_path.to_string()));
        current_scope_path = scope_path
            .rsplit_once("::")
            .map(|(parent_path, _)| parent_path);
    }
    scope_paths.push(None);
    scope_paths
}

fn visible_csharp_file_item<'tree>(
    file_items: &'tree BTreeMap<String, Vec<CSharpFileItem<'tree>>>,
    name: &str,
    scope_path: Option<&str>,
) -> Option<&'tree CSharpFileItem<'tree>> {
    let items = file_items.get(name)?;
    for scope_path in csharp_visible_scope_paths(scope_path) {
        let mut candidates = items.iter().filter(|item| item.parent_path == scope_path);
        let Some(candidate) = candidates.next() else {
            continue;
        };
        if candidates.next().is_some() {
            return None;
        }
        return Some(candidate);
    }
    None
}

fn visible_csharp_using_alias<'tree>(
    using_aliases: &'tree BTreeMap<String, Vec<CSharpFileItem<'tree>>>,
    name: &str,
    namespace_scope_path: Option<&str>,
) -> Option<&'tree CSharpFileItem<'tree>> {
    let aliases = using_aliases.get(name)?;
    for scope_path in csharp_visible_scope_paths(namespace_scope_path) {
        let mut candidates = aliases
            .iter()
            .filter(|alias| alias.parent_path == scope_path);
        let Some(candidate) = candidates.next() else {
            continue;
        };
        if candidates.next().is_some() {
            return None;
        }
        return Some(candidate);
    }
    None
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
    items: &mut BTreeMap<String, Vec<CSharpFileItem<'tree>>>,
    using_aliases: &mut BTreeMap<String, Vec<CSharpFileItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    walk_csharp_file_items(root, root, source, items, using_aliases, deadline)
}

fn walk_csharp_file_items<'tree>(
    root: Node<'tree>,
    node: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<CSharpFileItem<'tree>>>,
    using_aliases: &mut BTreeMap<String, Vec<CSharpFileItem<'tree>>>,
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
                walk_csharp_file_items(root, body, source, items, using_aliases, deadline)?;
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
        "using_directive" => insert_csharp_using_alias_item(root, node, source, using_aliases)?,
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                walk_csharp_file_items(root, child, source, items, using_aliases, deadline)?;
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
    items: &mut BTreeMap<String, Vec<CSharpFileItem<'tree>>>,
) -> Result<()> {
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(());
    };
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() {
        return Ok(());
    }
    let semantic_path = csharp_semantic_path(root, node, source, &name)?;
    items.entry(name.clone()).or_default().push(CSharpFileItem {
        name,
        node_kind,
        node,
        origin_type: "module_scope",
        parent_path: semantic_parent_path(&semantic_path),
        semantic_path,
    });
    Ok(())
}

fn collect_csharp_record_component_items<'tree>(
    root: Node<'tree>,
    record: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<CSharpFileItem<'tree>>>,
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
        items.entry(name.clone()).or_default().push(CSharpFileItem {
            name,
            node_kind: "record_parameter",
            node: parameter,
            origin_type: "module_scope",
            parent_path: semantic_parent_path(&semantic_path),
            semantic_path,
        });
    }
    Ok(())
}

fn collect_csharp_declarator_items<'tree>(
    root: Node<'tree>,
    declaration: Node<'tree>,
    source: &str,
    node_kind: &'static str,
    items: &mut BTreeMap<String, Vec<CSharpFileItem<'tree>>>,
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
            items.entry(name.clone()).or_default().push(CSharpFileItem {
                name,
                node_kind,
                node: declarator,
                origin_type: "module_scope",
                parent_path: semantic_parent_path(&semantic_path),
                semantic_path,
            });
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
    using_aliases: &mut BTreeMap<String, Vec<CSharpFileItem<'tree>>>,
) -> Result<()> {
    // Only the alias form (`using Alias = Namespace.Type;`) introduces a
    // usable simple name. Plain and static using directives do not bind a
    // simple name, and their targets are not modeled, so references to names
    // introduced that way fail closed. Aliases retain their lexical namespace
    // scope so only the caller's namespace, an enclosing namespace, or the
    // file root can resolve them.
    let Some(name_node) = using_directive.child_by_field_name("name") else {
        return Ok(());
    };
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() {
        return Ok(());
    }
    let parent_path = csharp_namespace_scope_path(root, using_directive, source)?;
    using_aliases
        .entry(name.clone())
        .or_default()
        .push(CSharpFileItem {
            name: name.clone(),
            node_kind: "using_directive",
            node: using_directive,
            origin_type: "imported_module",
            parent_path,
            semantic_path: Some(name),
        });
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
