mod scope;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{ReferenceValidation, resolved_binding_decision, unresolved_binding_decision};
use crate::deadline::DeadlineCheck;
use crate::language::{ParsedDocument, node_text, normalize_path};
use crate::model::{SymbolSummary, SymbolSummaryInit, ValidationBinding};
use crate::semantic::java::{
    java_parameters, java_return_type, java_semantic_path, java_signature, java_symbol_name,
};

use scope::JavaBinding;
use scope::scan_java_symbol_scope;

pub(crate) fn collect_java_reference_validation_with_deadline(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<ReferenceValidation> {
    let normalized_path = normalize_path(path);
    let scope_scan = scan_java_symbol_scope(symbol_node, source, deadline)?;
    let mut file_items = BTreeMap::new();
    collect_java_file_items(document.tree.root_node(), source, &mut file_items, deadline)?;
    let scope_path = java_symbol_scope_path(document.tree.root_node(), symbol_node, source)?;

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
        let summary = java_local_symbol_summary(&normalized_path, &scope_path, binding);
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
            deadline.check("validating Java references")?;
        }
        if JAVA_PREDECLARED_NAMES.contains(&name.as_str()) {
            continue;
        }
        match visible_java_file_item(&file_items, name, scope_path.as_deref()) {
            Some(item) => {
                let summary = java_item_symbol_summary(&normalized_path, source, item);
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

fn visible_java_file_item<'tree>(
    file_items: &'tree BTreeMap<String, Vec<JavaFileItem<'tree>>>,
    name: &str,
    scope_path: Option<&str>,
) -> Option<&'tree JavaFileItem<'tree>> {
    let items = file_items.get(name)?;
    let mut current_scope_path = scope_path;
    while let Some(scope_path) = current_scope_path {
        let mut candidates = items
            .iter()
            .filter(|item| item.parent_path.as_deref() == Some(scope_path));
        let Some(candidate) = candidates.next() else {
            current_scope_path = scope_path
                .rsplit_once("::")
                .map(|(parent_path, _)| parent_path);
            continue;
        };
        if candidates.next().is_some() {
            return None;
        }
        return Some(candidate);
    }

    let mut root_candidates = items.iter().filter(|item| item.parent_path.is_none());
    let candidate = root_candidates.next()?;
    if root_candidates.next().is_some() {
        return None;
    }
    Some(candidate)
}

fn java_symbol_scope_path(
    root: Node<'_>,
    symbol_node: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    match java_symbol_name(symbol_node, source)? {
        Some(name) => java_semantic_path(root, symbol_node, source, &name),
        None => Ok(None),
    }
}

fn java_local_symbol_summary(
    normalized_path: &str,
    scope_path: &Option<String>,
    binding: &JavaBinding,
) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::java::{}::{}::{}",
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

fn java_item_symbol_summary(
    normalized_path: &str,
    source: &str,
    item: &JavaFileItem<'_>,
) -> SymbolSummary {
    let is_function = item.node_kind == "method_declaration";
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::java::{}::{}::{}",
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
            java_signature(item.node, source)
        } else {
            None
        },
        parameters: if is_function {
            java_parameters(item.node, source)
        } else {
            Vec::new()
        },
        return_type: if is_function {
            java_return_type(item.node, source)
        } else {
            None
        },
        docstring: None,
    })
}

struct JavaFileItem<'tree> {
    name: String,
    node_kind: &'static str,
    node: Node<'tree>,
    origin_type: &'static str,
    parent_path: Option<String>,
    semantic_path: Option<String>,
}

fn collect_java_file_items<'tree>(
    root: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<JavaFileItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    walk_java_file_items(root, root, source, items, deadline)
}

fn walk_java_file_items<'tree>(
    root: Node<'tree>,
    node: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<JavaFileItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning Java file items")?;
    }
    match node.kind() {
        "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "annotation_type_declaration" => {
            insert_java_declaration_item(root, node, source, node.kind(), items)?;
            if node.kind() == "record_declaration" {
                collect_java_record_component_items(root, node, source, items)?;
            }
            if let Some(body) = node.child_by_field_name("body") {
                walk_java_file_items(root, body, source, items, deadline)?;
            }
        }
        "method_declaration" => {
            insert_java_declaration_item(root, node, source, "method_declaration", items)?
        }
        "field_declaration" | "constant_declaration" => {
            collect_java_declarator_items(root, node, source, node.kind(), items)?
        }
        "enum_constant" => {
            insert_java_declaration_item(root, node, source, "enum_constant", items)?
        }
        "import_declaration" => insert_java_import_item(root, node, source, items)?,
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                walk_java_file_items(root, child, source, items, deadline)?;
            }
        }
    }
    Ok(())
}

fn insert_java_declaration_item<'tree>(
    root: Node<'tree>,
    node: Node<'tree>,
    source: &str,
    node_kind: &'static str,
    items: &mut BTreeMap<String, Vec<JavaFileItem<'tree>>>,
) -> Result<()> {
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(());
    };
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() {
        return Ok(());
    }
    let semantic_path = java_semantic_path(root, node, source, &name)?;
    items.entry(name.clone()).or_default().push(JavaFileItem {
        name,
        node_kind,
        node,
        origin_type: "module_scope",
        parent_path: semantic_parent_path(&semantic_path),
        semantic_path,
    });
    Ok(())
}

