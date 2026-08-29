use std::collections::BTreeSet;

use anyhow::Result;
use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::language::node_text;

#[derive(Debug, Clone)]
pub(super) struct KotlinBinding {
    pub(super) name: String,
    pub(super) node_kind: &'static str,
    pub(super) start_byte: usize,
    pub(super) end_byte: usize,
}

pub(super) struct KotlinScopeScan {
    pub(super) local_bindings: Vec<KotlinBinding>,
    pub(super) local_references: BTreeSet<String>,
    pub(super) external_references: BTreeSet<String>,
}

/// Walks the patched Kotlin symbol node and classifies identifier references
/// into locally visible bindings (function parameters, primary-constructor
/// class parameters, local `val`/`var` properties including destructured
/// declarations, `for`-loop variables, lambda and anonymous-function
/// parameters including the implicit `it` lambda parameter, catch parameters,
/// and setter parameters) versus names that must resolve at file scope
/// (same-file declarations and imports). Type spellings, member names, labels,
/// annotation names, and `this`/`super` receivers are not value references
/// and are skipped.
pub(super) fn scan_kotlin_symbol_scope(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<KotlinScopeScan> {
    let mut scan = KotlinScopeScan {
        local_bindings: Vec::new(),
        local_references: BTreeSet::new(),
        external_references: BTreeSet::new(),
    };
    let mut scopes: Vec<Vec<KotlinBinding>> = Vec::new();

    match symbol_node.kind() {
        "function_declaration" | "anonymous_function" => {
            let mut function_scope = Vec::new();
            collect_kotlin_function_bindings(symbol_node, source, &mut function_scope, &mut scan)?;
            scopes.push(function_scope);
            if let Some(body) = kotlin_function_body(symbol_node) {
                walk_kotlin_node(body, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        "secondary_constructor" => {
            let mut function_scope = Vec::new();
            collect_kotlin_function_bindings(symbol_node, source, &mut function_scope, &mut scan)?;
            scopes.push(function_scope);
            walk_kotlin_children(symbol_node, source, &mut scopes, &mut scan, deadline)?;
        }
        // A patched type declaration validates references in field
        // initializers, method bodies, and nested declarations; its own name,
        // type parameters, and delegation specifiers are not value references.
        "class_declaration" => {
            let mut class_scope = Vec::new();
            collect_kotlin_class_parameters(symbol_node, source, &mut class_scope, &mut scan)?;
            scopes.push(class_scope);
            if let Some(body) = kotlin_class_body(symbol_node) {
                walk_kotlin_node(body, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        "object_declaration" | "companion_object" => {
            scopes.push(Vec::new());
            if let Some(body) = kotlin_class_body(symbol_node) {
                walk_kotlin_node(body, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        _ => {
            walk_kotlin_children(symbol_node, source, &mut scopes, &mut scan, deadline)?;
        }
    }

    Ok(scan)
}

fn walk_kotlin_node(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning Kotlin patch references")?;
    }
    match node.kind() {
        "identifier" => record_kotlin_reference(node, source, scopes, scan),
        "block" => walk_kotlin_block(node, source, scopes, scan, deadline),
        "function_declaration" | "anonymous_function" => {
            walk_kotlin_function_declaration(node, source, scopes, scan, deadline)
        }
        "secondary_constructor" => {
            walk_kotlin_secondary_constructor(node, source, scopes, scan, deadline)
        }
        "class_declaration" => walk_kotlin_class_declaration(node, source, scopes, scan, deadline),
        "object_declaration" | "companion_object" => {
            walk_kotlin_object_declaration(node, source, scopes, scan, deadline)
        }
        "object_literal" => walk_kotlin_object_literal(node, source, scopes, scan, deadline),
        "property_declaration" => {
            walk_kotlin_property_declaration(node, source, scopes, scan, deadline)
        }
        "lambda_literal" => walk_kotlin_lambda(node, source, scopes, scan, deadline),
        "for_statement" => walk_kotlin_for_statement(node, source, scopes, scan, deadline),
        "catch_block" => walk_kotlin_catch_block(node, source, scopes, scan, deadline),
        "setter" => walk_kotlin_setter(node, source, scopes, scan, deadline),
        "navigation_expression" => walk_kotlin_navigation(node, source, scopes, scan, deadline),
        "callable_reference" => {
            walk_kotlin_callable_reference(node, source, scopes, scan, deadline)
        }
        "as_expression" | "is_expression" => {
            walk_kotlin_type_test_expression(node, source, scopes, scan, deadline)
        }
        "type_test" => Ok(()),
        "labeled_expression" => {
            walk_kotlin_labeled_expression(node, source, scopes, scan, deadline)
        }
        "return_expression" => walk_kotlin_return(node, source, scopes, scan, deadline),
        "constructor_invocation" | "constructor_delegation_call" => {
            walk_kotlin_constructor_invocation(node, source, scopes, scan, deadline)
        }
        "getter" => walk_kotlin_children(node, source, scopes, scan, deadline),
        _ => {
            if is_ignored_kotlin_node_kind(node.kind()) {
                Ok(())
            } else {
                walk_kotlin_children(node, source, scopes, scan, deadline)
            }
        }
    }
}

fn walk_kotlin_children(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_kotlin_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_kotlin_block(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Vec::new());
    walk_kotlin_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_kotlin_function_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // A named local function is visible after its declaration in the
    // enclosing lexical scope and inside its own body for recursion. Bind it
    // before entering its parameter scope so both contexts can resolve it.
    if let Some(name) = node.child_by_field_name("name") {
        bind_kotlin_name(name, source, "local_function", scopes, scan)?;
    }
    let mut function_scope = Vec::new();
    collect_kotlin_function_bindings(node, source, &mut function_scope, scan)?;
    scopes.push(function_scope);
    if let Some(body) = kotlin_function_body(node) {
        walk_kotlin_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_kotlin_secondary_constructor(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut function_scope = Vec::new();
    collect_kotlin_function_bindings(node, source, &mut function_scope, scan)?;
    scopes.push(function_scope);
    walk_kotlin_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_kotlin_class_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut class_scope = Vec::new();
    collect_kotlin_class_parameters(node, source, &mut class_scope, scan)?;
    scopes.push(class_scope);
    if let Some(body) = kotlin_class_body(node) {
        walk_kotlin_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_kotlin_object_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Vec::new());
    if let Some(body) = kotlin_class_body(node) {
        walk_kotlin_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_kotlin_object_literal(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Vec::new());
    walk_kotlin_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_kotlin_property_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Initializers, delegates, and accessor bodies are evaluated before the
    // declared name becomes visible, so walk every child except the
    // declaration itself first, then bind the name(s).
    let mut cursor = node.walk();
    let mut bindings = Vec::new();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "variable_declaration" => bindings.push(child),
            "multi_variable_declaration" => bindings.push(child),
            _ => walk_kotlin_node(child, source, scopes, scan, deadline)?,
        }
    }
    for declaration in bindings {
        collect_kotlin_variable_bindings(declaration, source, scopes, scan)?;
    }
    Ok(())
}

fn walk_kotlin_lambda(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut lambda_scope = Vec::new();
    let mut parameters: Option<Node<'_>> = None;
    let mut body_children = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "lambda_parameters" {
            parameters = Some(child);
        } else {
            body_children.push(child);
        }
    }
    match parameters {
        Some(parameters) => {
            collect_kotlin_lambda_parameters(parameters, source, &mut lambda_scope, scan)?;
        }
        None => bind_kotlin_implicit_it(&mut lambda_scope, scan),
    }
    scopes.push(lambda_scope);
    for child in body_children {
        walk_kotlin_node(child, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_kotlin_for_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The iterable expression is evaluated in the enclosing scope; the loop
    // variables are scoped to the loop body.
    let mut cursor = node.walk();
    let mut loop_bindings = Vec::new();
    let mut body: Option<Node<'_>> = None;
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "variable_declaration" | "multi_variable_declaration" => loop_bindings.push(child),
            "block" => body = Some(child),
            _ => walk_kotlin_node(child, source, scopes, scan, deadline)?,
        }
    }
    scopes.push(Vec::new());
    for declaration in loop_bindings {
        collect_kotlin_variable_bindings(declaration, source, scopes, scan)?;
    }
    if let Some(body) = body {
        walk_kotlin_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_kotlin_catch_block(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Vec::new());
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "identifier" {
            bind_kotlin_name(child, source, "catch_parameter", scopes, scan)?;
        } else {
            walk_kotlin_node(child, source, scopes, scan, deadline)?;
        }
    }
    scopes.pop();
    Ok(())
}

fn walk_kotlin_setter(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Vec::new());
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "identifier" {
            bind_kotlin_name(child, source, "setter_parameter", scopes, scan)?;
        } else {
            walk_kotlin_node(child, source, scopes, scan, deadline)?;
        }
    }
    scopes.pop();
    Ok(())
}

fn walk_kotlin_navigation(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    let Some(receiver) = children.first().copied() else {
        return Ok(());
    };
    // A `::`-separated chain is a callable reference: `Type::member` where the
    // receiver is a type spelling, or `value::member` where the receiver is a
    // bound value. A leading identifier is treated as a value reference only
    // when it does not look like a type spelling (uppercase-initial), matching
    // Kotlin naming conventions; other receiver shapes keep their value walk.
    if children.len() >= 2
        && source
            .get(receiver.end_byte()..children[1].start_byte())
            .is_some_and(|separator| separator.contains("::"))
    {
        if receiver.kind() == "identifier" {
            let text = node_text(receiver, source)?.trim().to_string();
            if !text.is_empty() && kotlin_is_value_like_identifier(&text) {
                walk_kotlin_node(receiver, source, scopes, scan, deadline)?;
            }
            return Ok(());
        }
        walk_kotlin_node(receiver, source, scopes, scan, deadline)?;
        return Ok(());
    }
    // Plain member access: the receiver is a value expression; the trailing
    // identifiers are member names, not value references.
    walk_kotlin_node(receiver, source, scopes, scan, deadline)?;
    Ok(())
}

fn walk_kotlin_callable_reference(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // A leading-`::` reference such as `::increment` or `::Foo` names the
    // referenced callable; every identifier child is a value reference.
    walk_kotlin_children(node, source, scopes, scan, deadline)
}

fn walk_kotlin_type_test_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // `value as Type` and `value is Type`: only the left side is a value
    // reference; the right-hand type is a type spelling.
    if let Some(left) = node.child_by_field_name("left") {
        walk_kotlin_node(left, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_kotlin_labeled_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "label" {
            continue;
        }
        walk_kotlin_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_kotlin_return(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let label = node.child_by_field_name("label");
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if label.is_some_and(|label| label == child) {
            continue;
        }
        walk_kotlin_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_kotlin_constructor_invocation(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<KotlinBinding>>,
    scan: &mut KotlinScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Only the argument list is a value context; the constructed type is a
    // type spelling.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "value_arguments" {
            walk_kotlin_node(child, source, scopes, scan, deadline)?;
        }
    }
    Ok(())
}

fn collect_kotlin_function_bindings(
    node: Node<'_>,
    source: &str,
    out: &mut Vec<KotlinBinding>,
    scan: &mut KotlinScopeScan,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "function_value_parameters" {
            let mut parameter_cursor = child.walk();
            for parameter in child.named_children(&mut parameter_cursor) {
                if parameter.kind() == "parameter"
                    && let Some(name) = kotlin_parameter_name_node(parameter)
                {
                    bind_kotlin_name_into(name, source, "parameter", out, scan)?;
                }
            }
        }
    }
    Ok(())
}

fn collect_kotlin_class_parameters(
    node: Node<'_>,
    source: &str,
    out: &mut Vec<KotlinBinding>,
    scan: &mut KotlinScopeScan,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "primary_constructor"
            && let Some(parameters) = kotlin_direct_child_by_kind(child, &["class_parameters"])
        {
            let mut parameter_cursor = parameters.walk();
            for parameter in parameters.named_children(&mut parameter_cursor) {
                if parameter.kind() == "class_parameter"
                    && let Some(name) = kotlin_parameter_name_node(parameter)
                {
                    bind_kotlin_name_into(name, source, "class_parameter", out, scan)?;
                }
            }
        }
    }
    Ok(())
}

fn collect_kotlin_variable_bindings(
    node: Node<'_>,
    source: &str,
    scopes: &mut [Vec<KotlinBinding>],
    scan: &mut KotlinScopeScan,
) -> Result<()> {
    if node.kind() == "variable_declaration" {
        if let Some(name) = kotlin_parameter_name_node(node) {
            bind_kotlin_name(name, source, "variable_declaration", scopes, scan)?;
        }
        return Ok(());
    }
    if node.kind() == "multi_variable_declaration" {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "variable_declaration"
                && let Some(name) = kotlin_parameter_name_node(child)
            {
                bind_kotlin_name(name, source, "variable_declaration", scopes, scan)?;
            }
        }
    }
    Ok(())
}

fn collect_kotlin_lambda_parameters(
    node: Node<'_>,
    source: &str,
    out: &mut Vec<KotlinBinding>,
    scan: &mut KotlinScopeScan,
) -> Result<()> {
    let mut explicit = 0usize;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "variable_declaration" {
            continue;
        }
        explicit += 1;
        if let Some(name) = kotlin_parameter_name_node(child) {
            let text = node_text(name, source)?.trim().to_string();
            if text != "_" {
                bind_kotlin_name_into(name, source, "lambda_parameter", out, scan)?;
            }
        }
    }
    // With exactly one unused `_` parameter (or no explicit parameter list),
    // Kotlin still provides the implicit `it` parameter.
    if explicit == 1 {
        bind_kotlin_implicit_it(out, scan);
    }
    Ok(())
}

fn bind_kotlin_implicit_it(out: &mut Vec<KotlinBinding>, scan: &mut KotlinScopeScan) {
    let binding = KotlinBinding {
        name: "it".to_string(),
        node_kind: "lambda_parameter",
        start_byte: 0,
        end_byte: 0,
    };
    scan.local_bindings.push(binding.clone());
    out.push(binding);
}

fn record_kotlin_reference(
    node: Node<'_>,
    source: &str,
    scopes: &mut [Vec<KotlinBinding>],
    scan: &mut KotlinScopeScan,
) -> Result<()> {
    let name = node_text(node, source)?.trim().to_string();
    if name.is_empty() || name == "_" || is_kotlin_keyword_like_identifier(&name) {
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

fn bind_kotlin_name(
    name_node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    scopes: &mut [Vec<KotlinBinding>],
    scan: &mut KotlinScopeScan,
) -> Result<()> {
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() || name == "_" {
        return Ok(());
    }
    let binding = KotlinBinding {
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

fn bind_kotlin_name_into(
    name_node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    out: &mut Vec<KotlinBinding>,
    scan: &mut KotlinScopeScan,
) -> Result<()> {
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() || name == "_" {
        return Ok(());
    }
    let binding = KotlinBinding {
        name: name.clone(),
        node_kind,
        start_byte: name_node.start_byte(),
        end_byte: name_node.end_byte(),
    };
    scan.local_bindings.push(binding.clone());
    out.push(binding);
    Ok(())
}

fn kotlin_parameter_name_node(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "identifier")
}

fn kotlin_function_body<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "function_body")
}

fn kotlin_class_body<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| matches!(child.kind(), "class_body" | "enum_class_body"))
}

fn kotlin_direct_child_by_kind<'tree>(node: Node<'tree>, kinds: &[&str]) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| kinds.contains(&child.kind()))
}

