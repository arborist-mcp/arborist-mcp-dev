mod scope;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{ReferenceValidation, resolved_binding_decision, unresolved_binding_decision};
use crate::deadline::DeadlineCheck;
use crate::language::{ParsedDocument, node_text, normalize_path};
use crate::model::{SymbolSummary, SymbolSummaryInit, ValidationBinding};
use crate::semantic::kotlin::{
    kotlin_parameters, kotlin_return_type, kotlin_semantic_path, kotlin_signature,
    kotlin_symbol_name,
};

use scope::KotlinBinding;
use scope::scan_kotlin_symbol_scope;

pub(crate) fn collect_kotlin_reference_validation_with_deadline(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<ReferenceValidation> {
    let normalized_path = normalize_path(path);
    let scope_scan = scan_kotlin_symbol_scope(symbol_node, source, deadline)?;
    let mut file_items = BTreeMap::new();
    collect_kotlin_file_items(document.tree.root_node(), source, &mut file_items, deadline)?;
    let scope_path = kotlin_symbol_scope_path(document.tree.root_node(), symbol_node, source)?;

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
        let summary = kotlin_local_symbol_summary(&normalized_path, &scope_path, binding);
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
            deadline.check("validating Kotlin references")?;
        }
        if KOTLIN_PREDECLARED_NAMES.contains(&name.as_str()) {
            continue;
        }
        match visible_kotlin_file_item(&file_items, name, scope_path.as_deref()) {
            Some(item) => {
                let summary = kotlin_item_symbol_summary(&normalized_path, source, item);
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

fn visible_kotlin_file_item<'tree>(
    file_items: &'tree BTreeMap<String, Vec<KotlinFileItem<'tree>>>,
    name: &str,
    scope_path: Option<&str>,
) -> Option<&'tree KotlinFileItem<'tree>> {
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

fn kotlin_symbol_scope_path(
    root: Node<'_>,
    symbol_node: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    match kotlin_symbol_name(symbol_node, source)? {
        Some(name) => kotlin_semantic_path(root, symbol_node, source, &name),
        None => Ok(None),
    }
}

fn kotlin_local_symbol_summary(
    normalized_path: &str,
    scope_path: &Option<String>,
    binding: &KotlinBinding,
) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::kotlin::{}::{}::{}",
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

fn kotlin_item_symbol_summary(
    normalized_path: &str,
    source: &str,
    item: &KotlinFileItem<'_>,
) -> SymbolSummary {
    let is_function = item.node_kind == "function_declaration";
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{}::kotlin::{}::{}::{}",
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
            kotlin_signature(item.node, source)
        } else {
            None
        },
        parameters: if is_function {
            kotlin_parameters(item.node, source)
        } else {
            Vec::new()
        },
        return_type: if is_function {
            kotlin_return_type(item.node, source)
        } else {
            None
        },
        docstring: None,
    })
}

struct KotlinFileItem<'tree> {
    name: String,
    node_kind: &'static str,
    node: Node<'tree>,
    origin_type: &'static str,
    parent_path: Option<String>,
    semantic_path: Option<String>,
}

fn collect_kotlin_file_items<'tree>(
    root: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<KotlinFileItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    walk_kotlin_file_items(root, root, source, items, deadline)
}

fn walk_kotlin_file_items<'tree>(
    root: Node<'tree>,
    node: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<KotlinFileItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning Kotlin file items")?;
    }
    match node.kind() {
        "class_declaration" | "object_declaration" | "companion_object" | "type_alias" => {
            insert_kotlin_declaration_item(root, node, source, node.kind(), items)?;
            if node.kind() == "class_declaration" {
                insert_kotlin_class_parameter_items(root, node, source, items)?;
            }
            if let Some(body) = kotlin_class_body_node(node) {
                walk_kotlin_file_items(root, body, source, items, deadline)?;
            }
        }
        "function_declaration" => {
            insert_kotlin_declaration_item(root, node, source, "function_declaration", items)?
        }
        "property_declaration" => {
            insert_kotlin_declaration_item(root, node, source, "property_declaration", items)?
        }
        "import" => insert_kotlin_import_item(node, source, items)?,
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                walk_kotlin_file_items(root, child, source, items, deadline)?;
            }
        }
    }
    Ok(())
}

fn insert_kotlin_declaration_item<'tree>(
    root: Node<'tree>,
    node: Node<'tree>,
    source: &str,
    node_kind: &'static str,
    items: &mut BTreeMap<String, Vec<KotlinFileItem<'tree>>>,
) -> Result<()> {
    let Some(name) = kotlin_symbol_name(node, source)? else {
        return Ok(());
    };
    if name.is_empty() {
        return Ok(());
    }
    let semantic_path = kotlin_semantic_path(root, node, source, &name)?;
    items.entry(name.clone()).or_default().push(KotlinFileItem {
        name,
        node_kind,
        node,
        origin_type: "module_scope",
        parent_path: semantic_parent_path(&semantic_path),
        semantic_path,
    });
    Ok(())
}

