mod scope;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{ReferenceValidation, resolved_binding_decision, unresolved_binding_decision};
use crate::deadline::DeadlineCheck;
use crate::language::{ParsedDocument, node_text, normalize_path};
use crate::model::{SymbolSummary, SymbolSummaryInit, ValidationBinding};
use crate::semantic::javascript::{
    javascript_namespace_scope_name, javascript_parameters, javascript_return_type,
    javascript_semantic_path, javascript_signature, javascript_symbol_name,
};

use scope::{JavaScriptBinding, scan_javascript_symbol_scope};

/// Names that JavaScript/TypeScript make available without an explicit
/// same-file declaration or import. They are intentionally not reported as
/// unresolved so patched code can keep using standard library and host
/// bindings without a visible local declaration.
const JAVASCRIPT_PREDECLARED_NAMES: &[&str] = &[
    "AbortController",
    "AbortSignal",
    "AggregateError",
    "Array",
    "ArrayBuffer",
    "Atomics",
    "BigInt",
    "BigInt64Array",
    "BigUint64Array",
    "Boolean",
    "Buffer",
    "DataView",
    "Date",
    "DOMException",
    "Error",
    "EvalError",
    "FinalizationRegistry",
    "Float32Array",
    "Float64Array",
    "Function",
    "Infinity",
    "Intl",
    "Int16Array",
    "Int32Array",
    "Int8Array",
    "JSON",
    "Map",
    "Math",
    "NaN",
    "Number",
    "Object",
    "Promise",
    "Proxy",
    "RangeError",
    "ReferenceError",
    "Reflect",
    "RegExp",
    "Set",
    "SharedArrayBuffer",
    "String",
    "Symbol",
    "SyntaxError",
    "TextDecoder",
    "TextEncoder",
    "TypeError",
    "URIError",
    "URL",
    "URLSearchParams",
    "Uint16Array",
    "Uint32Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "WeakMap",
    "WeakRef",
    "WeakSet",
    "WebAssembly",
    "__dirname",
    "__filename",
    "alert",
    "clearInterval",
    "clearTimeout",
    "console",
    "decodeURI",
    "decodeURIComponent",
    "document",
    "encodeURI",
    "encodeURIComponent",
    "eval",
    "exports",
    "fetch",
    "global",
    "globalThis",
    "isFinite",
    "isNaN",
    "localStorage",
    "location",
    "module",
    "navigator",
    "parseFloat",
    "parseInt",
    "process",
    "queueMicrotask",
    "require",
    "sessionStorage",
    "setInterval",
    "setTimeout",
    "structuredClone",
    "undefined",
    "window",
];

