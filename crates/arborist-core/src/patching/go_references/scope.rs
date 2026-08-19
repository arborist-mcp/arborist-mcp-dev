use std::collections::BTreeSet;

use anyhow::Result;
use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::language::node_text;

#[derive(Debug, Clone)]
pub(super) struct GoBinding {
    pub(super) name: String,
    pub(super) node_kind: &'static str,
    pub(super) start_byte: usize,
    pub(super) end_byte: usize,
}

pub(super) struct GoScopeScan {
    pub(super) local_bindings: Vec<GoBinding>,
    pub(super) local_references: BTreeSet<String>,
    pub(super) external_references: BTreeSet<String>,
}

/// Walks the patched Go symbol node and classifies identifier references into
/// locally visible bindings (receiver, parameters, named results, `:=`/`var`/
/// `const` declarations, range variables, `if`/`for`/`switch` initializers,
/// type-switch aliases, and closure parameters) versus names that must resolve
/// at file scope (package-level declarations and import-introduced names).
pub(super) fn scan_go_symbol_scope(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<GoScopeScan> {
    let mut scan = GoScopeScan {
        local_bindings: Vec::new(),
        local_references: BTreeSet::new(),
        external_references: BTreeSet::new(),
    };
    let mut scopes: Vec<Vec<GoBinding>> = Vec::new();

    match symbol_node.kind() {
        "function_declaration" | "method_declaration" => {
            let mut function_scope = Vec::new();
            collect_go_function_bindings(symbol_node, source, &mut function_scope, &mut scan)?;
            scopes.push(function_scope);
            if let Some(body) = symbol_node.child_by_field_name("body") {
                walk_go_node(body, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        // Type specifications and aliases contain only type content, and type
        // annotations are intentionally not validated in this slice.
        "type_spec" | "type_alias" => {}
        _ => {
            let mut cursor = symbol_node.walk();
            for child in symbol_node.named_children(&mut cursor) {
                walk_go_node(child, source, &mut scopes, &mut scan, deadline)?;
            }
        }
    }

    Ok(scan)
}

fn walk_go_node(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning Go patch references")?;
    }
    match node.kind() {
        "identifier" => record_go_reference(node, source, scopes, scan),
        "block" => walk_go_block(node, source, scopes, scan, deadline),
        "short_var_declaration" => {
            walk_go_short_var_declaration(node, source, scopes, scan, deadline)
        }
        "var_spec" => walk_go_var_spec(node, source, scopes, scan, deadline),
        "const_spec" => walk_go_const_spec(node, source, scopes, scan, deadline),
        "if_statement" => walk_go_if_statement(node, source, scopes, scan, deadline),
        "for_statement" => walk_go_for_statement(node, source, scopes, scan, deadline),
        "func_literal" => walk_go_func_literal(node, source, scopes, scan, deadline),
        "function_declaration" | "method_declaration" => {
            walk_go_nested_function(node, source, scopes, scan, deadline)
        }
        "expression_switch_statement" => {
            walk_go_expression_switch_statement(node, source, scopes, scan, deadline)
        }
        "type_switch_statement" => {
            walk_go_type_switch_statement(node, source, scopes, scan, deadline)
        }
        "select_statement" => walk_go_select_statement(node, source, scopes, scan, deadline),
        "communication_case" => walk_go_communication_case(node, source, scopes, scan, deadline),
        "expression_case" | "default_case" => {
            walk_go_switch_case_body(node, source, scopes, scan, deadline)
        }
        "type_case" => walk_go_type_case(node, source, scopes, scan, deadline),
        "selector_expression" => walk_go_selector_expression(node, source, scopes, scan, deadline),
        "composite_literal" => walk_go_composite_literal(node, source, scopes, scan, deadline),
        "type_conversion_expression" | "type_assertion_expression" => {
            walk_go_operand(node, source, scopes, scan, deadline)
        }
        "type_instantiation_expression" => Ok(()),
        "labeled_statement" => walk_go_labeled_statement(node, source, scopes, scan, deadline),
        _ => {
            if is_ignored_go_node_kind(node.kind()) {
                Ok(())
            } else {
                walk_go_children(node, source, scopes, scan, deadline)
            }
        }
    }
}

fn walk_go_children(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_go_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_go_block(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Vec::new());
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_go_node(child, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_go_short_var_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The right-hand side is evaluated before the new names become visible.
    if let Some(right) = node.child_by_field_name("right") {
        walk_go_node(right, source, scopes, scan, deadline)?;
    }
    if let Some(left) = node.child_by_field_name("left") {
        bind_go_expression_list_identifiers(
            left,
            source,
            "short_var_declaration",
            scopes,
            scan,
            deadline,
        )?;
    }
    Ok(())
}

fn walk_go_var_spec(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    walk_go_spec_declaration(node, source, "var_spec", scopes, scan, deadline)
}

fn walk_go_const_spec(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    walk_go_spec_declaration(node, source, "const_spec", scopes, scan, deadline)
}

fn walk_go_spec_declaration(
    node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The initializer is evaluated before the declared names become visible.
    if let Some(value) = node.child_by_field_name("value") {
        walk_go_node(value, source, scopes, scan, deadline)?;
    }
    let mut cursor = node.walk();
    for name in node.children_by_field_name("name", &mut cursor) {
        bind_go_name(name, source, node_kind, scopes, scan)?;
    }
    Ok(())
}

fn walk_go_if_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // An `if` initializer is scoped to the whole statement, including the else.
    scopes.push(Vec::new());
    if let Some(initializer) = node.child_by_field_name("initializer") {
        walk_go_node(initializer, source, scopes, scan, deadline)?;
    }
    if let Some(condition) = node.child_by_field_name("condition") {
        walk_go_node(condition, source, scopes, scan, deadline)?;
    }
    if let Some(consequence) = node.child_by_field_name("consequence") {
        walk_go_node(consequence, source, scopes, scan, deadline)?;
    }
    if let Some(alternative) = node.child_by_field_name("alternative") {
        walk_go_node(alternative, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_go_for_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let body_id = node.child_by_field_name("body").map(|body| body.id());
    let mut cursor = node.walk();
    let mut head = None;
    for child in node.named_children(&mut cursor) {
        if Some(child.id()) != body_id {
            head = Some(child);
            break;
        }
    }
    let Some(head) = head else {
        // `for { ... }` with only a body.
        return walk_go_children(node, source, scopes, scan, deadline);
    };
    match head.kind() {
        "for_clause" => {
            // The for-clause initializer is scoped to the whole statement.
            scopes.push(Vec::new());
            walk_go_node(head, source, scopes, scan, deadline)?;
            if let Some(body) = node.child_by_field_name("body") {
                walk_go_node(body, source, scopes, scan, deadline)?;
            }
            scopes.pop();
        }
        "range_clause" => {
            // Range variables are scoped to the body; the iterable is evaluated
            // in the enclosing scope before the body scope is pushed.
            walk_go_range_clause(head, source, scopes, scan, deadline)?;
            if let Some(body) = node.child_by_field_name("body") {
                walk_go_node(body, source, scopes, scan, deadline)?;
            }
            scopes.pop();
        }
        _ => {
            walk_go_node(head, source, scopes, scan, deadline)?;
            if let Some(body) = node.child_by_field_name("body") {
                walk_go_node(body, source, scopes, scan, deadline)?;
            }
        }
    }
    Ok(())
}

fn walk_go_range_clause(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(right) = node.child_by_field_name("right") {
        walk_go_node(right, source, scopes, scan, deadline)?;
    }
    // Leave the range scope pushed; the caller walks the body inside it.
    scopes.push(Vec::new());
    if let Some(left) = node.child_by_field_name("left") {
        if go_assignment_uses_short_var(node, source) {
            bind_go_expression_list_identifiers(
                left,
                source,
                "range_clause",
                scopes,
                scan,
                deadline,
            )?;
        } else {
            walk_go_node(left, source, scopes, scan, deadline)?;
        }
    }
    Ok(())
}

fn walk_go_func_literal(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut bindings = Vec::new();
    if let Some(parameters) = node.child_by_field_name("parameters") {
        collect_go_parameter_list_bindings(
            parameters,
            source,
            "closure_parameter",
            &mut bindings,
            scan,
        )?;
    }
    if let Some(result) = node.child_by_field_name("result")
        && result.kind() == "parameter_list"
    {
        collect_go_parameter_list_bindings(result, source, "closure_result", &mut bindings, scan)?;
    }
    scopes.push(bindings);
    if let Some(body) = node.child_by_field_name("body") {
        walk_go_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_go_nested_function(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Go does not allow nested function declarations; handle them defensively
    // like a function literal so parameters never leak into the enclosing scope.
    walk_go_func_literal(node, source, scopes, scan, deadline)
}

fn walk_go_expression_switch_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // A switch initializer is scoped to the whole switch statement.
    scopes.push(Vec::new());
    if let Some(initializer) = node.child_by_field_name("initializer") {
        walk_go_node(initializer, source, scopes, scan, deadline)?;
    }
    if let Some(value) = node.child_by_field_name("value") {
        walk_go_node(value, source, scopes, scan, deadline)?;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "expression_case" | "default_case" => {
                walk_go_switch_case_body(child, source, scopes, scan, deadline)?
            }
            _ => walk_go_node(child, source, scopes, scan, deadline)?,
        }
    }
    scopes.pop();
    Ok(())
}

fn walk_go_type_switch_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The type-switch alias is scoped to the whole statement; the value
    // expression is evaluated before the alias becomes visible.
    scopes.push(Vec::new());
    if let Some(initializer) = node.child_by_field_name("initializer") {
        walk_go_node(initializer, source, scopes, scan, deadline)?;
    }
    if let Some(value) = node.child_by_field_name("value") {
        walk_go_node(value, source, scopes, scan, deadline)?;
    }
    if let Some(alias) = node.child_by_field_name("alias") {
        bind_go_expression_list_identifiers(
            alias,
            source,
            "type_switch_alias",
            scopes,
            scan,
            deadline,
        )?;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "type_case" => walk_go_type_case(child, source, scopes, scan, deadline)?,
            "default_case" => walk_go_switch_case_body(child, source, scopes, scan, deadline)?,
            _ => walk_go_node(child, source, scopes, scan, deadline)?,
        }
    }
    scopes.pop();
    Ok(())
}

fn walk_go_switch_case_body(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Each case body is an implicit block.
    scopes.push(Vec::new());
    walk_go_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_go_type_case(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let type_id = node
        .child_by_field_name("type")
        .map(|type_node| type_node.id());
    scopes.push(Vec::new());
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if Some(child.id()) != type_id {
            walk_go_node(child, source, scopes, scan, deadline)?;
        }
    }
    scopes.pop();
    Ok(())
}

fn walk_go_select_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "communication_case" => {
                walk_go_communication_case(child, source, scopes, scan, deadline)?
            }
            "default_case" => walk_go_switch_case_body(child, source, scopes, scan, deadline)?,
            _ => walk_go_node(child, source, scopes, scan, deadline)?,
        }
    }
    Ok(())
}

