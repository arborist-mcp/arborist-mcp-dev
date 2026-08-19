use std::collections::BTreeSet;

use anyhow::Result;
use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::language::node_text;

#[derive(Debug, Clone)]
pub(super) struct JavaBinding {
    pub(super) name: String,
    pub(super) node_kind: &'static str,
    pub(super) start_byte: usize,
    pub(super) end_byte: usize,
}

pub(super) struct JavaScopeScan {
    pub(super) local_bindings: Vec<JavaBinding>,
    pub(super) local_references: BTreeSet<String>,
    pub(super) external_references: BTreeSet<String>,
}

/// Walks the patched Java symbol node and classifies identifier references into
/// locally visible bindings (formal parameters, local declarators, `for` and
/// enhanced-`for` variables, catch parameters, try-with-resources variables,
/// lambda parameters, and pattern variables) versus names that must resolve at
/// file scope (same-file declarations and import-introduced names). Type
/// spellings, field and method names, labels, and annotation names are not
/// value references and are skipped.
pub(super) fn scan_java_symbol_scope(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<JavaScopeScan> {
    let mut scan = JavaScopeScan {
        local_bindings: Vec::new(),
        local_references: BTreeSet::new(),
        external_references: BTreeSet::new(),
    };
    let mut scopes: Vec<Vec<JavaBinding>> = Vec::new();

    match symbol_node.kind() {
        "method_declaration" | "constructor_declaration" | "compact_constructor_declaration" => {
            let mut function_scope = Vec::new();
            collect_java_function_bindings(symbol_node, source, &mut function_scope, &mut scan)?;
            scopes.push(function_scope);
            if let Some(body) = symbol_node.child_by_field_name("body") {
                walk_java_node(body, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        // A patched type declaration validates references in field initializers,
        // method bodies, and nested declarations; its own name, type parameters,
        // superclass, and interface list are not value references.
        "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "annotation_type_declaration" => {
            let mut class_scope = Vec::new();
            if symbol_node.kind() == "record_declaration" {
                collect_java_record_components(symbol_node, source, &mut class_scope, &mut scan)?;
            }
            scopes.push(class_scope);
            if let Some(body) = symbol_node.child_by_field_name("body") {
                walk_java_node(body, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        _ => {
            let mut cursor = symbol_node.walk();
            for child in symbol_node.named_children(&mut cursor) {
                walk_java_node(child, source, &mut scopes, &mut scan, deadline)?;
            }
        }
    }

    Ok(scan)
}

fn walk_java_node(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning Java patch references")?;
    }
    match node.kind() {
        "identifier" => record_java_reference(node, source, scopes, scan),
        "block" => walk_java_block(node, source, scopes, scan, deadline),
        "method_declaration" | "constructor_declaration" | "compact_constructor_declaration" => {
            walk_java_function_declaration(node, source, scopes, scan, deadline)
        }
        "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "annotation_type_declaration" => {
            walk_java_type_declaration(node, source, scopes, scan, deadline)
        }
        "lambda_expression" => walk_java_lambda_expression(node, source, scopes, scan, deadline),
        "local_variable_declaration" => {
            walk_java_local_variable_declaration(node, source, scopes, scan, deadline)
        }
        "variable_declarator" => {
            walk_java_variable_declarator(node, source, scopes, scan, deadline)
        }
        "field_declaration" | "constant_declaration" => {
            walk_java_field_declaration(node, source, scopes, scan, deadline)
        }
        "enhanced_for_statement" => {
            walk_java_enhanced_for_statement(node, source, scopes, scan, deadline)
        }
        "for_statement" => walk_java_for_statement(node, source, scopes, scan, deadline),
        "try_with_resources_statement" => {
            walk_java_try_with_resources(node, source, scopes, scan, deadline)
        }
        "catch_clause" => walk_java_catch_clause(node, source, scopes, scan, deadline),
        "resource" => walk_java_resource(node, source, scopes, scan, deadline),
        "switch_block" => walk_java_switch_block(node, source, scopes, scan, deadline),
        "instanceof_expression" => walk_java_instanceof(node, source, scopes, scan, deadline),
        "cast_expression" => walk_java_cast(node, source, scopes, scan, deadline),
        "method_invocation" => walk_java_method_invocation(node, source, scopes, scan, deadline),
        "field_access" => walk_java_field_access(node, source, scopes, scan, deadline),
        "object_creation_expression" => {
            walk_java_object_creation(node, source, scopes, scan, deadline)
        }
        "method_reference" => walk_java_method_reference(node, source, scopes, scan, deadline),
        "labeled_statement" => walk_java_labeled_statement(node, source, scopes, scan, deadline),
        "break_statement" | "continue_statement" => Ok(()),
        "type_pattern" => walk_java_pattern_name(node, source, "type_pattern", scopes, scan),
        "record_pattern_component" => {
            walk_java_pattern_name(node, source, "record_pattern", scopes, scan)
        }
        "underscore_pattern" => Ok(()),
        "enum_constant" => walk_java_enum_constant(node, source, scopes, scan, deadline),
        "annotation_type_element_declaration" => {
            walk_java_annotation_default(node, source, scopes, scan, deadline)
        }
        _ => {
            if is_ignored_java_node_kind(node.kind()) {
                Ok(())
            } else {
                walk_java_children(node, source, scopes, scan, deadline)
            }
        }
    }
}

fn walk_java_children(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_java_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_java_block(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Vec::new());
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_java_node(child, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_java_function_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut function_scope = Vec::new();
    collect_java_function_bindings(node, source, &mut function_scope, scan)?;
    scopes.push(function_scope);
    if let Some(body) = node.child_by_field_name("body") {
        walk_java_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_java_type_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut class_scope = Vec::new();
    if node.kind() == "record_declaration" {
        collect_java_record_components(node, source, &mut class_scope, scan)?;
    }
    scopes.push(class_scope);
    if let Some(body) = node.child_by_field_name("body") {
        walk_java_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_java_lambda_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut lambda_scope = Vec::new();
    if let Some(parameters) = node.child_by_field_name("parameters") {
        match parameters.kind() {
            "formal_parameters" => bind_java_parameter_list(
                parameters,
                source,
                "lambda_parameter",
                &mut lambda_scope,
                scan,
            )?,
            "inferred_parameters" => {
                bind_java_inferred_parameters(parameters, source, &mut lambda_scope, scan)?
            }
            _ => {}
        }
    }
    scopes.push(lambda_scope);
    if let Some(body) = node.child_by_field_name("body") {
        walk_java_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_java_local_variable_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Declared names become visible only after their initializers run; the
    // declarator handler walks the value before binding the name.
    walk_java_children(node, source, scopes, scan, deadline)
}

fn walk_java_variable_declarator(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(value) = node.child_by_field_name("value") {
        walk_java_node(value, source, scopes, scan, deadline)?;
    }
    if let Some(name) = node.child_by_field_name("name") {
        bind_java_name(name, source, "variable_declarator", scopes, scan)?;
    }
    Ok(())
}

fn walk_java_field_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Field names are visible to every member of the class; the declarator
    // handler walks the initializer (if any) before binding the name into the
    // current class scope.
    walk_java_children(node, source, scopes, scan, deadline)
}

fn walk_java_enhanced_for_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The iterable expression is evaluated in the enclosing scope.
    if let Some(value) = node.child_by_field_name("value") {
        walk_java_node(value, source, scopes, scan, deadline)?;
    }
    scopes.push(Vec::new());
    if let Some(name) = node.child_by_field_name("name") {
        bind_java_name(name, source, "enhanced_for_statement", scopes, scan)?;
    }
    if let Some(body) = node.child_by_field_name("body") {
        walk_java_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_java_for_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The init variables are scoped to the whole `for` statement including the
    // body; the condition and update run inside that scope.
    scopes.push(Vec::new());
    walk_java_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_java_try_with_resources(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Resource variables are visible throughout the try-with-resources
    // statement, including catch clauses and the finally block.
    scopes.push(Vec::new());
    walk_java_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_java_catch_clause(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Vec::new());
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "catch_formal_parameter"
            && let Some(name) = child.child_by_field_name("name")
        {
            bind_java_name(name, source, "catch_formal_parameter", scopes, scan)?;
        } else {
            walk_java_node(child, source, scopes, scan, deadline)?;
        }
    }
    scopes.pop();
    Ok(())
}

fn walk_java_resource(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(value) = node.child_by_field_name("value") {
        walk_java_node(value, source, scopes, scan, deadline)?;
    }
    if let Some(name) = node.child_by_field_name("name") {
        bind_java_name(name, source, "resource", scopes, scan)?;
    }
    Ok(())
}

fn walk_java_switch_block(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The switch block is one scope; case labels and rules share it.
    scopes.push(Vec::new());
    walk_java_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_java_instanceof(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(left) = node.child_by_field_name("left") {
        walk_java_node(left, source, scopes, scan, deadline)?;
    }
    if let Some(pattern) = node.child_by_field_name("pattern") {
        // Record patterns and type patterns bind their component names.
        walk_java_node(pattern, source, scopes, scan, deadline)?;
    } else if let Some(name) = node.child_by_field_name("name") {
        // The older `x instanceof Foo f` form binds `f` as a pattern variable.
        bind_java_name(name, source, "type_pattern", scopes, scan)?;
    }
    // The right-hand type is a type spelling, not a value reference.
    Ok(())
}

fn walk_java_cast(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(value) = node.child_by_field_name("value") {
        walk_java_node(value, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_java_method_invocation(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // A qualified call records only the receiver expression; the member name is
    // not a value reference. A bare call records the callee name, which must
    // resolve to a visible member, static import, or predeclared name.
    if let Some(object) = node.child_by_field_name("object") {
        walk_java_node(object, source, scopes, scan, deadline)?;
    } else if let Some(name) = node.child_by_field_name("name") {
        walk_java_node(name, source, scopes, scan, deadline)?;
    }
    if let Some(arguments) = node.child_by_field_name("arguments") {
        walk_java_node(arguments, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_java_field_access(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(object) = node.child_by_field_name("object") {
        walk_java_node(object, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_java_object_creation(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            // The constructed type is a type spelling, not a value reference.
            "class_body" => {
                scopes.push(Vec::new());
                walk_java_node(child, source, scopes, scan, deadline)?;
                scopes.pop();
            }
            _ if is_java_type_kind(child.kind()) || child.kind() == "type_arguments" => {}
            _ => walk_java_node(child, source, scopes, scan, deadline)?,
        }
    }
    Ok(())
}

fn walk_java_method_reference(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Only the receiver side is a value reference; a type-qualified method
    // reference such as `Type::method` contributes no value reference, and the
    // method name after `::` is not validated.
    let mut cursor = node.walk();
    if let Some(first) = node.named_children(&mut cursor).next()
        && !is_java_type_kind(first.kind())
    {
        walk_java_node(first, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_java_labeled_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    let mut skipped_label = false;
    for child in node.named_children(&mut cursor) {
        if !skipped_label && child.kind() == "identifier" {
            skipped_label = true;
            continue;
        }
        walk_java_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_java_pattern_name(
    node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    scopes: &mut [Vec<JavaBinding>],
    scan: &mut JavaScopeScan,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "identifier" {
            bind_java_name(child, source, node_kind, scopes, scan)?;
        }
    }
    Ok(())
}

fn walk_java_enum_constant(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "argument_list" => walk_java_node(child, source, scopes, scan, deadline)?,
            "class_body" => {
                scopes.push(Vec::new());
                walk_java_node(child, source, scopes, scan, deadline)?;
                scopes.pop();
            }
            _ => {}
        }
    }
    Ok(())
}

fn walk_java_annotation_default(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<JavaBinding>>,
    scan: &mut JavaScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(value) = node.child_by_field_name("value") {
        walk_java_node(value, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn record_java_reference(
    node: Node<'_>,
    source: &str,
    scopes: &mut [Vec<JavaBinding>],
    scan: &mut JavaScopeScan,
) -> Result<()> {
    let name = node_text(node, source)?.trim().to_string();
    if name.is_empty() || name == "_" {
        return Ok(());
    }
    let visible = scopes
        .iter()
        .rev()
        .any(|scope| scope.iter().any(|binding| binding.name == name));
    if visible {
        scan.local_references.insert(name);
    } else {
        scan.external_references.insert(name);
    }
    Ok(())
}

fn collect_java_function_bindings(
    node: Node<'_>,
    source: &str,
    out: &mut Vec<JavaBinding>,
    scan: &mut JavaScopeScan,
) -> Result<()> {
    if let Some(parameters) = node.child_by_field_name("parameters") {
        bind_java_parameter_list(parameters, source, "formal_parameter", out, scan)?;
    }
    Ok(())
}

fn bind_java_parameter_list(
    parameters: Node<'_>,
    source: &str,
    node_kind: &'static str,
    out: &mut Vec<JavaBinding>,
    scan: &mut JavaScopeScan,
) -> Result<()> {
    let mut cursor = parameters.walk();
    for child in parameters.named_children(&mut cursor) {
        match child.kind() {
            "formal_parameter" => {
                if let Some(name) = child.child_by_field_name("name") {
                    bind_java_name_into(name, source, node_kind, out, scan)?;
                }
            }
            "spread_parameter" => {
                let mut declarator_cursor = child.walk();
                if let Some(declarator) = child
                    .named_children(&mut declarator_cursor)
                    .find(|candidate| candidate.kind() == "variable_declarator")
                    && let Some(name) = declarator.child_by_field_name("name")
                {
                    bind_java_name_into(name, source, node_kind, out, scan)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn bind_java_inferred_parameters(
    parameters: Node<'_>,
    source: &str,
    out: &mut Vec<JavaBinding>,
    scan: &mut JavaScopeScan,
) -> Result<()> {
    let mut cursor = parameters.walk();
    for child in parameters.named_children(&mut cursor) {
        if child.kind() == "identifier" {
            bind_java_name_into(child, source, "inferred_parameters", out, scan)?;
        }
    }
    Ok(())
}

fn collect_java_record_components(
    node: Node<'_>,
    source: &str,
    out: &mut Vec<JavaBinding>,
    scan: &mut JavaScopeScan,
) -> Result<()> {
    if let Some(parameters) = node.child_by_field_name("parameters") {
        bind_java_parameter_list(parameters, source, "record_component", out, scan)?;
    }
    Ok(())
}

fn bind_java_name(
    name_node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    scopes: &mut [Vec<JavaBinding>],
    scan: &mut JavaScopeScan,
) -> Result<()> {
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() || name == "_" {
        return Ok(());
    }
    let binding = JavaBinding {
        name: name.clone(),
        node_kind,
        start_byte: name_node.start_byte(),
        end_byte: name_node.end_byte(),
    };
    scan.local_bindings.push(binding.clone());
    if let Some(scope) = scopes.last_mut() {
        scope.push(binding);
    }
    Ok(())
}

fn bind_java_name_into(
    name_node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    out: &mut Vec<JavaBinding>,
    scan: &mut JavaScopeScan,
) -> Result<()> {
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() || name == "_" {
        return Ok(());
    }
    let binding = JavaBinding {
        name: name.clone(),
        node_kind,
        start_byte: name_node.start_byte(),
        end_byte: name_node.end_byte(),
    };
    scan.local_bindings.push(binding.clone());
    out.push(binding);
    Ok(())
}

fn is_java_type_kind(kind: &str) -> bool {
    matches!(
        kind,
        "type_identifier"
            | "scoped_type_identifier"
            | "generic_type"
            | "array_type"
            | "annotated_type"
            | "integral_type"
            | "floating_point_type"
            | "boolean_type"
            | "void_type"
            | "wildcard"
            | "dimensions"
    )
}

fn is_ignored_java_node_kind(kind: &str) -> bool {
    is_java_type_kind(kind)
        || matches!(
            kind,
            // Type-adjacent spellings that never name a value.
            "type_arguments"
                | "type_parameter"
                | "type_parameters"
                | "type_bound"
                | "superclass"
                | "super_interfaces"
                | "extends_interfaces"
                | "type_list"
                | "permits"
                | "throws"
                | "catch_type"
                | "class_literal"
                // Parameter plumbing is consumed by binding helpers.
                | "formal_parameters"
                | "formal_parameter"
                | "spread_parameter"
                | "receiver_parameter"
                | "inferred_parameters"
                | "catch_formal_parameter"
                | "variable_declarator_id"
                // Name and label spellings are not value references.
                | "scoped_identifier"
                | "modifiers"
                | "this"
                | "super"
                // Annotations and their arguments are not value references.
                | "marker_annotation"
                | "annotation"
                | "annotation_argument_list"
                | "element_value_pair"
                | "element_value_array_initializer"
                // Package, import, and module plumbing never appears in bodies.
                | "package_declaration"
                | "import_declaration"
                | "module_declaration"
                | "module_body"
                | "module_directive"
                | "requires_module_directive"
                | "exports_module_directive"
                | "opens_module_directive"
                | "uses_module_directive"
                | "provides_module_directive"
                | "requires_modifier"
        )
}