pub(crate) fn collect_javascript_reference_validation_with_deadline(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<ReferenceValidation> {
    let normalized_path = normalize_path(path);
    let scope_scan = scan_javascript_symbol_scope(symbol_node, source, deadline)?;
    let mut file_items = BTreeMap::new();
    collect_javascript_file_items(document.tree.root_node(), source, &mut file_items, deadline)?;
    exclude_typescript_type_only_import_aliases(&mut file_items, source)?;
    let scope_path = javascript_symbol_scope_path(symbol_node, source)?;

    let mut validation = ReferenceValidation::default();
    for name in &scope_scan.tdz_references {
        validation
            .binding_decisions
            .push(unresolved_binding_decision(name));
        validation.unresolved_identifiers.push(name.clone());
    }
    for name in &scope_scan.local_references {
        // The public validation report carries one decision per identifier
        // spelling. If the spelling is also referenced outside its local scope,
        // validate it as an external reference instead of claiming every site
        // resolves to the local binding.
        if scope_scan.external_references.contains(name) || scope_scan.tdz_references.contains(name)
        {
            continue;
        }
        let Some(binding) = scope_scan
            .local_bindings
            .iter()
            .find(|binding| &binding.name == name)
        else {
            continue;
        };
        let summary = javascript_local_symbol_summary(&normalized_path, &scope_path, binding);
        validation
            .binding_decisions
            .push(resolved_binding_decision(name, &summary));
        validation.resolved_identifiers.push(ValidationBinding {
            name: name.clone(),
            symbol: summary,
        });
    }
    for name in &scope_scan.external_references {
        if scope_scan.tdz_references.contains(name) {
            continue;
        }
        if let Some(deadline) = deadline {
            deadline.check("validating JavaScript references")?;
        }
        if JAVASCRIPT_PREDECLARED_NAMES.contains(&name.as_str()) {
            continue;
        }
        match visible_javascript_file_item(&file_items, name, scope_path.as_deref()) {
            Some(item) => {
                let summary = javascript_item_symbol_summary(&normalized_path, source, item);
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

fn javascript_symbol_scope_path(symbol_node: Node<'_>, source: &str) -> Result<Option<String>> {
    match javascript_symbol_name(symbol_node, source)? {
        Some(name) => javascript_semantic_path(symbol_node, source, &name).map(Some),
        None => Ok(None),
    }
}

fn javascript_local_symbol_summary(
    normalized_path: &str,
    scope_path: &Option<String>,
    binding: &JavaScriptBinding,
) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::javascript::{}::{}::{}",
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

fn javascript_item_symbol_summary(
    normalized_path: &str,
    source: &str,
    item: &JavaScriptFileItem<'_>,
) -> SymbolSummary {
    let is_function = matches!(
        item.node_kind,
        "function_declaration" | "generator_function_declaration" | "function_signature"
    );
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::javascript::{}::{}::{}",
            normalized_path,
            item.parent_path.as_deref().unwrap_or("<module>"),
            item.node_kind,
            item.name
        ),
        semantic_path: item.semantic_path.clone(),
        scope_path: item.parent_path.clone(),
        file_path: normalized_path.to_string(),
        node_kind: item.node_kind.to_string(),
        origin_type: item.origin_type.to_string(),
        byte_range: (item.node.start_byte(), item.node.end_byte()),
        signature: if is_function {
            javascript_signature(item.node, source)
        } else {
            None
        },
        parameters: if is_function {
            javascript_parameters(item.node, source)
        } else {
            Vec::new()
        },
        return_type: if is_function {
            javascript_return_type(item.node, source)
        } else {
            None
        },
        docstring: None,
    })
}

struct JavaScriptFileItem<'tree> {
    name: String,
    node_kind: &'static str,
    node: Node<'tree>,
    origin_type: &'static str,
    parent_path: Option<String>,
    semantic_path: String,
}

fn visible_javascript_file_item<'tree>(
    file_items: &'tree BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
    name: &str,
    scope_path: Option<&str>,
) -> Option<&'tree JavaScriptFileItem<'tree>> {
    let items = file_items.get(name)?;
    let mut current_scope_path = scope_path;
    while let Some(scope_path) = current_scope_path {
        let mut candidates = items.iter().filter(|item| {
            item.parent_path.as_deref() == Some(scope_path)
                && javascript_file_item_provides_runtime_binding(item)
        });
        let Some(candidate) = candidates.next() else {
            current_scope_path = scope_path
                .rsplit_once("::")
                .map(|(parent_path, _)| parent_path);
            continue;
        };
        if candidates.all(|other| javascript_file_items_merge(candidate, other)) {
            return Some(candidate);
        }
        return None;
    }

    let mut root_candidates = items.iter().filter(|item| {
        item.parent_path.is_none() && javascript_file_item_provides_runtime_binding(item)
    });
    let candidate = root_candidates.next()?;
    if root_candidates.all(|other| javascript_file_items_merge(candidate, other)) {
        Some(candidate)
    } else {
        None
    }
}

fn javascript_file_item_provides_runtime_binding(item: &JavaScriptFileItem<'_>) -> bool {
    if matches!(
        item.node_kind,
        "interface_declaration" | "type_alias_declaration"
    ) {
        return false;
    }
    item.node_kind != "enum_declaration" || !typescript_const_enum_declaration(item.node)
}

fn typescript_const_enum_declaration(node: Node<'_>) -> bool {
    node.child(0).is_some_and(|child| child.kind() == "const")
}