fn walk_go_communication_case(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Variables declared by a receive `:=` are scoped to the case body.
    scopes.push(Vec::new());
    let communication_id = node
        .child_by_field_name("communication")
        .map(|child| child.id());
    if let Some(communication) = node.child_by_field_name("communication") {
        if communication.kind() == "receive_statement" {
            if let Some(right) = communication.child_by_field_name("right") {
                walk_go_node(right, source, scopes, scan, deadline)?;
            }
            if let Some(left) = communication.child_by_field_name("left") {
                if go_assignment_uses_short_var(communication, source) {
                    bind_go_expression_list_identifiers(
                        left,
                        source,
                        "receive_statement",
                        scopes,
                        scan,
                        deadline,
                    )?;
                } else {
                    walk_go_node(left, source, scopes, scan, deadline)?;
                }
            }
        } else {
            walk_go_node(communication, source, scopes, scan, deadline)?;
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if Some(child.id()) != communication_id {
            walk_go_node(child, source, scopes, scan, deadline)?;
        }
    }
    scopes.pop();
    Ok(())
}

fn walk_go_selector_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The operand is a value reference; the field name is a `field_identifier`
    // and is intentionally not validated in this slice.
    if let Some(operand) = node.child_by_field_name("operand") {
        walk_go_node(operand, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_go_composite_literal(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The literal type is a type expression and is not a value reference.
    if let Some(body) = node.child_by_field_name("body") {
        walk_go_node(body, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_go_operand(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Type conversions and type assertions reference a type, not a value.
    if let Some(operand) = node.child_by_field_name("operand") {
        walk_go_node(operand, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_go_labeled_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Labels are not variable references.
    let label_id = node.child_by_field_name("label").map(|label| label.id());
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if Some(child.id()) != label_id {
            walk_go_node(child, source, scopes, scan, deadline)?;
        }
    }
    Ok(())
}

fn bind_go_expression_list_identifiers(
    node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    scopes: &mut Vec<Vec<GoBinding>>,
    scan: &mut GoScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "identifier" {
            bind_go_name(child, source, node_kind, scopes, scan)?;
        } else {
            walk_go_node(child, source, scopes, scan, deadline)?;
        }
    }
    Ok(())
}

fn bind_go_name(
    node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    scopes: &mut [Vec<GoBinding>],
    scan: &mut GoScopeScan,
) -> Result<()> {
    let name = node_text(node, source)?.trim().to_string();
    if name.is_empty() || name == "_" {
        return Ok(());
    }
    let binding = GoBinding {
        name: name.clone(),
        node_kind,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    };
    scan.local_bindings.push(binding.clone());
    if let Some(scope) = scopes.last_mut() {
        scope.push(binding);
    }
    Ok(())
}

fn collect_go_function_bindings(
    node: Node<'_>,
    source: &str,
    out: &mut Vec<GoBinding>,
    scan: &mut GoScopeScan,
) -> Result<()> {
    if node.kind() == "method_declaration"
        && let Some(receiver) = node.child_by_field_name("receiver")
    {
        collect_go_parameter_list_bindings(receiver, source, "receiver", out, scan)?;
    }
    if let Some(parameters) = node.child_by_field_name("parameters") {
        collect_go_parameter_list_bindings(parameters, source, "parameter", out, scan)?;
    }
    if let Some(result) = node.child_by_field_name("result")
        && result.kind() == "parameter_list"
    {
        collect_go_parameter_list_bindings(result, source, "result", out, scan)?;
    }
    Ok(())
}

fn collect_go_parameter_list_bindings(
    parameter_list: Node<'_>,
    source: &str,
    node_kind: &'static str,
    out: &mut Vec<GoBinding>,
    scan: &mut GoScopeScan,
) -> Result<()> {
    let mut cursor = parameter_list.walk();
    for child in parameter_list.named_children(&mut cursor) {
        match child.kind() {
            "parameter_declaration" | "variadic_parameter_declaration" => {
                let mut name_cursor = child.walk();
                for name in child.children_by_field_name("name", &mut name_cursor) {
                    push_go_binding(name, source, node_kind, out, scan)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn push_go_binding(
    node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    out: &mut Vec<GoBinding>,
    scan: &mut GoScopeScan,
) -> Result<()> {
    let name = node_text(node, source)?.trim().to_string();
    if name.is_empty() || name == "_" {
        return Ok(());
    }
    let binding = GoBinding {
        name: name.clone(),
        node_kind,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    };
    out.push(binding.clone());
    scan.local_bindings.push(binding);
    Ok(())
}

fn record_go_reference(
    node: Node<'_>,
    source: &str,
    scopes: &[Vec<GoBinding>],
    scan: &mut GoScopeScan,
) -> Result<()> {
    let name = node_text(node, source)?.trim().to_string();
    if name.is_empty() || name == "_" {
        return Ok(());
    }
    if go_name_visible(&name, scopes) {
        scan.local_references.insert(name);
    } else {
        scan.external_references.insert(name);
    }
    Ok(())
}

fn go_name_visible(name: &str, scopes: &[Vec<GoBinding>]) -> bool {
    scopes
        .iter()
        .rev()
        .any(|scope| scope.iter().any(|binding| binding.name == name))
}

fn go_assignment_uses_short_var(node: Node<'_>, source: &str) -> bool {
    let Some(left) = node.child_by_field_name("left") else {
        return false;
    };
    let Some(right) = node.child_by_field_name("right") else {
        return false;
    };
    let Some(between) = source.get(left.end_byte()..right.start_byte()) else {
        return false;
    };
    between.contains(":=")
}

fn is_ignored_go_node_kind(kind: &str) -> bool {
    matches!(
        kind,
        // Type positions: names and structural type spellings never become
        // value references, and type annotations are not validated here.
        "type_identifier"
            | "field_identifier"
            | "package_identifier"
            | "label_name"
            | "qualified_type"
            | "generic_type"
            | "pointer_type"
            | "struct_type"
            | "interface_type"
            | "map_type"
            | "slice_type"
            | "array_type"
            | "implicit_length_array_type"
            | "channel_type"
            | "function_type"
            | "parenthesized_type"
            | "negated_type"
            | "type_arguments"
            | "type_parameter_list"
            | "type_parameter_declaration"
            | "type_constraint"
            | "constraint"
            | "field_declaration"
            | "field_declaration_list"
            | "method_elem"
            | "type_elem"
            // Declaration plumbing handled by dedicated walkers.
            | "parameter_declaration"
            | "variadic_parameter_declaration"
            | "parameter_list"
            // File-level clauses and imports are not part of a symbol body.
            | "package_clause"
            | "import_declaration"
            | "import_spec"
            | "import_spec_list"
    )
}