/// A value-like leading identifier in a callable reference such as
/// `value::member` is lowercase-initial; type spellings follow the
/// uppercase-initial Kotlin convention and are skipped.
fn kotlin_is_value_like_identifier(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|first| !first.is_uppercase())
}

fn is_kotlin_keyword_like_identifier(name: &str) -> bool {
    matches!(
        name,
        "break"
            | "continue"
            | "return"
            | "this"
            | "super"
            | "null"
            | "true"
            | "false"
            | "is"
            | "in"
            | "as"
            | "when"
            | "if"
            | "else"
            | "for"
            | "while"
            | "do"
            | "fun"
            | "val"
            | "var"
            | "class"
            | "object"
            | "companion"
            | "try"
            | "catch"
            | "finally"
            | "throw"
            | "typealias"
            | "interface"
            | "enum"
            | "import"
            | "package"
            | "internal"
            | "public"
            | "private"
            | "protected"
            | "override"
            | "open"
            | "abstract"
            | "final"
            | "sealed"
            | "data"
            | "inner"
            | "lateinit"
            | "by"
            | "get"
            | "set"
            | "init"
            | "constructor"
            | "where"
            | "vararg"
            | "noinline"
            | "crossinline"
            | "reified"
            | "tailrec"
            | "operator"
            | "infix"
            | "external"
            | "const"
            | "suspend"
            | "annotation"
            | "actual"
            | "expect"
    )
}

