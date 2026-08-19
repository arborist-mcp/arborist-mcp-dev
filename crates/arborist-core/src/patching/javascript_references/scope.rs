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
}

/// A scope in the walker stack. `is_function_scope` marks function-like
/// scopes so `var` declarations bind to the nearest function scope while
/// `let`/`const` bind to the innermost block scope.
struct Scope {
    is_function_scope: bool,
    bindings: Vec<JavaScriptBinding>,
}

/// Walks the patched JavaScript/TypeScript symbol node and classifies
/// identifier references into locally visible bindings (function and
/// arrow-function parameters, local `const`/`let`/`var` bindings including
/// destructured declarations, `for`/`for-in`/`for-of` loop variables, catch
/// parameters, and nested callable parameters) versus names that must resolve
/// at file scope (same-file declarations and imports). Property names, object
/// keys, labels, JSX tag and attribute names, and TypeScript type spellings
/// are not value references and are skipped.
pub(super) fn scan_javascript_symbol_scope(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<JavaScriptScopeScan> {
    let mut scan = JavaScriptScopeScan {
        local_bindings: Vec::new(),
        local_references: BTreeSet::new(),
        external_references: BTreeSet::new(),
    };
    let mut scopes: Vec<Scope> = Vec::new();

    match symbol_node.kind() {
        "function_declaration" | "generator_function_declaration" | "method_definition" => {
            walk_javascript_function(symbol_node, source, &mut scopes, &mut scan, deadline)?;
        }
        // A callable variable declarator (`const f = (...) => ...`) is the
        // patched symbol; validate the callable's parameters and body.
        "variable_declarator" => {
            if let Some(value) = symbol_node.child_by_field_name("value")
                && matches!(value.kind(), "arrow_function" | "function_expression")
            {
                walk_javascript_function(value, source, &mut scopes, &mut scan, deadline)?;
            } else {
                walk_javascript_children(symbol_node, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        "class_declaration" | "abstract_class_declaration" => {
            scopes.push(Scope {
                is_function_scope: false,
                bindings: Vec::new(),
            });
            if let Some(body) = javascript_class_body(symbol_node) {
                walk_javascript_node(body, source, &mut scopes, &mut scan, deadline)?;
            }
        }
        // Interfaces, enums, and type aliases carry no value references to
        // validate; their member and type spellings are not value bindings.
        "interface_declaration" | "enum_declaration" | "type_alias_declaration" => {}
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
        "lexical_declaration" | "variable_declaration" => {
            walk_javascript_declaration(node, source, scopes, scan, deadline)
        }
        "variable_declarator" => {
            walk_javascript_variable_declarator(node, source, scopes, scan, deadline)
        }
        "function_declaration"
        | "generator_function_declaration"
        | "function_expression"
        | "arrow_function"
        | "method_definition" => walk_javascript_function(node, source, scopes, scan, deadline),
        "class_declaration" | "abstract_class_declaration" => {
            walk_javascript_class(node, source, scopes, scan, deadline)
        }
        "public_field_definition" | "field_definition" => {
            if let Some(value) = node.child_by_field_name("value") {
                walk_javascript_node(value, source, scopes, scan, deadline)?;
            }
            Ok(())
        }
        "class_static_block" => walk_javascript_children(node, source, scopes, scan, deadline),
        "statement_block" => walk_javascript_block(node, source, scopes, scan, deadline),
        "for_in_statement" => walk_javascript_for_in(node, source, scopes, scan, deadline),
        "for_statement" => walk_javascript_for_statement(node, source, scopes, scan, deadline),
        "catch_clause" => walk_javascript_catch(node, source, scopes, scan, deadline),
        "interface_declaration" | "enum_declaration" | "type_alias_declaration" => Ok(()),
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
        is_function_scope: false,
        bindings: Vec::new(),
    });
    walk_javascript_children(node, source, scopes, scan, deadline)?;
    scopes.pop();
    Ok(())
}

fn walk_javascript_function(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // A nested function declaration binds its own name in the enclosing scope
    // so recursive and later calls resolve. The patched symbol itself has no
    // enclosing scope here; its name resolves through same-file items.
    if matches!(
        node.kind(),
        "function_declaration" | "generator_function_declaration"
    ) && let Some(name_node) = node.child_by_field_name("name")
        && let Some(enclosing) = scopes.last_mut()
    {
        bind_javascript_name(name_node, source, node.kind(), enclosing, scan)?;
    }
    let (function_scope, defaults) = collect_javascript_callable_bindings(node, source, scan)?;
    scopes.push(function_scope);
    for default in defaults {
        walk_javascript_node(default, source, scopes, scan, deadline)?;
    }
    if let Some(body) = javascript_callable_body(node) {
        walk_javascript_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_javascript_class(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(name_node) = node.child_by_field_name("name")
        && let Some(enclosing) = scopes.last_mut()
    {
        bind_javascript_name(name_node, source, node.kind(), enclosing, scan)?;
    }
    scopes.push(Scope {
        is_function_scope: false,
        bindings: Vec::new(),
    });
    if let Some(body) = javascript_class_body(node) {
        walk_javascript_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
}

fn walk_javascript_declaration(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    // `var` is function-scoped; `let`/`const` are block-scoped. Bind into the
    // target scope but still walk initializer values in the current block
    // scope, matching JavaScript evaluation semantics conservatively.
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

fn walk_javascript_for_in(
    node: Node<'_>,
    source: &str,
    scopes: &mut Vec<Scope>,
    scan: &mut JavaScriptScopeScan,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    scopes.push(Scope {
        is_function_scope: false,
        bindings: Vec::new(),
    });
    if let Some(left) = node.child_by_field_name("left") {
        if matches!(left.kind(), "lexical_declaration" | "variable_declaration") {
            walk_javascript_declaration(left, source, scopes, scan, deadline)?;
        } else {
            let mut defaults = Vec::new();
            collect_javascript_pattern_bindings(
                left,
                source,
                "loop_variable",
                scopes.last_mut().expect("loop scope is active"),
                scan,
                &mut defaults,
            )?;
            for default in defaults {
                walk_javascript_node(default, source, scopes, scan, deadline)?;
            }
        }
    }
    if let Some(right) = node.child_by_field_name("right") {
        walk_javascript_node(right, source, scopes, scan, deadline)?;
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
        is_function_scope: false,
        bindings: Vec::new(),
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
        is_function_scope: false,
        bindings: Vec::new(),
    });
    if let Some(parameter) = node.child_by_field_name("parameter") {
        let mut defaults = Vec::new();
        collect_javascript_pattern_bindings(
            parameter,
            source,
            "catch_parameter",
            scopes.last_mut().expect("catch scope is active"),
            scan,
            &mut defaults,
        )?;
        for default in defaults {
            walk_javascript_node(default, source, scopes, scan, deadline)?;
        }
    }
    if let Some(body) = node.child_by_field_name("body") {
        walk_javascript_node(body, source, scopes, scan, deadline)?;
    }
    scopes.pop();
    Ok(())
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
    let visible = scopes
        .iter()
        .rev()
        .any(|scope| scope.bindings.iter().any(|binding| binding.name == name));
    if visible {
        scan.local_references.insert(name);
    } else {
        scan.external_references.insert(name);
    }
    Ok(())
}

fn javascript_var_target_scope_index(scopes: &[Scope]) -> Option<usize> {
    for (index, scope) in scopes.iter().enumerate().rev() {
        if scope.is_function_scope {
            return Some(index);
        }
    }
    if scopes.is_empty() { None } else { Some(0) }
}

/// Collects the parameter bindings of a callable node plus the default-value
/// expressions that must be walked inside the new function scope.
fn collect_javascript_callable_bindings<'tree>(
    node: Node<'tree>,
    source: &str,
    scan: &mut JavaScriptScopeScan,
) -> Result<(Scope, Vec<Node<'tree>>)> {
    let mut scope = Scope {
        is_function_scope: true,
        bindings: Vec::new(),
    };
    let mut defaults = Vec::new();
    // A named function expression binds its own name inside its scope.
    if node.kind() == "function_expression"
        && let Some(name_node) = node.child_by_field_name("name")
        && name_node.kind() == "identifier"
    {
        bind_javascript_name(name_node, source, "function_expression", &mut scope, scan)?;
    }
    let parameters = node
        .child_by_field_name("parameters")
        .or_else(|| node.child_by_field_name("parameter"));
    if let Some(parameters) = parameters {
        if parameters.kind() == "identifier" {
            bind_javascript_name(parameters, source, "parameter", &mut scope, scan)?;
        } else {
            let mut cursor = parameters.walk();
            for parameter in parameters.named_children(&mut cursor) {
                if parameter.kind() == "identifier" {
                    bind_javascript_name(parameter, source, "parameter", &mut scope, scan)?;
                } else if let Some(pattern) = javascript_parameter_pattern(parameter) {
                    collect_javascript_pattern_bindings(
                        pattern,
                        source,
                        "parameter",
                        &mut scope,
                        scan,
                        &mut defaults,
                    )?;
                }
            }
        }
    }
    Ok((scope, defaults))
}

/// Returns the binding pattern of a single formal parameter, falling back to
/// the first pattern-shaped child for parameter-property spellings such as
/// `constructor(private initial: T)` where the pattern is not a field.
fn javascript_parameter_pattern(node: Node<'_>) -> Option<Node<'_>> {
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

fn bind_javascript_name(
    name_node: Node<'_>,
    source: &str,
    node_kind: &'static str,
    scope: &mut Scope,
    scan: &mut JavaScriptScopeScan,
) -> Result<()> {
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() {
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
            | "enum_body"
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