fn insert_kotlin_class_parameter_items<'tree>(
    root: Node<'tree>,
    class: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<KotlinFileItem<'tree>>>,
) -> Result<()> {
    let Some(class_name) = kotlin_symbol_name(class, source)? else {
        return Ok(());
    };
    let class_path = kotlin_semantic_path(root, class, source, &class_name)?;
    let mut cursor = class.walk();
    for child in class.named_children(&mut cursor) {
        if child.kind() != "primary_constructor" {
            continue;
        }
        let Some(parameters) = kotlin_direct_child_by_kind(child, &["class_parameters"]) else {
            continue;
        };
        let mut parameter_cursor = parameters.walk();
        for parameter in parameters.named_children(&mut parameter_cursor) {
            if parameter.kind() != "class_parameter" {
                continue;
            }
            let Some(name_node) = kotlin_parameter_name_node(parameter) else {
                continue;
            };
            let name = node_text(name_node, source)?.trim().to_string();
            if name.is_empty() {
                continue;
            }
            let semantic_path = class_path
                .as_ref()
                .map(|class_path| format!("{class_path}::{name}"));
            items.entry(name.clone()).or_default().push(KotlinFileItem {
                name,
                node_kind: "class_parameter",
                node: parameter,
                origin_type: "module_scope",
                parent_path: semantic_parent_path(&semantic_path),
                semantic_path,
            });
        }
    }
    Ok(())
}

fn insert_kotlin_import_item<'tree>(
    import: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<KotlinFileItem<'tree>>>,
) -> Result<()> {
    let Some(simple_name) = kotlin_import_simple_name(import, source) else {
        return Ok(());
    };
    items
        .entry(simple_name.clone())
        .or_default()
        .push(KotlinFileItem {
            name: simple_name.clone(),
            node_kind: "import",
            node: import,
            origin_type: "imported_module",
            parent_path: None,
            semantic_path: Some(simple_name),
        });
    Ok(())
}

/// Returns the rightmost identifier of an import, which is the usable simple
/// name, or the alias when the import is aliased such as
/// `import com.example.util.Helper as H`.
fn kotlin_import_simple_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut identifiers = Vec::new();
    collect_kotlin_identifiers(node, &mut identifiers);
    identifiers
        .last()
        .and_then(|last| node_text(*last, source).ok())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn collect_kotlin_identifiers<'tree>(node: Node<'tree>, out: &mut Vec<Node<'tree>>) {
    if node.kind() == "identifier" {
        out.push(node);
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_kotlin_identifiers(child, out);
    }
}

fn kotlin_parameter_name_node(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "identifier")
}

fn kotlin_class_body_node<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| matches!(child.kind(), "class_body" | "enum_class_body"))
}

fn kotlin_direct_child_by_kind<'tree>(node: Node<'tree>, kinds: &[&str]) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| kinds.contains(&child.kind()))
}

fn semantic_parent_path(semantic_path: &Option<String>) -> Option<String> {
    let path = semantic_path.as_deref()?;
    let mut parts: Vec<&str> = path.split("::").collect();
    parts.pop();
    (!parts.is_empty()).then(|| parts.join("::"))
}

/// Kotlin standard-library top-level names that are visible without an import
/// (the `kotlin.*` auto-imported surface) and therefore never require a
/// same-file or imported binding. Type spellings are skipped during scanning,
/// so this list covers values and functions used as bare references.
const KOTLIN_PREDECLARED_NAMES: &[&str] = &[
    "println",
    "print",
    "error",
    "TODO",
    "require",
    "check",
    "assert",
    "listOf",
    "setOf",
    "mapOf",
    "mutableListOf",
    "mutableSetOf",
    "mutableMapOf",
    "emptyList",
    "emptySet",
    "emptyMap",
    "arrayOf",
    "arrayOfNulls",
    "emptyArray",
    "intArrayOf",
    "longArrayOf",
    "shortArrayOf",
    "byteArrayOf",
    "doubleArrayOf",
    "floatArrayOf",
    "booleanArrayOf",
    "charArrayOf",
    "arrayListOf",
    "hashMapOf",
    "linkedMapOf",
    "sortedMapOf",
    "hashSetOf",
    "linkedSetOf",
    "sortedSetOf",
    "repeat",
    "lazy",
    "with",
    "run",
    "apply",
    "also",
    "let",
    "takeIf",
    "takeUnless",
    "synchronized",
    "minOf",
    "maxOf",
    "sumOf",
    "compareBy",
    "compareByDescending",
    "rangeTo",
    "downTo",
    "until",
    "step",
    "coerceAtLeast",
    "coerceAtMost",
    "coerceIn",
    "buildString",
    "field",
];
