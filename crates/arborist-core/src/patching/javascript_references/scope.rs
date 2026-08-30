use std::collections::BTreeSet;

use anyhow::Result;
use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::language::node_text;

#[derive(Debug, Clone)]
pub(super) struct JavaScriptBinding {
    pub(super) name: String,
    pub(super) node_kind: &'static str,
    pub(super) start_byte: usize,
    pub(super) end_byte: usize,
}

pub(super) struct JavaScriptScopeScan {
    pub(super) local_bindings: Vec<JavaScriptBinding>,
    pub(super) local_references: BTreeSet<String>,
    pub(super) external_references: BTreeSet<String>,
    pub(super) tdz_references: BTreeSet<String>,
}

struct JavaScriptParameter<'tree> {
    pattern: Node<'tree>,
    default_initializer: Option<Node<'tree>>,
}

/// A scope in the walker stack. `captures_var_bindings` marks scopes such as
/// functions and class static blocks where `var` declarations are collected,
/// while `let`/`const` bind to the innermost block scope.
struct Scope {
    captures_var_bindings: bool,
    // Callable bodies usually defer evaluation until after the initializer
    // completes. Directly invoked function expressions are the exception.
    defers_tdz_references: bool,
    bindings: Vec<JavaScriptBinding>,
    initializing_names: BTreeSet<String>,
}

