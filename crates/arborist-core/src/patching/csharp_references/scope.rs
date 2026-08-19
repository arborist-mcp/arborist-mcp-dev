use std::collections::BTreeSet;

use anyhow::Result;
use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::language::node_text;

#[derive(Debug, Clone)]
pub(super) struct CSharpBinding {
    pub(super) name: String,
    pub(super) node_kind: &'static str,
    pub(super) start_byte: usize,
    pub(super) end_byte: usize,
}

pub(super) struct CSharpScopeScan {
    pub(super) local_bindings: Vec<CSharpBinding>,
    pub(super) local_references: BTreeSet<String>,
    pub(super) external_references: BTreeSet<String>,
}

/// Walks the patched C# symbol node and classifies identifier references into
/// locally visible bindings (formal parameters, local declarators, `for` and
/// `foreach` variables, catch parameters, using-resource variables, lambda and
/// anonymous-method parameters, local functions and their parameters, pattern
/// variables, out-variables, and query variables) versus names that must
/// resolve at file scope (same-file declarations and aliased using
/// directives). Type spellings, member names, labels, attribute names, and
/// `nameof`/`typeof`/`sizeof` arguments are not value references and are
/// skipped.
pub(super) fn scan_csharp_symbol_scope(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<CSharpScopeScan> {
    let mut scan = CSharpScopeScan {
        local_bindings: Vec::new(),
        local_references: BTreeSet::new(),
        external_references: BTreeSet::new(),
    };
    let mut scopes: Vec<Vec<CSharpBinding>> = Vec::new();

    match symbol_node.kind() {
        "method_declaration"
        | "constructor_declaration"
        | "destructor_declaration"
        | "operator_declaration"
        | "conversion_operator_declaration" => {
            let mut function_scope = Vec::new();
            collect_csharp_function_bindings(symbol_node, source, &mut function_scope, &mut scan)?;
            scopes.push(function_scope);
            walk_csharp_function_body(symbol_node, source, &mut scopes, &mut scan, deadline)?;
        }
        // A patched type declaration validates references in field
        // initializers, method bodies, and nested declarations; its own name,
        // type parameters, base list, and interface list are not value
        // references. Record components are visible to the whole record.
        "class_declaration"
        | "struct_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration" => {
            let mut class_scope = Vec::new();
            if symbol_node.kind() == "record_declaration"
                && let Some(parameters) = csharp_record_parameter_list(symbol_node)
            {
                bind_csharp_parameter_list(
                    parameters,
                    source,
                    "record_parameter",
                    &mut class_scope,
                    &mut scan,
                )?;
            }
            scopes.push(class_scope);
            if let Some(body) = symbol_node.child_by_field_name("body") {
                walk_csharp_node(body, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        _ => {
            let mut cursor = symbol_node.walk();
            for child in symbol_node.named_children(&mut cursor) {
                walk_csharp_node(child, source, &mut scopes, &mut scan, deadline)?;
            }
        }
    }

    Ok(scan)
}

fn walk_csharp_node(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning C# patch references")?;
    }
    match node.kind() {
        "identifier" => record_csharp_reference(node, source, scopes, scan),
        "block" => walk_csharp_block(node, source, scopes, scan, deadline),
        "method_declaration"
        | "constructor_declaration"
        | "destructor_declaration"
        | "operator_declaration"
        | "conversion_operator_declaration" => {
            walk_csharp_function_declaration(node, source, scopes, scan, deadline)
        }
        "class_declaration"
        | "struct_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration" => {
            walk_csharp_type_declaration(node, source, scopes, scan, deadline)
        }
        "local_function_statement" => {
            walk_csharp_local_function(node, source, scopes, scan, deadline)
        }
        "property_declaration" | "event_declaration" | "event_field_declaration" => {
            walk_csharp_property_declaration(node, source, scopes, scan, deadline)
        }
        "indexer_declaration" => {
            walk_csharp_indexer_declaration(node, source, scopes, scan, deadline)
        }
        "accessor_declaration" => {
            walk_csharp_accessor_declaration(node, source, scopes, scan, deadline)
        }
        "variable_declaration" => {
            walk_csharp_variable_declaration(node, source, scopes, scan, deadline)
        }
        "variable_declarator" => {
            walk_csharp_variable_declarator(node, source, scopes, scan, deadline)
        }
        "for_statement" => walk_csharp_for_statement(node, source, scopes, scan, deadline),
        "foreach_statement" => walk_csharp_foreach_statement(node, source, scopes, scan, deadline),
        "catch_clause" => walk_csharp_catch_clause(node, source, scopes, scan, deadline),
        "catch_declaration" => walk_csharp_catch_declaration(node, source, scopes, scan, deadline),
        "using_statement" => walk_csharp_using_statement(node, source, scopes, scan, deadline),
        "lambda_expression" => walk_csharp_lambda_expression(node, source, scopes, scan, deadline),
        "anonymous_method_expression" => {
            walk_csharp_anonymous_method(node, source, scopes, scan, deadline)
        }
        "declaration_pattern" | "var_pattern" => {
            walk_csharp_pattern_variable(node, source, "pattern_variable", scopes, scan, deadline)
        }
        "recursive_pattern" => walk_csharp_recursive_pattern(node, source, scopes, scan, deadline),
        "list_pattern" => walk_csharp_list_pattern(node, source, scopes, scan, deadline),
        "tuple_pattern" => walk_csharp_tuple_pattern(node, source, scopes, scan, deadline),
        "parenthesized_variable_designation" => {
            walk_csharp_parenthesized_designation(node, source, scopes, scan, deadline)
        }
        "subpattern" => walk_csharp_subpattern(node, source, scopes, scan, deadline),
        "type_pattern" => Ok(()),
        "is_pattern_expression" => walk_csharp_is_pattern(node, source, scopes, scan, deadline),
        "is_expression" | "as_expression" => {
            walk_csharp_is_as(node, source, scopes, scan, deadline)
        }
        "cast_expression" => walk_csharp_cast(node, source, scopes, scan, deadline),
        "member_access_expression" => {
            walk_csharp_member_access(node, source, scopes, scan, deadline)
        }
        "member_binding_expression" => Ok(()),
        "conditional_access_expression" => {
            walk_csharp_conditional_access(node, source, scopes, scan, deadline)
        }
        "invocation_expression" => walk_csharp_invocation(node, source, scopes, scan, deadline),
        "object_creation_expression" | "implicit_object_creation_expression" => {
            walk_csharp_object_creation(node, source, scopes, scan, deadline)
        }
        "array_creation_expression"
        | "implicit_array_creation_expression"
        | "implicit_stackalloc_expression" => {
            walk_csharp_array_creation(node, source, scopes, scan, deadline)
        }
        "anonymous_object_creation_expression" => {
            walk_csharp_anonymous_object(node, source, scopes, scan, deadline)
        }
        "initializer_expression" => walk_csharp_initializer(node, source, scopes, scan, deadline),
        "with_expression" => walk_csharp_with_expression(node, source, scopes, scan, deadline),
        "with_initializer" => walk_csharp_with_initializer(node, source, scopes, scan, deadline),
        "argument" => walk_csharp_argument(node, source, scopes, scan, deadline),
        "declaration_expression" => {
            walk_csharp_declaration_expression(node, source, scopes, scan, deadline)
        }
        "parameter" | "parameter_array" => {
            walk_csharp_parameter(node, source, scopes, scan, deadline)
        }
        "enum_member_declaration" => walk_csharp_enum_member(node, source, scopes, scan, deadline),
        "switch_body" => walk_csharp_switch_body(node, source, scopes, scan, deadline),
        "labeled_statement" => walk_csharp_labeled_statement(node, source, scopes, scan, deadline),
        "typeof_expression" | "sizeof_expression" | "default_expression" => Ok(()),
        "query_expression" => walk_csharp_query_expression(node, source, scopes, scan, deadline),
        "from_clause" => walk_csharp_from_clause(node, source, scopes, scan, deadline),
        "let_clause" => walk_csharp_let_clause(node, source, scopes, scan, deadline),
        "join_clause" => walk_csharp_join_clause(node, source, scopes, scan, deadline),
        "join_into_clause" => walk_csharp_join_into_clause(node, source, scopes, scan, deadline),
        "break_statement" | "continue_statement" | "goto_statement" | "empty_statement" => Ok(()),
        _ => {
            if is_ignored_csharp_node_kind(node.kind()) {
                Ok(())
            } else {
                walk_csharp_children(node, source, scopes, scan, deadline)
            }
        }
    }
}

fn walk_csharp_children(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_csharp_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_block(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Vec::new());
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_csharp_node(child, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_csharp_function_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut function_scope = Vec::new();
    collect_csharp_function_bindings(node, source, &mut function_scope, scan)?;
    scopes.push(function_scope);
    walk_csharp_function_body(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_csharp_function_body(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(body) = node.child_by_field_name("body") {
        walk_csharp_node(body, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_type_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut class_scope = Vec::new();
    if node.kind() == "record_declaration"
        && let Some(parameters) = csharp_record_parameter_list(node)
    {
        bind_csharp_parameter_list(
            parameters,
            source,
            "record_parameter",
            &mut class_scope,
            scan,
        )?;
    }
    scopes.push(class_scope);
    if let Some(body) = node.child_by_field_name("body") {
        walk_csharp_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_csharp_local_function(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The local function name is visible in the enclosing block from its
    // declaration point, including inside its own body.
    if let Some(name) = node.child_by_field_name("name") {
        bind_csharp_name(name, source, "local_function", scopes, scan)?;
    }
    let mut function_scope = Vec::new();
    if let Some(parameters) = node.child_by_field_name("parameters") {
        bind_csharp_parameter_list(
            parameters,
            source,
            "formal_parameter",
            &mut function_scope,
            scan,
        )?;
    }
    scopes.push(function_scope);
    walk_csharp_function_body(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_csharp_property_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The property, event, or event-field name and its type are not value
    // references; only accessor bodies, initializers, and declarators are
    // walked.
    let name_field = node.child_by_field_name("name");
    let type_field = node.child_by_field_name("type");
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if name_field.is_some_and(|field| field.id() == child.id())
            || type_field.is_some_and(|field| field.id() == child.id())
        {
            continue;
        }
        walk_csharp_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_indexer_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut indexer_scope = Vec::new();
    if let Some(parameters) = node.child_by_field_name("parameters") {
        bind_csharp_parameter_list(
            parameters,
            source,
            "indexer_parameter",
            &mut indexer_scope,
            scan,
        )?;
    }
    scopes.push(indexer_scope);
    if let Some(accessors) = node.child_by_field_name("accessors") {
        walk_csharp_node(accessors, source, scopes, scan, deadline)?;
    }
    if let Some(value) = node.child_by_field_name("value") {
        walk_csharp_node(value, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_csharp_accessor_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Vec::new());
    // The `value` keyword is implicitly in scope inside `set`, `init`, `add`,
    // and `remove` accessor bodies.
    let has_value_parameter = node
        .child_by_field_name("name")
        .and_then(|name| node_text(name, source).ok())
        .is_some_and(|name| matches!(name.trim(), "set" | "init" | "add" | "remove"));
    if has_value_parameter {
        let binding = CSharpBinding {
            name: "value".to_string(),
            node_kind: "implicit_value",
            start_byte: node.start_byte(),
            end_byte: node.start_byte(),
        };
        scan.local_bindings.push(binding.clone());
        if let Some(scope) = scopes.last_mut() {
            scope.push(binding);
        }
    }
    walk_csharp_function_body(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_csharp_variable_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The declared type is not a value reference; only the declarators matter.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            walk_csharp_node(child, source, scopes, scan, deadline)?;
        }
    }
    Ok(())
}

fn walk_csharp_variable_declarator(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The declared name becomes visible only after the initializer has run; a
    // tuple deconstruction binds each component name instead.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "identifier" | "bracketed_argument_list" => {}
            _ => walk_csharp_node(child, source, scopes, scan, deadline)?,
        }
    }
    if let Some(name) = node.child_by_field_name("name")
        && name.kind() == "identifier"
    {
        bind_csharp_name(name, source, "variable_declarator", scopes, scan)?;
    }
    Ok(())
}

fn walk_csharp_for_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The init variables are scoped to the whole `for` statement including the
    // condition, update, and body.
    scopes.push(Vec::new());
    walk_csharp_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_csharp_foreach_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The iterable expression is evaluated in the enclosing scope.
    if let Some(right) = node.child_by_field_name("right") {
        walk_csharp_node(right, source, scopes, scan, deadline)?;
    }
    scopes.push(Vec::new());
    if let Some(left) = node.child_by_field_name("left") {
        match left.kind() {
            "identifier" => bind_csharp_name(left, source, "foreach_variable", scopes, scan)?,
            _ => walk_csharp_node(left, source, scopes, scan, deadline)?,
        }
    }
    if let Some(body) = node.child_by_field_name("body") {
        walk_csharp_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_csharp_catch_clause(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The catch parameter, filter, and body share one scope.
    scopes.push(Vec::new());
    walk_csharp_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_csharp_catch_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut [Vec<CSharpBinding>],
    scan: &mut CSharpScopeScan,
    _deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The exception type is not a value reference; the optional variable name
    // is bound for the catch body.
    if let Some(name) = node.child_by_field_name("name") {
        bind_csharp_name(name, source, "catch_parameter", scopes, scan)?;
    }
    Ok(())
}

fn walk_csharp_using_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The using-resource variable is visible throughout the statement,
    // including the body.
    scopes.push(Vec::new());
    walk_csharp_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_csharp_lambda_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut lambda_scope = Vec::new();
    if let Some(parameters) = node.child_by_field_name("parameters") {
        match parameters.kind() {
            "parameter_list" => bind_csharp_parameter_list(
                parameters,
                source,
                "lambda_parameter",
                &mut lambda_scope,
                scan,
            )?,
            "implicit_parameter" => bind_csharp_name_into(
                parameters,
                source,
                "lambda_parameter",
                &mut lambda_scope,
                scan,
            )?,
            _ => {}
        }
    }
    scopes.push(lambda_scope);
    if let Some(body) = node.child_by_field_name("body") {
        walk_csharp_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_csharp_anonymous_method(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut lambda_scope = Vec::new();
    if let Some(parameters) = node.child_by_field_name("parameters") {
        bind_csharp_parameter_list(
            parameters,
            source,
            "lambda_parameter",
            &mut lambda_scope,
            scan,
        )?;
    }
    scopes.push(lambda_scope);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "block" {
            walk_csharp_node(child, source, scopes, scan, deadline)?;
        }
    }
    scopes.pop();
    Ok(())
}

fn walk_csharp_pattern_variable(
    node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The matched type is not a value reference; the designation binds a
    // pattern variable (or a parenthesized designation binds several).
    if let Some(name) = node.child_by_field_name("name") {
        bind_csharp_name(name, source, node_kind, scopes, scan)?;
    } else {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "parenthesized_variable_designation" {
                walk_csharp_node(child, source, scopes, scan, deadline)?;
            }
        }
    }
    Ok(())
}

fn walk_csharp_recursive_pattern(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The matched type and any nested type spellings are not value references;
    // subpatterns walk their value sides and a trailing designation binds a
    // pattern variable.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "identifier" | "generic_name" | "qualified_name" | "discard" => {}
            _ => walk_csharp_node(child, source, scopes, scan, deadline)?,
        }
    }
    if let Some(name) = node.child_by_field_name("name") {
        bind_csharp_name(name, source, "pattern_variable", scopes, scan)?;
    }
    Ok(())
}

fn walk_csharp_list_pattern(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "discard" => {}
            _ => walk_csharp_node(child, source, scopes, scan, deadline)?,
        }
    }
    if let Some(name) = node.child_by_field_name("name") {
        bind_csharp_name(name, source, "pattern_variable", scopes, scan)?;
    }
    Ok(())
}

fn walk_csharp_tuple_pattern(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "identifier" => bind_csharp_name(child, source, "tuple_pattern", scopes, scan)?,
            "discard" => {}
            _ => walk_csharp_node(child, source, scopes, scan, deadline)?,
        }
    }
    Ok(())
}

fn walk_csharp_parenthesized_designation(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "identifier" => bind_csharp_name(child, source, "pattern_variable", scopes, scan)?,
            "discard" => {}
            _ => walk_csharp_node(child, source, scopes, scan, deadline)?,
        }
    }
    Ok(())
}

fn walk_csharp_subpattern(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // A property subpattern such as `{ X: pattern }` starts with the member
    // name, which is not a value reference.
    let mut cursor = node.walk();
    let named = node.named_children(&mut cursor).collect::<Vec<_>>();
    let skip_first = named.len() >= 2 && named[0].kind() == "identifier";
    for (index, child) in named.iter().enumerate() {
        if skip_first && index == 0 {
            continue;
        }
        walk_csharp_node(*child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_is_pattern(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(expression) = node.child_by_field_name("expression") {
        walk_csharp_node(expression, source, scopes, scan, deadline)?;
    }
    if let Some(pattern) = node.child_by_field_name("pattern") {
        walk_csharp_node(pattern, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_is_as(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Only the left operand is a value reference; the right side is a type
    // spelling.
    if let Some(left) = node.child_by_field_name("left") {
        walk_csharp_node(left, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_cast(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(value) = node.child_by_field_name("value") {
        walk_csharp_node(value, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_member_access(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Only the receiver expression is a value reference; the member name is
    // not. `this.X` and `base.X` have no receiver field and contribute
    // nothing.
    if let Some(expression) = node.child_by_field_name("expression") {
        walk_csharp_node(expression, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_conditional_access(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "member_binding_expression" {
            continue;
        }
        walk_csharp_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_invocation(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // A qualified call records only the receiver expression; the member name
    // is not a value reference. A bare call records the callee name, which
    // must resolve to a visible member or predeclared name. `nameof` arguments
    // are name spellings, not value references, so the whole invocation is
    // skipped.
    if let Some(function) = node.child_by_field_name("function") {
        let is_nameof = function.kind() == "identifier"
            && node_text(function, source).is_ok_and(|name| name.trim() == "nameof");
        if !is_nameof {
            walk_csharp_node(function, source, scopes, scan, deadline)?;
        }
    }
    if let Some(arguments) = node.child_by_field_name("arguments") {
        walk_csharp_node(arguments, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_object_creation(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The constructed type is a type spelling, not a value reference.
    if let Some(arguments) = node.child_by_field_name("arguments") {
        walk_csharp_node(arguments, source, scopes, scan, deadline)?;
    }
    if let Some(initializer) = node.child_by_field_name("initializer") {
        walk_csharp_node(initializer, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_array_creation(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if !is_ignored_csharp_node_kind(child.kind()) {
            walk_csharp_node(child, source, scopes, scan, deadline)?;
        }
    }
    Ok(())
}

fn walk_csharp_anonymous_object(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // `new { Member = expr }` walks only the value; a bare shorthand member is
    // a projection reference.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "assignment_expression" => {
                if let Some(right) = child.child_by_field_name("right") {
                    walk_csharp_node(right, source, scopes, scan, deadline)?;
                }
            }
            _ => walk_csharp_node(child, source, scopes, scan, deadline)?,
        }
    }
    Ok(())
}

fn walk_csharp_initializer(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Object initializer members (`{ Name = expr }`) walk only the value side;
    // collection and array initializers walk their element expressions.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "assignment_expression" => {
                if let Some(right) = child.child_by_field_name("right") {
                    walk_csharp_node(right, source, scopes, scan, deadline)?;
                }
            }
            _ => walk_csharp_node(child, source, scopes, scan, deadline)?,
        }
    }
    Ok(())
}

fn walk_csharp_with_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    walk_csharp_children(node, source, scopes, scan, deadline)
}

fn walk_csharp_with_initializer(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // `with { Member = expr }` skips the member name and walks the value.
    let mut cursor = node.walk();
    let mut skipped = false;
    for child in node.named_children(&mut cursor) {
        if !skipped && child.kind() == "identifier" {
            skipped = true;
            continue;
        }
        walk_csharp_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_argument(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // A named argument (`name: value`) skips the parameter name; `ref`, `out`,
    // and `in` keywords are not named children. A declaration expression binds
    // an out-variable.
    let name_field = node.child_by_field_name("name");
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if name_field.is_some_and(|name| name.id() == child.id()) {
            continue;
        }
        walk_csharp_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_declaration_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut [Vec<CSharpBinding>],
    scan: &mut CSharpScopeScan,
    _deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // `out var x` binds the variable into the enclosing scope.
    if let Some(name) = node.child_by_field_name("name") {
        bind_csharp_name(name, source, "declaration_expression", scopes, scan)?;
    }
    Ok(())
}

fn walk_csharp_parameter(
    node: Node<'_>,
    source: &str,
    scopes: &mut [Vec<CSharpBinding>],
    scan: &mut CSharpScopeScan,
    _deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Parameter plumbing reached outside the dedicated binding helpers binds
    // the parameter name into the current scope.
    if let Some(name) = node.child_by_field_name("name") {
        bind_csharp_name(name, source, "formal_parameter", scopes, scan)?;
    }
    Ok(())
}

fn walk_csharp_enum_member(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The member name is not a value reference; an explicit value expression
    // is walked.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "identifier" {
            continue;
        }
        walk_csharp_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_switch_body(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // All switch sections share one scope; pattern variables bound in one
    // section are conservatively visible to the whole switch body.
    scopes.push(Vec::new());
    walk_csharp_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_csharp_labeled_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    let mut skipped = false;
    for child in node.named_children(&mut cursor) {
        if !skipped && child.kind() == "identifier" {
            skipped = true;
            continue;
        }
        walk_csharp_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_query_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Query variables are scoped to the whole query expression; a continuation
    // name after `into` is bound as a query variable.
    scopes.push(Vec::new());
    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    for (index, child) in children.iter().enumerate() {
        if child.kind() == "identifier" && index > 0 {
            let previous_end = children[index - 1].end_byte();
            let gap = source.get(previous_end..child.start_byte()).unwrap_or("");
            if gap.contains("into") {
                bind_csharp_name(*child, source, "query_variable", scopes, scan)?;
                continue;
            }
        }
        walk_csharp_node(*child, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_csharp_from_clause(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The query variable name is bound, not referenced; the source expression
    // is walked in the enclosing scope.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "identifier" => {}
            _ if is_ignored_csharp_node_kind(child.kind()) => {}
            _ => walk_csharp_node(child, source, scopes, scan, deadline)?,
        }
    }
    if let Some(name) = node.child_by_field_name("name") {
        bind_csharp_name(name, source, "query_variable", scopes, scan)?;
    }
    Ok(())
}

fn walk_csharp_let_clause(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    let mut bound = false;
    for child in node.named_children(&mut cursor) {
        if !bound && child.kind() == "identifier" {
            bind_csharp_name(child, source, "query_variable", scopes, scan)?;
            bound = true;
            continue;
        }
        walk_csharp_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_csharp_join_clause(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Vec<CSharpBinding>>,
    scan: &mut CSharpScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The join range variable is bound; the source, the `on`/`equals`
    // expressions, and any `into` continuation are walked.
    let mut cursor = node.walk();
    let mut bound = false;
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "identifier" if !bound => {
                bind_csharp_name(child, source, "query_variable", scopes, scan)?;
                bound = true;
            }
            _ if is_ignored_csharp_node_kind(child.kind()) => {}
            _ => walk_csharp_node(child, source, scopes, scan, deadline)?,
        }
    }
    Ok(())
}

fn walk_csharp_join_into_clause(
    node: Node<'_>,
    source: &str,
    scopes: &mut [Vec<CSharpBinding>],
    scan: &mut CSharpScopeScan,
    _deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "identifier" {
            bind_csharp_name(child, source, "query_variable", scopes, scan)?;
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

fn record_csharp_reference(
    node: Node<'_>,
    source: &str,
    scopes: &mut [Vec<CSharpBinding>],
    scan: &mut CSharpScopeScan,
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

fn collect_csharp_function_bindings(
    node: Node<'_>,
    source: &str,
    out: &mut Vec<CSharpBinding>,
    scan: &mut CSharpScopeScan,
) -> Result<()> {
    if let Some(parameters) = node.child_by_field_name("parameters") {
        bind_csharp_parameter_list(parameters, source, "formal_parameter", out, scan)?;
    }
    Ok(())
}

fn bind_csharp_parameter_list(
    parameters: Node<'_>,
    source: &str,
    node_kind: &'static str,
    out: &mut Vec<CSharpBinding>,
    scan: &mut CSharpScopeScan,
) -> Result<()> {
    let mut cursor = parameters.walk();
    for child in parameters.named_children(&mut cursor) {
        match child.kind() {
            "parameter" | "parameter_array" => {
                if let Some(name) = child.child_by_field_name("name") {
                    bind_csharp_name_into(name, source, node_kind, out, scan)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn bind_csharp_name(
    name_node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    scopes: &mut [Vec<CSharpBinding>],
    scan: &mut CSharpScopeScan,
) -> Result<()> {
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() || name == "_" {
        return Ok(());
    }
    let binding = CSharpBinding {
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

fn bind_csharp_name_into(
    name_node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    out: &mut Vec<CSharpBinding>,
    scan: &mut CSharpScopeScan,
) -> Result<()> {
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() || name == "_" {
        return Ok(());
    }
    let binding = CSharpBinding {
        name: name.clone(),
        node_kind,
        start_byte: name_node.start_byte(),
        end_byte: name_node.end_byte(),
    };
    scan.local_bindings.push(binding.clone());
    out.push(binding);
    Ok(())
}

fn is_ignored_csharp_node_kind(kind: &str) -> bool {
    matches!(
        kind,
        // Type spellings that never name a value.
        "predefined_type"
            | "implicit_type"
            | "generic_name"
            | "qualified_name"
            | "alias_qualified_name"
            | "array_type"
            | "nullable_type"
            | "pointer_type"
            | "ref_type"
            | "function_pointer_type"
            | "tuple_type"
            | "type_argument_list"
            | "type_parameter_list"
            | "type_parameter"
            | "type_parameter_constraints_clause"
            | "primary_constraint"
            | "name_equals"
            | "base_list"
            | "interface_base_type"
            | "primary_constructor_base_type"
            | "explicit_interface_specifier"
            // Attributes and their arguments are not value references.
            | "attribute"
            | "attribute_list"
            | "attribute_argument"
            | "attribute_argument_list"
            | "global_attribute"
            // Name, label, and member-name plumbing.
            | "modifier"
            | "discard"
            | "member_binding_expression"
            // Statements whose names or labels are not value references.
            | "break_statement"
            | "continue_statement"
            | "goto_statement"
            | "empty_statement"
            // Using directives are consumed by file-item collection.
            | "using_directive"
    )
}