fn exclude_typescript_type_only_import_aliases(
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'_>>>,
    source: &str,
) -> Result<()> {
    let known_paths = items
        .values()
        .flatten()
        .map(|item| item.semantic_path.clone())
        .collect::<BTreeSet<_>>();
    let mut type_only_paths = javascript_type_only_semantic_paths(items);

    loop {
        let aliases = items
            .values()
            .flatten()
            .filter(|item| item.node_kind == "import_alias")
            .map(|item| {
                Ok((
                    item.semantic_path.clone(),
                    typescript_import_alias_target_paths(item, source)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut changed = false;
        for (alias_path, target_paths) in aliases {
            let Some(target_path) = target_paths
                .iter()
                .find(|target_path| known_paths.contains(*target_path))
            else {
                continue;
            };
            if type_only_paths.contains(target_path) {
                changed |= type_only_paths.insert(alias_path);
            }
        }
        if !changed {
            break;
        }
    }

    for bindings in items.values_mut() {
        bindings.retain(|item| {
            item.node_kind != "import_alias" || !type_only_paths.contains(&item.semantic_path)
        });
    }
    Ok(())
}

fn javascript_type_only_semantic_paths(
    items: &BTreeMap<String, Vec<JavaScriptFileItem<'_>>>,
) -> BTreeSet<String> {
    let mut type_only_paths = BTreeSet::new();
    let mut runtime_paths = BTreeSet::new();
    for item in items.values().flatten() {
        if javascript_file_item_provides_runtime_binding(item) {
            runtime_paths.insert(item.semantic_path.clone());
        } else {
            type_only_paths.insert(item.semantic_path.clone());
        }
    }
    type_only_paths.retain(|path| !runtime_paths.contains(path));
    type_only_paths
}

fn typescript_import_alias_target_paths(
    item: &JavaScriptFileItem<'_>,
    source: &str,
) -> Result<Vec<String>> {
    let Some(target) = item.node.named_child(1) else {
        return Ok(Vec::new());
    };
    let target_path = node_text(target, source)?
        .trim()
        .split('.')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>()
        .join("::");
    if target_path.is_empty() {
        return Ok(Vec::new());
    }

    let mut candidates = Vec::new();
    let mut scope_path = item.parent_path.as_deref();
    while let Some(scope) = scope_path {
        candidates.push(format!("{scope}::{target_path}"));
        scope_path = scope.rsplit_once("::").map(|(parent, _)| parent);
    }
    candidates.push(target_path);
    Ok(candidates)
}

fn javascript_file_items_merge(
    first: &JavaScriptFileItem<'_>,
    second: &JavaScriptFileItem<'_>,
) -> bool {
    if first.semantic_path != second.semantic_path || first.parent_path != second.parent_path {
        return false;
    }

    let first_is_namespace = is_javascript_namespace_file_item(first);
    let second_is_namespace = is_javascript_namespace_file_item(second);
    if first_is_namespace {
        second_is_namespace || is_typescript_namespace_merge_value(second)
    } else {
        second_is_namespace && is_typescript_namespace_merge_value(first)
    }
}

fn is_javascript_namespace_file_item(item: &JavaScriptFileItem<'_>) -> bool {
    matches!(item.node_kind, "internal_module" | "module")
}

fn is_typescript_namespace_merge_value(item: &JavaScriptFileItem<'_>) -> bool {
    matches!(
        item.node_kind,
        "function_declaration"
            | "generator_function_declaration"
            | "function_signature"
            | "class_declaration"
            | "abstract_class_declaration"
            | "enum_declaration"
    )
}

fn collect_javascript_file_items<'tree>(
    root: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    collect_javascript_scope_items(root, source, None, items, deadline)
}

fn collect_javascript_scope_items<'tree>(
    scope: Node<'tree>,
    source: &str,
    scope_path: Option<&str>,
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = scope.walk();
    for child in scope.named_children(&mut cursor) {
        if let Some(deadline) = deadline {
            deadline.check("scanning JavaScript file items")?;
        }
        match child.kind() {
            "function_declaration"
            | "generator_function_declaration"
            | "class_declaration"
            | "abstract_class_declaration"
            | "enum_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "function_signature" => {
                insert_javascript_declaration_item(child, source, child.kind(), scope_path, items)?
            }
            "lexical_declaration" | "using_declaration" | "variable_declaration" => {
                collect_javascript_top_level_declarator_names(child, source, scope_path, items)?
            }
            "import_statement" => {
                collect_javascript_import_names(child, source, scope_path, items)?
            }
            "import_alias" => {
                collect_javascript_import_alias_name(child, source, scope_path, items)?
            }
            "ambient_declaration" => {
                collect_javascript_scope_items(child, source, scope_path, items, deadline)?
            }
            "export_statement" => {
                collect_javascript_export_names(child, source, scope_path, items, deadline)?
            }
            "internal_module" | "module" => {
                collect_javascript_namespace_scope_items(
                    child, source, scope_path, items, deadline,
                )?;
            }
            "expression_statement" => {
                let Some(namespace) = child
                    .named_child(0)
                    .filter(|node| matches!(node.kind(), "internal_module" | "module"))
                else {
                    continue;
                };
                collect_javascript_namespace_scope_items(
                    namespace, source, scope_path, items, deadline,
                )?;
            }
            _ => collect_javascript_hoisted_var_items(child, source, scope_path, items, deadline)?,
        }
    }
    Ok(())
}

fn collect_javascript_hoisted_var_items<'tree>(
    node: Node<'tree>,
    source: &str,
    scope_path: Option<&str>,
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning JavaScript hoisted module var bindings")?;
    }
    match node.kind() {
        // A `var` declared in a nested callable or class-static block belongs
        // to that nested scope, not to the surrounding module or namespace.
        "function_declaration"
        | "generator_function_declaration"
        | "function_expression"
        | "generator_function"
        | "arrow_function"
        | "method_definition"
        | "class_declaration"
        | "abstract_class_declaration"
        | "class"
        | "class_static_block"
        // Namespace bodies create a distinct runtime namespace scope and are
        // collected through `collect_javascript_namespace_scope_items`.
        | "internal_module"
        | "module" => Ok(()),
        "variable_declaration" => {
            collect_javascript_top_level_declarator_names(node, source, scope_path, items)
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_javascript_hoisted_var_items(child, source, scope_path, items, deadline)?;
            }
            Ok(())
        }
    }
}