/// Walks the patched JavaScript/TypeScript symbol node and classifies
/// identifier references into locally visible bindings (function and
/// arrow-function parameters, local `const`/`let`/`var` bindings including
/// destructured declarations, `for`/`for-in`/`for-of` loop variables, catch
/// parameters, and nested callable parameters) versus names that must resolve
/// at file scope (same-file declarations and imports). Non-computed property
/// names and object keys, labels, JSX tag and attribute names, and TypeScript
/// type spellings are not value references and are skipped.
pub(super) fn scan_javascript_symbol_scope(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<JavaScriptScopeScan> {
    let mut scan = JavaScriptScopeScan {
        local_bindings: Vec::new(),
        local_references: BTreeSet::new(),
        external_references: BTreeSet::new(),
        tdz_references: BTreeSet::new(),
    };
    let mut scopes: Vec<Scope> = Vec::new();

    match symbol_node.kind() {
        "function_declaration" | "generator_function_declaration" | "method_definition" => {
            walk_javascript_function(symbol_node, source, &mut scopes, &mut scan, false, deadline)?;
        }
        // A callable variable declarator (`const f = (...) => ...`) is the
        // patched symbol; validate the callable's parameters and body.
        "variable_declarator" => {
            if let Some(value) = symbol_node.child_by_field_name("value")
                && matches!(
                    value.kind(),
                    "arrow_function" | "function_expression" | "generator_function"
                )
            {
                walk_javascript_function(value, source, &mut scopes, &mut scan, false, deadline)?;
            } else {
                walk_javascript_children(symbol_node, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        "class_declaration" | "abstract_class_declaration" => {
            walk_javascript_class(symbol_node, source, &mut scopes, &mut scan, false, deadline)?;
        }
        "enum_declaration" => {
            walk_javascript_enum(symbol_node, source, &mut scopes, &mut scan, deadline)?;
        }
        // Interfaces and type aliases carry no value references to validate;
        // their member and type spellings are not value bindings.
        "interface_declaration" | "type_alias_declaration" => {}
        _ => {
            walk_javascript_children(symbol_node, source, &mut scopes, &mut scan, deadline)?;
        }
    }

    Ok(scan)
}

fn walk_javascript_node(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning JavaScript patch references")?;
    }
    match node.kind() {
        "identifier" => record_javascript_reference(node, source, scopes, scan),
        "shorthand_property_identifier" => record_javascript_reference(node, source, scopes, scan),
        // Member/property names, destructuring shorthand bindings, and labels
        // are not value references.
        "property_identifier"
        | "shorthand_property_identifier_pattern"
        | "statement_identifier" => Ok(()),
        "lexical_declaration" | "using_declaration" | "variable_declaration" => {
            walk_javascript_declaration(node, source, scopes, scan, deadline)
        }
        "variable_declarator" => {
            walk_javascript_variable_declarator(node, source, scopes, scan, deadline)
        }
        "function_declaration"
        | "generator_function_declaration"
        | "function_expression"
        | "generator_function"
        | "arrow_function"
        | "method_definition" => {
            walk_javascript_function(node, source, scopes, scan, false, deadline)
        }
        "class_declaration" | "abstract_class_declaration" | "class" => {
            walk_javascript_class(node, source, scopes, scan, false, deadline)
        }
        "public_field_definition" | "field_definition" => {
            walk_javascript_field_definition(node, source, scopes, scan, false, deadline)
        }
        "class_static_block" => {
            walk_javascript_class_static_block(node, source, scopes, scan, deadline)
        }
        "statement_block" => walk_javascript_block(node, source, scopes, scan, deadline),
        "switch_statement" => walk_javascript_switch(node, source, scopes, scan, deadline),
        "for_in_statement" => walk_javascript_for_in(node, source, scopes, scan, deadline),
        "for_statement" => walk_javascript_for_statement(node, source, scopes, scan, deadline),
        "catch_clause" => walk_javascript_catch(node, source, scopes, scan, deadline),
        "enum_declaration" => walk_javascript_enum(node, source, scopes, scan, deadline),
        "enum_body" => walk_javascript_enum_body(node, source, scopes, scan, deadline),
        "interface_declaration" | "type_alias_declaration" => Ok(()),
        "import_statement"
        | "import_clause"
        | "named_imports"
        | "import_specifier"
        | "namespace_import"
        | "export_statement"
        | "export_clause"
        | "export_specifier"
        | "formal_parameters"
        | "required_parameter"
        | "optional_parameter"
        | "function_signature"
        | "method_signature"
        | "abstract_method_signature" => Ok(()),
        "call_expression" => walk_javascript_call_expression(node, source, scopes, scan, deadline),
        "new_expression" => walk_javascript_new_expression(node, source, scopes, scan, deadline),
        "assignment_expression" => {
            walk_javascript_assignment_expression(node, source, scopes, scan, deadline)
        }
        "member_expression" | "subscript_expression" | "optional_chain" => {
            if let Some(object) = node.child_by_field_name("object") {
                walk_javascript_node(object, source, scopes, scan, deadline)?;
            }
            if node.kind() == "subscript_expression"
                && let Some(index) = node.child_by_field_name("index")
            {
                walk_javascript_node(index, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        "pair" => {
            if let Some(key) = node.child_by_field_name("key") {
                walk_javascript_computed_property_name(key, source, scopes, scan, deadline)?;
            }
            if let Some(value) = node.child_by_field_name("value") {
                walk_javascript_node(value, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        "as_expression" | "satisfies_expression" => {
            if let Some(left) = node.child_by_field_name("left") {
                walk_javascript_node(left, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        "jsx_element"
        | "jsx_opening_element"
        | "jsx_closing_element"
        | "jsx_self_closing_element" => {
            walk_javascript_jsx_element(node, source, scopes, scan, deadline)
        }
        "jsx_namespace_name" => Ok(()),
        "jsx_attribute" => {
            if let Some(value) = node.child_by_field_name("value") {
                walk_javascript_node(value, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        "jsx_expression" => walk_javascript_children(node, source, scopes, scan, deadline),
        _ => {
            if is_ignored_javascript_type_kind(node.kind()) {
                Ok(())
            } else {
                walk_javascript_children(node, source, scopes, scan, deadline)
            }
        }
    }
}

fn walk_javascript_field_definition(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    instance_initializer_executes_now: bool,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    walk_javascript_decorators(node, source, scopes, scan, deadline)?;
    if let Some(name) = node
        .child_by_field_name("property")
        .or_else(|| node.child_by_field_name("name"))
    {
        // Computed keys are evaluated when the class is created, regardless
        // of whether the field initializer is deferred until construction.
        walk_javascript_computed_property_name(name, source, scopes, scan, deadline)?;
    }
    let Some(value) = node.child_by_field_name("value") else {
        return Ok(());
    };
    if is_javascript_static_field_definition(node) || instance_initializer_executes_now {
        return walk_javascript_node(value, source, scopes, scan, deadline);
    }

    // Instance field initializers run when an instance is constructed, which
    // is after a normal class-expression binding has finished initializing.
    // Model that delayed execution without making the field a `var` scope.
    scopes.push(Scope {
        captures_var_bindings: false,
        defers_tdz_references: true,
        bindings: Vec::new(),
        initializing_names: BTreeSet::new(),
    });
    walk_javascript_node(value, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn is_javascript_static_field_definition(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| child.kind() == "static")
}

fn walk_javascript_decorators(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for decorator in node.children_by_field_name("decorator", &mut cursor) {
        walk_javascript_node(decorator, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_javascript_parameter_decorators(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let Some(parameters) = node
        .child_by_field_name("parameters")
        .or_else(|| node.child_by_field_name("parameter"))
    else {
        return Ok(());
    };
    let mut cursor = parameters.walk();
    for parameter in parameters.named_children(&mut cursor) {
        walk_javascript_decorators(parameter, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_javascript_computed_property_name(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if node.kind() == "computed_property_name" {
        walk_javascript_node(node, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_javascript_children(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_javascript_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_javascript_block(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Scope {
        captures_var_bindings: false,
        defers_tdz_references: false,
        bindings: Vec::new(),
        initializing_names: BTreeSet::new(),
    });
    {
        let scope = scopes
            .last_mut()
            .expect("a JavaScript block scope was just pushed");
        mark_javascript_block_lexical_bindings_initializing(node, source, scope)?;
        hoist_javascript_block_function_declarations(node, source, scope, scan)?;
    }
    walk_javascript_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn mark_javascript_block_lexical_bindings_initializing(
    node: Node<'_>,
    source: &str,
    scope: &mut Scope,
) -> Result<()> {
    let mut cursor = node.walk();
    for statement in node.named_children(&mut cursor) {
        mark_javascript_lexical_binding_initializing(statement, source, scope)?;
    }
    Ok(())
}

fn hoist_javascript_block_function_declarations(
    node: Node<'_>,
    source: &str,
    scope: &mut Scope,
    scan: &mut JavaScriptScopeScan,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if matches!(
            child.kind(),
            "function_declaration" | "generator_function_declaration"
        ) {
            bind_javascript_function_declaration_name(child, source, scope, scan)?;
        }
    }
    Ok(())
}

fn walk_javascript_class_static_block(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Static initialization blocks are their own lexical and `var` scope.
    // In particular, a `var` declared here must not leak to an enclosing
    // function, but it is hoisted across the entire static block.
    scopes.push(Scope {
        captures_var_bindings: true,
        defers_tdz_references: false,
        bindings: Vec::new(),
        initializing_names: BTreeSet::new(),
    });
    {
        let static_scope = scopes
            .last_mut()
            .expect("a JavaScript static-block scope was just pushed");
        mark_javascript_block_lexical_bindings_initializing(node, source, static_scope)?;
        hoist_javascript_block_function_declarations(node, source, static_scope, scan)?;
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            hoist_javascript_function_var_bindings(child, source, static_scope, scan, deadline)?;
        }
    }
    walk_javascript_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_javascript_switch(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // The discriminant is evaluated before the switch's lexical environment
    // exists, so a declaration in a case cannot shadow an outer binding here.
    if let Some(value) = node.child_by_field_name("value") {
        walk_javascript_node(value, source, scopes, scan, deadline)?;
    }

    scopes.push(Scope {
        captures_var_bindings: false,
        defers_tdz_references: false,
        bindings: Vec::new(),
        initializing_names: BTreeSet::new(),
    });
    if let Some(body) = node.child_by_field_name("body") {
        {
            let scope = scopes
                .last_mut()
                .expect("a JavaScript switch scope was just pushed");
            mark_javascript_switch_lexical_bindings_initializing(body, source, scope)?;
            hoist_javascript_switch_function_declarations(body, source, scope, scan)?;
        }
        walk_javascript_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn mark_javascript_switch_lexical_bindings_initializing(
    body: Node<'_>,
    source: &str,
    scope: &mut Scope,
) -> Result<()> {
    let mut case_cursor = body.walk();
    for case_clause in body.named_children(&mut case_cursor) {
        if !matches!(case_clause.kind(), "switch_case" | "switch_default") {
            continue;
        }
        let mut statement_cursor = case_clause.walk();
        for statement in case_clause.children_by_field_name("body", &mut statement_cursor) {
            mark_javascript_lexical_binding_initializing(statement, source, scope)?;
        }
    }
    Ok(())
}

fn mark_javascript_lexical_binding_initializing(
    node: Node<'_>,
    source: &str,
    scope: &mut Scope,
) -> Result<()> {
    match node.kind() {
        "lexical_declaration" | "using_declaration" => {
            let mut cursor = node.walk();
            for declarator in node.named_children(&mut cursor) {
                if declarator.kind() != "variable_declarator" {
                    continue;
                }
                if let Some(name) = declarator.child_by_field_name("name") {
                    mark_javascript_pattern_initializing(name, source, scope)?;
                }
            }
        }
        "class_declaration" | "abstract_class_declaration" | "enum_declaration" => {
            if let Some(name) = node.child_by_field_name("name") {
                mark_javascript_pattern_initializing(name, source, scope)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn hoist_javascript_switch_function_declarations(
    body: Node<'_>,
    source: &str,
    scope: &mut Scope,
    scan: &mut JavaScriptScopeScan,
) -> Result<()> {
    let mut case_cursor = body.walk();
    for case_clause in body.named_children(&mut case_cursor) {
        if !matches!(case_clause.kind(), "switch_case" | "switch_default") {
            continue;
        }
        let mut statement_cursor = case_clause.walk();
        for statement in case_clause.children_by_field_name("body", &mut statement_cursor) {
            if matches!(
                statement.kind(),
                "function_declaration" | "generator_function_declaration"
            ) {
                bind_javascript_function_declaration_name(statement, source, scope, scan)?;
            }
        }
    }
    Ok(())
}

fn walk_javascript_function(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    immediately_invoked: bool,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    walk_javascript_decorators(node, source, scopes, scan, deadline)?;
    walk_javascript_parameter_decorators(node, source, scopes, scan, deadline)?;
    if node.kind() == "method_definition"
        && let Some(name) = node.child_by_field_name("name")
    {
        walk_javascript_computed_property_name(name, source, scopes, scan, deadline)?;
    }
    // Block-scoped function declarations are hoisted before their enclosing
    // block is walked. Keep this fallback for declaration contexts that do
    // not pass through the block pre-scan; the helper avoids duplicate binding
    // evidence when the declaration was already hoisted.
    if matches!(
        node.kind(),
        "function_declaration" | "generator_function_declaration"
    ) && let Some(enclosing) = scopes.last_mut()
    {
        bind_javascript_function_declaration_name(node, source, enclosing, scan)?;
    }
    let (function_scope, parameter_patterns) =
        collect_javascript_callable_bindings(node, source, scan, !immediately_invoked)?;
    scopes.push(function_scope);
    for parameter in parameter_patterns {
        if let Some(parameter_default) = parameter.default_initializer {
            walk_javascript_node(parameter_default, source, scopes, scan, deadline)?;
        }
        walk_javascript_lexical_pattern_initialization(
            parameter.pattern,
            source,
            "parameter",
            scopes,
            scan,
            deadline,
        )?;
    }
    if let Some(body) = javascript_callable_body(node) {
        {
            let function_scope = scopes
                .last_mut()
                .expect("a JavaScript function scope was just pushed");
            hoist_javascript_function_var_bindings(body, source, function_scope, scan, deadline)?;
        }
        walk_javascript_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn hoist_javascript_function_var_bindings(
    node: Node<'_>,
    source: &str,
    function_scope: &mut Scope,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning JavaScript hoisted var bindings")?;
    }
    if matches!(
        node.kind(),
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
    ) {
        return Ok(());
    }
    if node.kind() == "variable_declaration" {
        let mut cursor = node.walk();
        for declarator in node.named_children(&mut cursor) {
            if declarator.kind() != "variable_declarator" {
                continue;
            }
            if let Some(name) = declarator.child_by_field_name("name") {
                let mut defaults = Vec::new();
                collect_javascript_pattern_bindings(
                    name,
                    source,
                    "variable_declarator",
                    function_scope,
                    scan,
                    &mut defaults,
                )?;
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        hoist_javascript_function_var_bindings(child, source, function_scope, scan, deadline)?;
    }
    Ok(())
}

fn walk_javascript_enum(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // TypeScript enums produce runtime values. They are block-scoped in the
    // source language and become available while their member initializers are
    // evaluated, after remaining in the temporal dead zone before declaration.
    let created_scope = scopes.is_empty();
    if created_scope {
        scopes.push(Scope {
            captures_var_bindings: false,
            defers_tdz_references: false,
            bindings: Vec::new(),
            initializing_names: BTreeSet::new(),
        });
    }
    if let Some(name) = node.child_by_field_name("name") {
        let scope = scopes
            .last_mut()
            .expect("an enum scope is available while its declaration is walked");
        let name_text = node_text(name, source)?.trim().to_string();
        scope.initializing_names.remove(&name_text);
        bind_javascript_name(name, source, node.kind(), scope, scan)?;
    }
    if let Some(body) = node.child_by_field_name("body") {
        walk_javascript_node(body, source, scopes, scan, deadline)?;
    }
    if created_scope {
        scopes.pop();
    }
    Ok(())
}

fn walk_javascript_enum_body(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Enum members are runtime assignments. Member names are property names,
    // but computed names and value initializers are evaluated expressions.
    let mut cursor = node.walk();
    for member in node.named_children(&mut cursor) {
        if member.kind() != "enum_assignment" {
            continue;
        }
        if let Some(name) = member.child_by_field_name("name") {
            walk_javascript_computed_property_name(name, source, scopes, scan, deadline)?;
        }
        if let Some(value) = member.child_by_field_name("value") {
            walk_javascript_node(value, source, scopes, scan, deadline)?;
        }
    }
    Ok(())
}

fn walk_javascript_class(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    immediately_constructed: bool,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let name = node.child_by_field_name("name");
    let is_declaration = matches!(
        node.kind(),
        "class_declaration" | "abstract_class_declaration"
    );

    // Decorator expressions are evaluated in the surrounding scope while the
    // class is being defined, before its internal class-name scope is entered.
    walk_javascript_decorators(node, source, scopes, scan, deadline)?;

    // Named classes have an internal binding that is in the temporal dead
    // zone while their heritage is evaluated, then remains visible to class
    // members. Keep that binding separate from the surrounding declaration
    // scope so named class expressions do not leak their internal name.
    scopes.push(Scope {
        captures_var_bindings: false,
        defers_tdz_references: false,
        bindings: Vec::new(),
        initializing_names: BTreeSet::new(),
    });
    if let Some(name) = name {
        mark_javascript_pattern_initializing(
            name,
            source,
            scopes
                .last_mut()
                .expect("a JavaScript class scope was just pushed"),
        )?;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "class_heritage" {
            walk_javascript_class_heritage(child, source, scopes, scan, deadline)?;
        }
    }

    if let Some(name) = name {
        if is_declaration && scopes.len() >= 2 {
            let enclosing_index = scopes.len() - 2;
            let enclosing = &mut scopes[enclosing_index];
            let name_text = node_text(name, source)?.trim().to_string();
            enclosing.initializing_names.remove(&name_text);
            bind_javascript_name(name, source, node.kind(), enclosing, scan)?;
        }
        let class_scope = scopes
            .last_mut()
            .expect("a JavaScript class scope was just pushed");
        let name_text = node_text(name, source)?.trim().to_string();
        class_scope.initializing_names.remove(&name_text);
        bind_javascript_name(name, source, node.kind(), class_scope, scan)?;
    }

    if let Some(body) = javascript_class_body(node) {
        walk_javascript_class_body(
            body,
            source,
            scopes,
            scan,
            immediately_constructed,
            deadline,
        )?;
    }
    scopes.pop();
    Ok(())
}

fn walk_javascript_class_heritage(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    match node.kind() {
        "implements_clause" => Ok(()),
        "extends_clause" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() != "type_arguments" {
                    walk_javascript_node(child, source, scopes, scan, deadline)?;
                }
            }
            Ok(())
        }
        "class_heritage" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                walk_javascript_class_heritage(child, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        _ => walk_javascript_node(node, source, scopes, scan, deadline),
    }
}

fn walk_javascript_class_body(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    immediately_constructed: bool,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if matches!(child.kind(), "public_field_definition" | "field_definition") {
            walk_javascript_field_definition(
                child,
                source,
                scopes,
                scan,
                immediately_constructed,
                deadline,
            )?;
        } else if immediately_constructed && is_javascript_constructor(child, source)? {
            walk_javascript_function(child, source, scopes, scan, true, deadline)?;
        } else {
            walk_javascript_node(child, source, scopes, scan, deadline)?;
        }
    }
    Ok(())
}

fn is_javascript_constructor(node: Node<'_>, source: &str) -> Result<bool> {
    if node.kind() != "method_definition" {
        return Ok(false);
    }
    let Some(name) = node.child_by_field_name("name") else {
        return Ok(false);
    };
    Ok(name.kind() == "property_identifier" && node_text(name, source)?.trim() == "constructor")
}

fn walk_javascript_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let is_var = node.kind() == "variable_declaration";
    let mut cursor = node.walk();
    let declarators = node
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "variable_declarator")
        .collect::<Vec<_>>();
    let target_index = if is_var {
        javascript_var_target_scope_index(scopes)
    } else {
        scopes.len().checked_sub(1)
    };
    let Some(target_index) = target_index else {
        return Ok(());
    };

    if is_var {
        let mut deferred = Vec::new();
        {
            let target_scope = &mut scopes[target_index];
            for declarator in &declarators {
                let mut defaults = Vec::new();
                if let Some(name_node) = declarator.child_by_field_name("name") {
                    collect_javascript_pattern_bindings(
                        name_node,
                        source,
                        "variable_declarator",
                        target_scope,
                        scan,
                        &mut defaults,
                    )?;
                }
                deferred.push((defaults, declarator.child_by_field_name("value")));
            }
        }
        for (defaults, value) in deferred {
            for default in defaults {
                walk_javascript_node(default, source, scopes, scan, deadline)?;
            }
            if let Some(value) = value {
                walk_javascript_node(value, source, scopes, scan, deadline)?;
            }
        }
        return Ok(());
    }

    // Lexical bindings exist for the entire declaration, but they remain in
    // the temporal dead zone until each declarator initializer completes.
    // Keep their names separate from visible bindings while walking defaults
    // and values, then expose every declarator in source order.
    let mut pending = Vec::new();
    let mut initializing_names = BTreeSet::new();
    for declarator in declarators {
        let Some(name_node) = declarator.child_by_field_name("name") else {
            continue;
        };
        let mut names = BTreeSet::new();
        let mut defaults = Vec::new();
        collect_javascript_pattern_initialization(name_node, source, &mut names, &mut defaults)?;
        initializing_names.extend(names.iter().cloned());
        pending.push((
            name_node,
            names,
            defaults,
            declarator.child_by_field_name("value"),
        ));
    }
    scopes[target_index]
        .initializing_names
        .extend(initializing_names);

    for (name_node, names, defaults, value) in pending {
        for default in defaults {
            walk_javascript_node(default, source, scopes, scan, deadline)?;
        }
        if let Some(value) = value {
            walk_javascript_node(value, source, scopes, scan, deadline)?;
        }
        let target_scope = &mut scopes[target_index];
        for name in names {
            target_scope.initializing_names.remove(&name);
        }
        let mut ignored_defaults = Vec::new();
        collect_javascript_pattern_bindings(
            name_node,
            source,
            "variable_declarator",
            target_scope,
            scan,
            &mut ignored_defaults,
        )?;
    }
    Ok(())
}

fn mark_javascript_pattern_initializing(
    node: Node<'_>,
    source: &str,
    scope: &mut Scope,
) -> Result<()> {
    let mut names = BTreeSet::new();
    let mut ignored_defaults = Vec::new();
    collect_javascript_pattern_initialization(node, source, &mut names, &mut ignored_defaults)?;
    scope.initializing_names.extend(names);
    Ok(())
}

fn walk_javascript_lexical_pattern_initialization(
    node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    match node.kind() {
        "identifier" | "shorthand_property_identifier_pattern" => {
            let name = node_text(node, source)?.trim().to_string();
            let scope = scopes
                .last_mut()
                .expect("pattern initialization scope is active");
            scope.initializing_names.remove(&name);
            bind_javascript_name(node, source, node_kind, scope, scan)
        }
        "assignment_pattern" | "object_assignment_pattern" => {
            if let Some(right) = node.child_by_field_name("right") {
                walk_javascript_node(right, source, scopes, scan, deadline)?;
            }
            if let Some(left) = node.child_by_field_name("left") {
                walk_javascript_lexical_pattern_initialization(
                    left, source, node_kind, scopes, scan, deadline,
                )?;
            }
            Ok(())
        }
        "rest_pattern" | "array_pattern" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                walk_javascript_lexical_pattern_initialization(
                    child, source, node_kind, scopes, scan, deadline,
                )?;
            }
            Ok(())
        }
        "object_pattern" => {
            let mut cursor = node.walk();
            for member in node.named_children(&mut cursor) {
                if member.kind() == "pair_pattern" {
                    if let Some(key) = member.child_by_field_name("key") {
                        walk_javascript_computed_property_name(
                            key, source, scopes, scan, deadline,
                        )?;
                    }
                    if let Some(value) = member.child_by_field_name("value") {
                        walk_javascript_lexical_pattern_initialization(
                            value, source, node_kind, scopes, scan, deadline,
                        )?;
                    }
                } else {
                    walk_javascript_lexical_pattern_initialization(
                        member, source, node_kind, scopes, scan, deadline,
                    )?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn javascript_for_in_has_lexical_binding(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.children_by_field_name("kind", &mut cursor)
        .any(|kind| matches!(kind.kind(), "const" | "let" | "using"))
}

fn javascript_for_in_has_var_binding(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.children_by_field_name("kind", &mut cursor)
        .any(|kind| kind.kind() == "var")
}

fn collect_javascript_pattern_initialization<'tree>(
    node: Node<'tree>,
    source: &str,
    names: &mut BTreeSet<String>,
    defaults: &mut Vec<Node<'tree>>,
) -> Result<()> {
    match node.kind() {
        "identifier" | "shorthand_property_identifier_pattern" => {
            let name = node_text(node, source)?.trim().to_string();
            if !name.is_empty() {
                names.insert(name);
            }
        }
        "assignment_pattern" | "object_assignment_pattern" => {
            if let Some(left) = node.child_by_field_name("left") {
                collect_javascript_pattern_initialization(left, source, names, defaults)?;
            }
            if let Some(right) = node.child_by_field_name("right") {
                defaults.push(right);
            }
        }
        "rest_pattern" | "array_pattern" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_javascript_pattern_initialization(child, source, names, defaults)?;
            }
        }
        "object_pattern" => {
            let mut cursor = node.walk();
            for member in node.named_children(&mut cursor) {
                if member.kind() == "pair_pattern" {
                    if let Some(value) = member.child_by_field_name("value") {
                        collect_javascript_pattern_initialization(value, source, names, defaults)?;
                    }
                } else {
                    collect_javascript_pattern_initialization(member, source, names, defaults)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn walk_javascript_variable_declarator(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let Some(target_index) = scopes.len().checked_sub(1) else {
        return Ok(());
    };
    let mut defaults = Vec::new();
    {
        let target_scope = &mut scopes[target_index];
        if let Some(name_node) = node.child_by_field_name("name") {
            collect_javascript_pattern_bindings(
                name_node,
                source,
                "variable_declarator",
                target_scope,
                scan,
                &mut defaults,
            )?;
        }
    }
    for default in defaults {
        walk_javascript_node(default, source, scopes, scan, deadline)?;
    }
    if let Some(value) = node.child_by_field_name("value") {
        walk_javascript_node(value, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_javascript_assignment_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(left) = node.child_by_field_name("left") {
        walk_javascript_assignment_target(left, source, scopes, scan, deadline)?;
    }
    if let Some(right) = node.child_by_field_name("right") {
        walk_javascript_node(right, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_javascript_assignment_target(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    match node.kind() {
        "identifier" | "shorthand_property_identifier_pattern" => {
            record_javascript_reference(node, source, scopes, scan)
        }
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                walk_javascript_assignment_target(child, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        "array_pattern" | "rest_pattern" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                walk_javascript_assignment_target(child, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        "object_assignment_pattern" => {
            if let Some(left) = node.child_by_field_name("left") {
                walk_javascript_assignment_target(left, source, scopes, scan, deadline)?;
            }
            if let Some(right) = node.child_by_field_name("right") {
                walk_javascript_node(right, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        "object_pattern" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "pair_pattern" {
                    if let Some(key) = child.child_by_field_name("key") {
                        walk_javascript_computed_property_name(
                            key, source, scopes, scan, deadline,
                        )?;
                    }
                    if let Some(value) = child.child_by_field_name("value") {
                        walk_javascript_assignment_target(value, source, scopes, scan, deadline)?;
                    }
                } else {
                    walk_javascript_assignment_target(child, source, scopes, scan, deadline)?;
                }
            }
            Ok(())
        }
        _ => walk_javascript_node(node, source, scopes, scan, deadline),
    }
}
fn walk_javascript_for_in(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Scope {
        captures_var_bindings: false,
        defers_tdz_references: false,
        bindings: Vec::new(),
        initializing_names: BTreeSet::new(),
    });
    let left = node.child_by_field_name("left");
    let lexical_left = javascript_for_in_has_lexical_binding(node)
        .then_some(left)
        .flatten();
    let var_left = javascript_for_in_has_var_binding(node)
        .then_some(left)
        .flatten();
    let mut post_iterable_defaults = Vec::new();
    if let Some(left) = lexical_left {
        // The iterable expression runs while the per-loop lexical bindings
        // exist but are still in their temporal dead zone.
        mark_javascript_pattern_initializing(
            left,
            source,
            scopes.last_mut().expect("loop scope is active"),
        )?;
    } else if let Some(left) = var_left {
        if let Some(target_index) = javascript_var_target_scope_index(scopes) {
            collect_javascript_pattern_bindings(
                left,
                source,
                "loop_variable",
                &mut scopes[target_index],
                scan,
                &mut post_iterable_defaults,
            )?;
        }
    } else if let Some(left) = left {
        // Assignment targets do not declare bindings. Scan them as values so
        // unresolved identifiers and computed member components are validated.
        walk_javascript_assignment_target(left, source, scopes, scan, deadline)?;
    }
    if let Some(right) = node.child_by_field_name("right") {
        walk_javascript_node(right, source, scopes, scan, deadline)?;
    }
    for default in post_iterable_defaults {
        walk_javascript_node(default, source, scopes, scan, deadline)?;
    }
    if let Some(left) = lexical_left {
        walk_javascript_lexical_pattern_initialization(
            left,
            source,
            "loop_variable",
            scopes,
            scan,
            deadline,
        )?;
    }
    if let Some(body) = node.child_by_field_name("body") {
        walk_javascript_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_javascript_for_statement(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Scope {
        captures_var_bindings: false,
        defers_tdz_references: false,
        bindings: Vec::new(),
        initializing_names: BTreeSet::new(),
    });
    walk_javascript_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_javascript_catch(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Scope {
        captures_var_bindings: false,
        defers_tdz_references: false,
        bindings: Vec::new(),
        initializing_names: BTreeSet::new(),
    });
    if let Some(parameter) = node.child_by_field_name("parameter") {
        let mut ignored_defaults = Vec::new();
        {
            let scope = scopes.last_mut().expect("catch scope is active");
            collect_javascript_pattern_bindings(
                parameter,
                source,
                "catch_parameter",
                scope,
                scan,
                &mut ignored_defaults,
            )?;
            mark_javascript_pattern_initializing(parameter, source, scope)?;
        }
        walk_javascript_lexical_pattern_initialization(
            parameter,
            source,
            "catch_parameter",
            scopes,
            scan,
            deadline,
        )?;
    }
    if let Some(body) = node.child_by_field_name("body") {
        walk_javascript_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_javascript_call_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(function) = node.child_by_field_name("function") {
        if let Some(callable) = javascript_immediately_invoked_callable(function, true) {
            walk_javascript_function(callable, source, scopes, scan, true, deadline)?;
        } else {
            walk_javascript_node(function, source, scopes, scan, deadline)?;
        }
    }
    if let Some(arguments) = node.child_by_field_name("arguments") {
        walk_javascript_node(arguments, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn walk_javascript_new_expression(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(constructor) = node.child_by_field_name("constructor") {
        // Unlike direct calls, `new` cannot execute an arrow-function body.
        if let Some(class) = javascript_immediately_constructed_class(constructor) {
            walk_javascript_class(class, source, scopes, scan, true, deadline)?;
        } else if let Some(callable) = javascript_immediately_invoked_callable(constructor, false) {
            walk_javascript_function(callable, source, scopes, scan, true, deadline)?;
        } else {
            walk_javascript_node(constructor, source, scopes, scan, deadline)?;
        }
    }
    if let Some(arguments) = node.child_by_field_name("arguments") {
        walk_javascript_node(arguments, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn javascript_immediately_constructed_class(node: Node<'_>) -> Option<Node<'_>> {
    let node = javascript_unwrap_value_expression(node);
    (node.kind() == "class").then_some(node)
}

/// Returns a function expression that is evaluated immediately by a direct
/// call or constructor. Parentheses and TypeScript value-level casts do not
/// defer the invocation.
fn javascript_immediately_invoked_callable(
    node: Node<'_>,
    allows_arrow_function: bool,
) -> Option<Node<'_>> {
    let node = javascript_unwrap_value_expression(node);
    match node.kind() {
        "function_expression" => Some(node),
        "arrow_function" if allows_arrow_function => Some(node),
        _ => None,
    }
}

fn javascript_unwrap_value_expression(mut node: Node<'_>) -> Node<'_> {
    loop {
        let next = match node.kind() {
            "parenthesized_expression" => {
                let mut cursor = node.walk();
                node.named_children(&mut cursor).next()
            }
            "as_expression" | "satisfies_expression" => node.child_by_field_name("left"),
            _ => None,
        };
        let Some(next) = next else {
            return node;
        };
        node = next;
    }
}

fn walk_javascript_jsx_element(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // Tag names and attribute names are not value references; only expression
    // content inside JSX is walked.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if matches!(child.kind(), "identifier" | "property_identifier") {
            continue;
        }
        walk_javascript_node(child, source, scopes, scan, deadline)?;
    }
    Ok(())
}

fn record_javascript_reference(
    node: Node<'_>,
    source: &str,
    scopes: &[Scope],
    scan: &mut JavaScriptScopeScan,
) -> Result<()> {
    let name = node_text(node, source)?.trim().to_string();
    if name.is_empty() {
        return Ok(());
    }
    let mut crossed_deferred_callable = false;
    for scope in scopes.iter().rev() {
        if scope.initializing_names.contains(&name) {
            if crossed_deferred_callable {
                // A deferred nested callable captures the binding after its
                // initializer completes, so it does not read through the
                // temporal dead zone.
                scan.local_references.insert(name);
            } else {
                scan.tdz_references.insert(name);
            }
            return Ok(());
        }
        if scope.bindings.iter().any(|binding| binding.name == name) {
            scan.local_references.insert(name);
            return Ok(());
        }
        crossed_deferred_callable |= scope.defers_tdz_references;
    }
    scan.external_references.insert(name);
    Ok(())
}

fn javascript_var_target_scope_index(scopes: &[Scope]) -> Option<usize> {
    for (index, scope) in scopes.iter().enumerate().rev() {
        if scope.captures_var_bindings {
            return Some(index);
        }
    }
    if scopes.is_empty() { None } else { Some(0) }
}

/// Collects the parameter bindings of a callable node and preserves their
/// source order with TypeScript-only default initializer expressions.
fn collect_javascript_callable_bindings<'tree>(
    node: Node<'tree>,
    source: &str,
    scan: &mut JavaScriptScopeScan,
    defers_tdz_references: bool,
) -> Result<(Scope, Vec<JavaScriptParameter<'tree>>)> {
    let mut scope = Scope {
        captures_var_bindings: true,
        defers_tdz_references,
        bindings: Vec::new(),
        initializing_names: BTreeSet::new(),
    };
    let mut parameter_patterns = Vec::new();
    // Named function and generator expressions bind their own name inside
    // their scope, including parameter default initializers.
    let expression_node_kind = match node.kind() {
        "function_expression" => Some("function_expression"),
        "generator_function" => Some("generator_function"),
        _ => None,
    };
    if let Some(node_kind) = expression_node_kind
        && let Some(name_node) = node.child_by_field_name("name")
        && name_node.kind() == "identifier"
    {
        bind_javascript_name(name_node, source, node_kind, &mut scope, scan)?;
    }
    let parameters = node
        .child_by_field_name("parameters")
        .or_else(|| node.child_by_field_name("parameter"));
    if let Some(parameters) = parameters {
        if parameters.kind() == "identifier" {
            parameter_patterns.push(JavaScriptParameter {
                pattern: parameters,
                default_initializer: None,
            });
        } else {
            let mut cursor = parameters.walk();
            for parameter in parameters.named_children(&mut cursor) {
                if parameter.kind() == "identifier" {
                    parameter_patterns.push(JavaScriptParameter {
                        pattern: parameter,
                        default_initializer: None,
                    });
                } else if let Some(pattern) = javascript_parameter_pattern(parameter) {
                    parameter_patterns.push(JavaScriptParameter {
                        pattern,
                        default_initializer: javascript_parameter_default_initializer(parameter),
                    });
                }
            }
        }
    }
    for parameter in &parameter_patterns {
        let mut ignored_defaults = Vec::new();
        collect_javascript_pattern_bindings(
            parameter.pattern,
            source,
            "parameter",
            &mut scope,
            scan,
            &mut ignored_defaults,
        )?;
        let mut names = BTreeSet::new();
        collect_javascript_pattern_initialization(
            parameter.pattern,
            source,
            &mut names,
            &mut ignored_defaults,
        )?;
        scope.initializing_names.extend(names);
    }
    Ok((scope, parameter_patterns))
}

/// Returns the value expression after a TypeScript parameter's `=` token.
/// JavaScript defaults are represented by an `assignment_pattern` and are
/// handled while that pattern is initialized instead.
fn javascript_parameter_default_initializer(node: Node<'_>) -> Option<Node<'_>> {
    if !matches!(node.kind(), "required_parameter" | "optional_parameter") {
        return None;
    }
    let mut cursor = node.walk();
    let mut after_equals = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "=" {
            after_equals = true;
        } else if after_equals && child.is_named() {
            return Some(child);
        }
    }
    None
}

/// Returns the binding pattern of a single formal parameter, falling back to
/// the first pattern-shaped child for parameter-property spellings such as
/// `constructor(private initial: T)` where the pattern is not a field.
fn javascript_parameter_pattern(node: Node<'_>) -> Option<Node<'_>> {
    if matches!(
        node.kind(),
        "identifier"
            | "object_pattern"
            | "array_pattern"
            | "assignment_pattern"
            | "rest_pattern"
            | "shorthand_property_identifier_pattern"
    ) {
        return Some(node);
    }
    if let Some(pattern) = node.child_by_field_name("pattern") {
        return Some(pattern);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|child| {
        matches!(
            child.kind(),
            "identifier"
                | "object_pattern"
                | "array_pattern"
                | "assignment_pattern"
                | "rest_pattern"
                | "shorthand_property_identifier_pattern"
        )
    })
}

/// Collects local names bound by a destructuring pattern. Default-value
/// expressions are returned separately so callers can walk them in the scope
/// where they are evaluated.
fn collect_javascript_pattern_bindings<'tree>(
    node: Node<'tree>,
    source: &str,
    node_kind: &'static str,
    scope: &mut Scope,
    scan: &mut JavaScriptScopeScan,
    defaults: &mut Vec<Node<'tree>>,
) -> Result<()> {
    match node.kind() {
        "identifier" | "shorthand_property_identifier_pattern" => {
            bind_javascript_name(node, source, node_kind, scope, scan)?;
        }
        "assignment_pattern" | "object_assignment_pattern" => {
            if let Some(left) = node.child_by_field_name("left") {
                collect_javascript_pattern_bindings(
                    left, source, node_kind, scope, scan, defaults,
                )?;
            }
            if let Some(right) = node.child_by_field_name("right") {
                defaults.push(right);
            }
        }
        "rest_pattern" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_javascript_pattern_bindings(
                    child, source, node_kind, scope, scan, defaults,
                )?;
            }
        }
        "object_pattern" => {
            let mut cursor = node.walk();
            for member in node.named_children(&mut cursor) {
                match member.kind() {
                    "shorthand_property_identifier_pattern" => {
                        bind_javascript_name(member, source, node_kind, scope, scan)?;
                    }
                    "object_assignment_pattern" | "assignment_pattern" => {
                        if let Some(left) = member.child_by_field_name("left") {
                            collect_javascript_pattern_bindings(
                                left, source, node_kind, scope, scan, defaults,
                            )?;
                        }
                        if let Some(right) = member.child_by_field_name("right") {
                            defaults.push(right);
                        }
                    }
                    "pair_pattern" => {
                        if let Some(value) = member.child_by_field_name("value") {
                            collect_javascript_pattern_bindings(
                                value, source, node_kind, scope, scan, defaults,
                            )?;
                        }
                    }
                    _ => {
                        collect_javascript_pattern_bindings(
                            member, source, node_kind, scope, scan, defaults,
                        )?;
                    }
                }
            }
        }
        "array_pattern" => {
            let mut cursor = node.walk();
            for element in node.named_children(&mut cursor) {
                collect_javascript_pattern_bindings(
                    element, source, node_kind, scope, scan, defaults,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn bind_javascript_function_declaration_name(
    node: Node<'_>,
    source: &str,
    scope: &mut Scope,
    scan: &mut JavaScriptScopeScan,
) -> Result<()> {
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(());
    };
    bind_javascript_name(name_node, source, node.kind(), scope, scan)
}

fn bind_javascript_name(
    name_node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    scope: &mut Scope,
    scan: &mut JavaScriptScopeScan,
) -> Result<()> {
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty()
        || scope.bindings.iter().any(|binding| {
            binding.start_byte == name_node.start_byte() && binding.end_byte == name_node.end_byte()
        })
    {
        return Ok(());
    }
    let binding = JavaScriptBinding {
        name: name.clone(),
        node_kind,
        start_byte: name_node.start_byte(),
        end_byte: name_node.end_byte(),
    };
    scan.local_bindings.push(binding.clone());
    scope.bindings.push(binding);
    Ok(())
}

fn javascript_callable_body<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    node.child_by_field_name("body")
}

fn javascript_class_body<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "class_body")
}

fn is_ignored_javascript_type_kind(kind: &str) -> bool {
    matches!(
        kind,
        "abstract_type"
            | "array_type"
            | "class_heritage"
            | "conditional_type"
            | "constructor_type"
            | "extends_clause"
            | "function_type"
            | "generic_type"
            | "heritage_clause"
            | "implements_clause"
            | "index_type_query"
            | "infer_type"
            | "intersection_type"
            | "keyof_type"
            | "literal_type"
            | "lookup_type"
            | "mapped_type"
            | "object_type"
            | "optional_type"
            | "parenthesized_type"
            | "predefined_type"
            | "property_signature"
            | "readonly_type"
            | "rest_type"
            | "template_literal_type"
            | "tuple_type"
            | "type_annotation"
            | "type_arguments"
            | "type_identifier"
            | "type_parameter"
            | "type_parameters"
            | "type_predicate"
            | "type_query"
            | "union_type"
    )
}