fn is_kotlin_type_kind(kind: &str) -> bool {
    matches!(
        kind,
        "user_type"
            | "nullable_type"
            | "non_nullable_type"
            | "parenthesized_type"
            | "function_type"
            | "function_type_parameters"
            | "type_arguments"
            | "type_projection"
            | "type_parameter"
            | "type_parameters"
            | "type_constraint"
            | "type_constraints"
            | "type_modifiers"
            | "type_parameter_modifiers"
            | "type"
    )
}

fn is_ignored_kotlin_node_kind(kind: &str) -> bool {
    is_kotlin_type_kind(kind)
        || matches!(
            kind,
            // Modifier and annotation spellings are not value references.
            "modifiers"
                | "class_modifier"
                | "function_modifier"
                | "property_modifier"
                | "member_modifier"
                | "visibility_modifier"
                | "parameter_modifier"
                | "parameter_modifiers"
                | "inheritance_modifier"
                | "platform_modifier"
                | "reification_modifier"
                | "variance_modifier"
                | "use_site_target"
                | "annotation"
                | "file_annotation"
                | "unescaped_annotation"
                // Declaration plumbing is consumed by binding helpers or is
                // not a value reference.
                | "class_parameters"
                | "class_parameter"
                | "primary_constructor"
                | "function_value_parameters"
                | "parameter"
                | "variable_declaration"
                | "multi_variable_declaration"
                | "lambda_parameters"
                | "enum_entry"
                // Package, import, and type spelling plumbing.
                | "package_header"
                | "import"
                | "qualified_identifier"
                // Labels, receivers, and literal internals.
                | "label"
                | "this_expression"
                | "super_expression"
                | "string_content"
                | "escape_sequence"
                | "line_comment"
                | "shebang"
                // Abstract grammar wrappers.
                | "source_file"
                | "statement"
                | "declaration"
                | "expression"
                | "primary_expression"
        )
}