fn collect_javascript_namespace_scope_items<'tree>(
    namespace: Node<'tree>,
    source: &str,
    scope_path: Option<&str>,
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let Some(namespace_name) = javascript_namespace_scope_name(namespace, source)? else {
        return Ok(());
    };
    let Some(namespace_binding_name) = namespace_name
        .split("::")
        .next()
        .filter(|name| !name.is_empty())
    else {
        return Ok(());
    };
    let namespace_node_kind = match namespace.kind() {
        "internal_module" => "internal_module",
        "module" => "module",
        _ => return Ok(()),
    };
    insert_javascript_file_item(
        namespace_binding_name.to_string(),
        namespace_node_kind,
        namespace,
        "module_scope",
        scope_path,
        items,
    );

    let Some(body) = namespace.child_by_field_name("body") else {
        return Ok(());
    };
    let namespace_scope_path = match scope_path {
        Some(parent_path) => format!("{parent_path}::{namespace_name}"),
        None => namespace_name,
    };
    collect_javascript_scope_items(body, source, Some(&namespace_scope_path), items, deadline)
}

fn insert_javascript_declaration_item<'tree>(
    node: Node<'tree>,
    source: &str,
    node_kind: &'static str,
    parent_path: Option<&str>,
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
) -> Result<()> {
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(());
    };
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() {
        return Ok(());
    }
    insert_javascript_file_item(name, node_kind, node, "module_scope", parent_path, items);
    Ok(())
}

fn collect_javascript_top_level_declarator_names<'tree>(
    node: Node<'tree>,
    source: &str,
    parent_path: Option<&str>,
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
) -> Result<()> {
    let mut cursor = node.walk();
    for declarator in node
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "variable_declarator")
    {
        let Some(name_node) = declarator.child_by_field_name("name") else {
            continue;
        };
        collect_javascript_top_level_pattern_names(
            name_node,
            source,
            declarator,
            parent_path,
            items,
        )?;
    }
    Ok(())
}