fn collect_java_record_component_items<'tree>(
    root: Node<'tree>,
    record: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<JavaFileItem<'tree>>>,
) -> Result<()> {
    let Some(parameters) = record.child_by_field_name("parameters") else {
        return Ok(());
    };
    let mut cursor = parameters.walk();
    for parameter in parameters.named_children(&mut cursor) {
        if parameter.kind() != "formal_parameter" {
            continue;
        }
        let Some(name_node) = parameter.child_by_field_name("name") else {
            continue;
        };
        let name = node_text(name_node, source)?.trim().to_string();
        if name.is_empty() {
            continue;
        }
        let semantic_path = java_semantic_path(root, parameter, source, &name)?;
        items.entry(name.clone()).or_default().push(JavaFileItem {
            name,
            node_kind: "record_component",
            node: parameter,
            origin_type: "module_scope",
            parent_path: semantic_parent_path(&semantic_path),
            semantic_path,
        });
    }
    Ok(())
}

fn collect_java_declarator_items<'tree>(
    root: Node<'tree>,
    declaration: Node<'tree>,
    source: &str,
    node_kind: &'static str,
    items: &mut BTreeMap<String, Vec<JavaFileItem<'tree>>>,
) -> Result<()> {
    let mut cursor = declaration.walk();
    for declarator in declaration.named_children(&mut cursor) {
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
        let semantic_path = java_semantic_path(root, declarator, source, &name)?;
        items.entry(name.clone()).or_default().push(JavaFileItem {
            name,
            node_kind,
            node: declarator,
            origin_type: "module_scope",
            parent_path: semantic_parent_path(&semantic_path),
            semantic_path,
        });
    }
    Ok(())
}

fn insert_java_import_item<'tree>(
    root: Node<'tree>,
    import: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<JavaFileItem<'tree>>>,
) -> Result<()> {
    let _ = root;
    let mut cursor = import.walk();
    let mut name_node = None;
    let mut wildcard = false;
    for child in import.named_children(&mut cursor) {
        match child.kind() {
            "scoped_identifier" | "identifier" => name_node = Some(child),
            "asterisk" => wildcard = true,
            _ => {}
        }
    }
    if wildcard {
        // Wildcard imports do not introduce a usable simple name; references
        // to wildcard-imported names fail closed.
        return Ok(());
    }
    let Some(name_node) = name_node else {
        return Ok(());
    };
    let Some(simple_name) = java_import_simple_name(name_node, source) else {
        return Ok(());
    };
    items
        .entry(simple_name.clone())
        .or_default()
        .push(JavaFileItem {
            name: simple_name.clone(),
            node_kind: "import_declaration",
            node: import,
            origin_type: "imported_module",
            parent_path: None,
            semantic_path: Some(simple_name),
        });
    Ok(())
}

fn java_import_simple_name(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" => node_text(node, source)
            .ok()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string),
        "scoped_identifier" => {
            // The rightmost identifier is the imported simple name.
            let mut cursor = node.walk();
            let mut last = None;
            for child in node.named_children(&mut cursor) {
                if matches!(child.kind(), "scoped_identifier" | "identifier") {
                    last = Some(child);
                }
            }
            last.and_then(|last| java_import_simple_name(last, source))
        }
        _ => None,
    }
}

fn semantic_parent_path(semantic_path: &Option<String>) -> Option<String> {
    let path = semantic_path.as_deref()?;
    let mut parts: Vec<&str> = path.split("::").collect();
    parts.pop();
    (!parts.is_empty()).then(|| parts.join("::"))
}

/// Predeclared Java names (primitive types, `java.lang` types, and common
/// `Object` instance methods) that are visible without an import and therefore
/// never require a same-file or imported binding.
const JAVA_PREDECLARED_NAMES: &[&str] = &[
    "boolean",
    "byte",
    "char",
    "short",
    "int",
    "long",
    "float",
    "double",
    "void",
    "Object",
    "String",
    "StringBuilder",
    "StringBuffer",
    "Character",
    "Byte",
    "Short",
    "Integer",
    "Long",
    "Float",
    "Double",
    "Boolean",
    "Number",
    "Void",
    "Math",
    "System",
    "Runtime",
    "Process",
    "ProcessBuilder",
    "ProcessHandle",
    "Class",
    "ClassLoader",
    "Thread",
    "ThreadGroup",
    "Runnable",
    "Exception",
    "RuntimeException",
    "Error",
    "Throwable",
    "ArithmeticException",
    "ArrayIndexOutOfBoundsException",
    "ArrayStoreException",
    "ClassCastException",
    "ClassNotFoundException",
    "CloneNotSupportedException",
    "Enum",
    "IllegalArgumentException",
    "IllegalStateException",
    "IndexOutOfBoundsException",
    "InterruptedException",
    "NegativeArraySizeException",
    "NoSuchFieldException",
    "NoSuchMethodException",
    "NullPointerException",
    "NumberFormatException",
    "SecurityException",
    "StackOverflowError",
    "StringIndexOutOfBoundsException",
    "TypeNotPresentException",
    "UnsupportedOperationException",
    "Iterable",
    "Comparable",
    "CharSequence",
    "AutoCloseable",
    "Cloneable",
    "Appendable",
    "Readable",
    "Deprecated",
    "FunctionalInterface",
    "Override",
    "SuppressWarnings",
    "SafeVarargs",
    "Record",
    "StackWalker",
    "Module",
    "Package",
    "toString",
    "hashCode",
    "equals",
    "getClass",
    "clone",
    "finalize",
    "notify",
    "notifyAll",
    "wait",
];
