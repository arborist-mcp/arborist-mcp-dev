use std::collections::BTreeSet;

use anyhow::Result;
use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::language::node_text;

#[derive(Debug, Clone)]
pub(super) struct RustBinding {
    pub(super) name: String,
    pub(super) node_kind: &'static str,
    pub(super) start_byte: usize,
    pub(super) end_byte: usize,
}

pub(super) struct RustScopeScan {
    pub(super) local_bindings: Vec<RustBinding>,
    pub(super) local_references: BTreeSet<String>,
    pub(super) external_references: BTreeSet<String>,
}

/// Walks the patched symbol node and classifies identifier references into
/// locally visible bindings (parameters, `let`/`for`/`match`/closure patterns,
/// and nested item declarations) versus names that must resolve at module scope.
pub(super) fn scan_rust_symbol_scope(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<RustScopeScan> {
    let mut scan = RustScopeScan {
        local_bindings: Vec::new(),
        local_references: BTreeSet::new(),
        external_references: BTreeSet::new(),
    };
    let mut scopes: Vec<Vec<RustBinding>> = Vec::new();

    match symbol_node.kind() {
        "function_item" | "function_signature_item" => {
            let mut function_scope = Vec::new();
            if let Some(parameters) = symbol_node.child_by_field_name("parameters") {
                collect_parameter_bindings(parameters, source, &mut function_scope, &mut scan)?;
            }
            scopes.push(function_scope);
            if let Some(body) = symbol_node.child_by_field_name("body") {
                walk_rust_node(body, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        "const_item" | "static_item" => {
            if let Some(value) = symbol_node.child_by_field_name("value") {
                walk_rust_node(value, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        _ => {
            walk_rust_item_children(symbol_node, source, &mut scopes, &mut scan, deadline)?;
        }
    }

    Ok(scan)
}

fn walk_rust_block(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Vec::new());
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_rust_node(child, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_rust_item_children(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let name_id = node.child_by_field_name("name").map(|name| name.id());
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if Some(child.id()) == name_id {
            continue;
        }
        walk_rust_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_rust_node(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning Rust patch references")?;
    }
    match node.kind() {
        "identifier" => record_rust_reference(node, source, scopes, scan),
        "block" => walk_rust_block(node, source, scopes, scan, deadline),
        "let_declaration" => walk_rust_let_declaration(node, source, scopes, scan, deadline),
        "for_expression" => walk_rust_for_expression(node, source, scopes, scan, deadline),
        "closure_expression" => walk_rust_closure_expression(node, source, scopes, scan, deadline),
        "match_expression" => walk_rust_match_expression(node, source, scopes, scan, deadline),
        "if_expression" => walk_rust_if_expression(node, source, scopes, scan, deadline),
        "while_expression" => walk_rust_while_expression(node, source, scopes, scan, deadline),
        "function_item" | "function_signature_item" => {
            walk_rust_function_item(node, source, scopes, scan, deadline)
        }
        "const_item" | "static_item" => {
            bind_nested_item_name(node, source, scopes, scan)?;
            if let Some(value) = node.child_by_field_name("value") {
                walk_rust_node(value, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        "struct_item" | "enum_item" | "trait_item" | "type_item" | "union_item" | "mod_item" => {
            bind_nested_item_name(node, source, scopes, scan)?;
            walk_rust_item_children(node, source, scopes, scan, deadline)
        }
        "parameters" => {
            let mut bindings = Vec::new();
            collect_parameter_bindings(node, source, &mut bindings, scan)?;
            if let Some(scope) = scopes.last_mut() {
                scope.extend(bindings);
            }
            Ok(())
        }
        "let_condition" => {
            if let Some(value) = node.child_by_field_name("value") {
                walk_rust_node(value, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        "field_expression" => walk_field_value(node, source, scopes, scan, deadline),
        "generic_function" => walk_field_value(node, source, scopes, scan, deadline),
        "enum_variant" => walk_enum_variant_value(node, source, scopes, scan, deadline),
        "struct_expression" => {
            if let Some(body) = node.child_by_field_name("body") {
                walk_rust_node(body, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        "field_initializer" => walk_field_value(node, source, scopes, scan, deadline),
        "shorthand_field_initializer" => {
            if let Some(identifier) = node.named_child(0)
                && identifier.kind() == "identifier"
            {
                record_rust_reference(identifier, source, scopes, scan)?;
            }
            Ok(())
        }
        "type_cast_expression" | "reference_expression" => {
            walk_field_value(node, source, scopes, scan, deadline)
        }
        "label"
        | "attribute_item"
        | "inner_attribute_item"
        | "type_parameters"
        | "where_clause"
        | "scoped_identifier"
        | "use_declaration"
        | "macro_invocation"
        | "macro_definition"
        | "self_parameter"
        | "parameter" => Ok(()),
        _ => walk_rust_children(node, source, scopes, scan, deadline),
    }
}

fn walk_rust_children(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_rust_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_field_value(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(value) = node.child_by_field_name("value") {
        walk_rust_node(value, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_enum_variant_value(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(value) = node.child_by_field_name("value") {
        walk_rust_node(value, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_rust_let_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(value) = node.child_by_field_name("value") {
        walk_rust_node(value, source, scopes, scan, deadline)?;
    }
    // A let-else alternative never sees the new pattern bindings.
    if let Some(alternative) = node.child_by_field_name("alternative") {
        walk_rust_node(alternative, source, scopes, scan, deadline)?;
    }
    if let Some(pattern) = node.child_by_field_name("pattern") {
        let mut bindings = Vec::new();
        collect_rust_pattern_bindings(pattern, source, "let_declaration", &mut bindings, scan)?;
        if let Some(scope) = scopes.last_mut() {
            scope.extend(bindings);
        }
    }
    Ok(())
}

fn walk_rust_for_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(value) = node.child_by_field_name("value") {
        walk_rust_node(value, source, scopes, scan, deadline)?;
    }
    let mut bindings = Vec::new();
    if let Some(pattern) = node.child_by_field_name("pattern") {
        collect_rust_pattern_bindings(pattern, source, "for_expression", &mut bindings, scan)?;
    }
    scopes.push(bindings);
    if let Some(body) = node.child_by_field_name("body") {
        walk_rust_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_rust_closure_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut bindings = Vec::new();
    if let Some(parameters) = node.child_by_field_name("parameters") {
        collect_closure_parameter_bindings(parameters, source, &mut bindings, scan)?;
    }
    scopes.push(bindings);
    if let Some(body) = node.child_by_field_name("body") {
        walk_rust_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_rust_match_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(value) = node.child_by_field_name("value") {
        walk_rust_node(value, source, scopes, scan, deadline)?;
    }
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for arm in body.named_children(&mut cursor) {
            if arm.kind() != "match_arm" {
                continue;
            }
            let mut arm_bindings = Vec::new();
            if let Some(pattern) = arm.child_by_field_name("pattern") {
                collect_rust_pattern_bindings(
                    pattern,
                    source,
                    "match_arm",
                    &mut arm_bindings,
                    scan,
                )?;
            }
            scopes.push(arm_bindings);
            if let Some(value) = arm.child_by_field_name("value") {
                walk_rust_node(value, source, scopes, scan, deadline)?;
            }
            if let Some(condition) = arm.child_by_field_name("condition") {
                walk_rust_node(condition, source, scopes, scan, deadline)?;
            }
            scopes.pop();
        }
    }
    Ok(())
}

fn walk_rust_if_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let condition_has_let = node
        .child_by_field_name("condition")
        .is_some_and(rust_condition_has_let_binding);
    if condition_has_let {
        if let Some(condition) = node.child_by_field_name("condition") {
            walk_rust_node(condition, source, scopes, scan, deadline)?;
        }
        let mut bindings = Vec::new();
        if let Some(condition) = node.child_by_field_name("condition") {
            collect_let_condition_bindings(condition, source, &mut bindings, scan)?;
        }
        scopes.push(bindings);
        if let Some(consequence) = node.child_by_field_name("consequence") {
            walk_rust_node(consequence, source, scopes, scan, deadline)?;
        }
        scopes.pop();
        if let Some(alternative) = node.child_by_field_name("alternative") {
            walk_rust_node(alternative, source, scopes, scan, deadline)?;
        }
        return Ok(());
    }
    if let Some(condition) = node.child_by_field_name("condition") {
        walk_rust_node(condition, source, scopes, scan, deadline)?;
    }
    if let Some(consequence) = node.child_by_field_name("consequence") {
        walk_rust_node(consequence, source, scopes, scan, deadline)?;
    }
    if let Some(alternative) = node.child_by_field_name("alternative") {
        walk_rust_node(alternative, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_rust_while_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let condition_has_let = node
        .child_by_field_name("condition")
        .is_some_and(rust_condition_has_let_binding);
    if condition_has_let {
        if let Some(condition) = node.child_by_field_name("condition") {
            walk_rust_node(condition, source, scopes, scan, deadline)?;
        }
        let mut bindings = Vec::new();
        if let Some(condition) = node.child_by_field_name("condition") {
            collect_let_condition_bindings(condition, source, &mut bindings, scan)?;
        }
        scopes.push(bindings);
        if let Some(body) = node.child_by_field_name("body") {
            walk_rust_node(body, source, scopes, scan, deadline)?;
        }
        scopes.pop();
        return Ok(());
    }
    if let Some(condition) = node.child_by_field_name("condition") {
        walk_rust_node(condition, source, scopes, scan, deadline)?;
    }
    if let Some(body) = node.child_by_field_name("body") {
        walk_rust_node(body, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_rust_function_item(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<RustBinding>>,
    scan: &mut RustScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    bind_nested_item_name(node, source, scopes, scan)?;
    let mut function_scope = Vec::new();
    if let Some(parameters) = node.child_by_field_name("parameters") {
        collect_parameter_bindings(parameters, source, &mut function_scope, scan)?;
    }
    scopes.push(function_scope);
    if let Some(body) = node.child_by_field_name("body") {
        walk_rust_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn bind_nested_item_name(
    node: Node<'_>,
    source: &str,
    scopes: &mut [Vec<RustBinding>],
    scan: &mut RustScopeScan,
) -> Result<()> {
    let Some(scope) = scopes.last_mut() else {
        return Ok(());
    };
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(());
    };
    if name_node.kind() != "identifier" {
        return Ok(());
    }
    let name = node_text(name_node, source)?.trim().to_string();
    if !is_rust_binding_name(&name) {
        return Ok(());
    }
    let binding = RustBinding {
        name: name.clone(),
        node_kind: node.kind(),
        start_byte: name_node.start_byte(),
        end_byte: name_node.end_byte(),
    };
    scan.local_bindings.push(binding.clone());
    scope.push(binding);
    Ok(())
}

fn collect_parameter_bindings(
    parameters: Node<'_>,
    source: &str,
    out: &mut Vec<RustBinding>,
    scan: &mut RustScopeScan,
) -> Result<()> {
    let mut cursor = parameters.walk();
    for child in parameters.named_children(&mut cursor) {
        if child.kind() == "parameter"
            && let Some(pattern) = child.child_by_field_name("pattern")
        {
            collect_rust_pattern_bindings(pattern, source, "parameter", out, scan)?;
        }
    }
    Ok(())
}

fn collect_closure_parameter_bindings(
    parameters: Node<'_>,
    source: &str,
    out: &mut Vec<RustBinding>,
    scan: &mut RustScopeScan,
) -> Result<()> {
    let mut cursor = parameters.walk();
    for child in parameters.named_children(&mut cursor) {
        if child.kind() == "parameter"
            && let Some(pattern) = child.child_by_field_name("pattern")
        {
            collect_rust_pattern_bindings(pattern, source, "closure_parameter", out, scan)?;
        } else {
            collect_rust_pattern_bindings(child, source, "closure_parameter", out, scan)?;
        }
    }
    Ok(())
}

fn collect_let_condition_bindings(
    node: Node<'_>,
    source: &str,
    out: &mut Vec<RustBinding>,
    scan: &mut RustScopeScan,
) -> Result<()> {
    if node.kind() == "let_condition" {
        if let Some(pattern) = node.child_by_field_name("pattern") {
            collect_rust_pattern_bindings(pattern, source, "let_condition", out, scan)?;
        }
        return Ok(());
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_let_condition_bindings(child, source, out, scan)?;
    }
    Ok(())
}

fn rust_condition_has_let_binding(node: Node<'_>) -> bool {
    match node.kind() {
        "let_condition" => true,
        "let_chain" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .any(rust_condition_has_let_binding)
        }
        _ => false,
    }
}

fn collect_rust_pattern_bindings(
    node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    out: &mut Vec<RustBinding>,
    scan: &mut RustScopeScan,
) -> Result<()> {
    match node.kind() {
        "identifier" | "shorthand_field_identifier" => {
            push_rust_binding(node, source, node_kind, out, scan)
        }
        "scoped_identifier"
        | "generic_pattern"
        | "const_block"
        | "macro_invocation"
        | "remaining_field_pattern" => Ok(()),
        "tuple_struct_pattern" | "struct_pattern" => {
            let type_id = node.child_by_field_name("type").map(|child| child.id());
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if Some(child.id()) != type_id {
                    collect_rust_pattern_bindings(child, source, node_kind, out, scan)?;
                }
            }
            Ok(())
        }
        "match_pattern" => {
            let condition_id = node
                .child_by_field_name("condition")
                .map(|child| child.id());
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if Some(child.id()) != condition_id {
                    collect_rust_pattern_bindings(child, source, node_kind, out, scan)?;
                }
            }
            Ok(())
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_rust_pattern_bindings(child, source, node_kind, out, scan)?;
            }
            Ok(())
        }
    }
}

fn push_rust_binding(
    node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    out: &mut Vec<RustBinding>,
    scan: &mut RustScopeScan,
) -> Result<()> {
    let name = node_text(node, source)?.trim().to_string();
    if !is_rust_binding_name(&name) {
        return Ok(());
    }
    let binding = RustBinding {
        name: name.clone(),
        node_kind,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    };
    out.push(binding.clone());
    scan.local_bindings.push(binding);
    Ok(())
}

fn is_rust_binding_name(name: &str) -> bool {
    name.starts_with("r#")
        || name
            .chars()
            .next()
            .is_some_and(|first| first == '_' || first.is_lowercase())
}

fn record_rust_reference(
    node: Node<'_>,
    source: &str,
    scopes: &mut [Vec<RustBinding>],
    scan: &mut RustScopeScan,
) -> Result<()> {
    let name = node_text(node, source)?.trim().to_string();
    if name.is_empty() {
        return Ok(());
    }
    if rust_name_visible(&name, scopes) {
        scan.local_references.insert(name);
    } else {
        scan.external_references.insert(name);
    }
    Ok(())
}

fn rust_name_visible(name: &str, scopes: &[Vec<RustBinding>]) -> bool {
    scopes
        .iter()
        .rev()
        .any(|scope| scope.iter().any(|binding| binding.name == name))
}