fn collect_javascript_top_level_pattern_names<'tree>(
    pattern: Node<'tree>,
    source: &str,
    declarator: Node<'tree>,
    parent_path: Option<&str>,
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
) -> Result<()> {
    match pattern.kind() {
        "identifier" | "shorthand_property_identifier_pattern" => {
            let name = node_text(pattern, source)?.trim().to_string();
            if !name.is_empty() {
                insert_javascript_file_item(
                    name,
                    "variable_declarator",
                    declarator,
                    "module_scope",
                    parent_path,
                    items,
                );
            }
        }
        "assignment_pattern" | "object_assignment_pattern" => {
            if let Some(left) = pattern.child_by_field_name("left") {
                collect_javascript_top_level_pattern_names(
                    left,
                    source,
                    declarator,
                    parent_path,
                    items,
                )?;
            }
        }
        "rest_pattern" | "array_pattern" => {
            let mut cursor = pattern.walk();
            for child in pattern.named_children(&mut cursor) {
                collect_javascript_top_level_pattern_names(
                    child,
                    source,
                    declarator,
                    parent_path,
                    items,
                )?;
            }
        }
        "object_pattern" => {
            let mut cursor = pattern.walk();
            for member in pattern.named_children(&mut cursor) {
                if member.kind() == "pair_pattern" {
                    if let Some(value) = member.child_by_field_name("value") {
                        collect_javascript_top_level_pattern_names(
                            value,
                            source,
                            declarator,
                            parent_path,
                            items,
                        )?;
                    }
                } else {
                    collect_javascript_top_level_pattern_names(
                        member,
                        source,
                        declarator,
                        parent_path,
                        items,
                    )?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn collect_javascript_import_alias_name<'tree>(
    node: Node<'tree>,
    source: &str,
    parent_path: Option<&str>,
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
) -> Result<()> {
    let Some(name_node) = node
        .named_child(0)
        .filter(|name| name.kind() == "identifier")
    else {
        return Ok(());
    };
    let name = node_text(name_node, source)?.trim().to_string();
    if !name.is_empty() {
        insert_javascript_file_item(
            name,
            "import_alias",
            node,
            "imported_module",
            parent_path,
            items,
        );
    }
    Ok(())
}

fn collect_javascript_import_names<'tree>(
    node: Node<'tree>,
    source: &str,
    parent_path: Option<&str>,
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
) -> Result<()> {
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if javascript_type_only_import(current) {
            continue;
        }
        match current.kind() {
            "namespace_import" => {
                let name_node = current.child_by_field_name("name").or_else(|| {
                    current
                        .named_child(0)
                        .filter(|name| name.kind() == "identifier")
                });
                if let Some(name_node) = name_node {
                    let name = node_text(name_node, source)?.trim().to_string();
                    if !name.is_empty() {
                        insert_javascript_file_item(
                            name,
                            "namespace_import",
                            current,
                            "imported_module",
                            parent_path,
                            items,
                        );
                    }
                }
            }
            "import_specifier" => {
                let name_node = current
                    .child_by_field_name("alias")
                    .or_else(|| current.child_by_field_name("name"));
                if let Some(name_node) = name_node {
                    let name = node_text(name_node, source)?.trim().to_string();
                    if !name.is_empty() {
                        insert_javascript_file_item(
                            name,
                            "import_specifier",
                            current,
                            "imported_module",
                            parent_path,
                            items,
                        );
                    }
                }
            }
            "identifier" => {
                // A bare identifier under the import clause is the default
                // import binding (`import def from "./module"`).
                let name = node_text(current, source)?.trim().to_string();
                if !name.is_empty() {
                    insert_javascript_file_item(
                        name,
                        "default_import",
                        current,
                        "imported_module",
                        parent_path,
                        items,
                    );
                }
            }
            _ => {
                let mut children = current
                    .named_children(&mut current.walk())
                    .collect::<Vec<_>>();
                children.reverse();
                pending.extend(children);
            }
        }
    }
    Ok(())
}

fn javascript_type_only_import(node: Node<'_>) -> bool {
    match node.kind() {
        "import_statement" => node.child(1).is_some_and(is_typescript_type_modifier),
        "import_specifier" => node.child(0).is_some_and(is_typescript_type_modifier),
        _ => false,
    }
}

fn is_typescript_type_modifier(node: Node<'_>) -> bool {
    !node.is_named() && matches!(node.kind(), "type" | "typeof")
}

fn insert_javascript_file_item<'tree>(
    name: String,
    node_kind: &'static str,
    node: Node<'tree>,
    origin_type: &'static str,
    parent_path: Option<&str>,
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
) {
    let parent_path = parent_path.map(str::to_string);
    let semantic_path = parent_path
        .as_deref()
        .map(|parent_path| format!("{parent_path}::{name}"))
        .unwrap_or_else(|| name.clone());
    items
        .entry(name.clone())
        .or_default()
        .push(JavaScriptFileItem {
            name,
            node_kind,
            node,
            origin_type,
            parent_path,
            semantic_path,
        });
}

fn collect_javascript_export_names<'tree>(
    node: Node<'tree>,
    source: &str,
    parent_path: Option<&str>,
    items: &mut BTreeMap<String, Vec<JavaScriptFileItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "function_declaration"
            | "generator_function_declaration"
            | "class_declaration"
            | "abstract_class_declaration"
            | "enum_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "function_signature" => {
                insert_javascript_declaration_item(child, source, child.kind(), parent_path, items)?
            }
            "lexical_declaration" | "using_declaration" | "variable_declaration" => {
                collect_javascript_top_level_declarator_names(child, source, parent_path, items)?
            }
            "internal_module" | "module" => collect_javascript_namespace_scope_items(
                child,
                source,
                parent_path,
                items,
                deadline,
            )?,
            "import_alias" => {
                collect_javascript_import_alias_name(child, source, parent_path, items)?
            }
            "ambient_declaration" => {
                collect_javascript_scope_items(child, source, parent_path, items, deadline)?
            }
            _ => {}
        }
    }
    Ok(())
}
