use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
use crate::semantic::go::{
    go_parameters, go_return_type, go_semantic_path, go_signature, go_symbol_name,
    is_go_symbol_node,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::symbol_reference_compat::reference_facts_from_legacy;
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_go_symbols_with_deadline(
    path: &Path,
    source: &str,
    root: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();
    collect_symbols(path, source, root, deadline, &mut symbols)?;
    Ok(symbols)
}

fn collect_symbols(
    path: &Path,
    source: &str,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
    symbols: &mut Vec<IndexedSymbol>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting Go symbols")?;
    }
    if is_go_symbol_node(node)
        && let Some(symbol) = indexed_symbol(path, source, node, deadline)?
    {
        symbols.push(symbol);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_symbols(path, source, child, deadline, symbols)?;
    }
    Ok(())
}

fn indexed_symbol(
    path: &Path,
    source: &str,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<IndexedSymbol>> {
    let Some(name) = go_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = go_semantic_path(node, source, &name)? else {
        return Ok(None);
    };
    let scope_path = semantic_path
        .rsplit_once("::")
        .map(|(scope_path, _)| scope_path.to_string());
    let references_by_name = collect_direct_local_calls(node, source, deadline)?;
    let call_arities_by_name = BTreeMap::new();

    Ok(Some(IndexedSymbol {
        symbol_id: semantic_path.clone(),
        base_name: symbol_base_name(&semantic_path),
        semantic_path,
        scope_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: go_signature(node, source),
        is_overload: false,
        parameters: go_parameters(node, source),
        return_type: go_return_type(node, source),
        docstring: None,
        reference_facts: reference_facts_from_legacy(&references_by_name, &call_arities_by_name),
        references_by_name,
        call_arities_by_name,
    }))
}

struct GoMethodReceiver {
    name: String,
    type_name: String,
}

struct GoLocalVariableType {
    type_name: String,
    available_after: usize,
}

struct GoDirectCallContext<'a> {
    local_functions: &'a BTreeMap<String, String>,
    bindings: &'a BTreeSet<String>,
    method_receiver: Option<&'a GoMethodReceiver>,
    parameter_types: &'a BTreeMap<String, String>,
    local_variable_types: &'a BTreeMap<String, GoLocalVariableType>,
    function_body_range: (usize, usize),
    body_bindings: &'a BTreeSet<String>,
}

fn collect_direct_local_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<BTreeSet<String>> {
    if !matches!(
        symbol_node.kind(),
        "function_declaration" | "method_declaration"
    ) {
        return Ok(BTreeSet::new());
    }
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(BTreeSet::new());
    };
    let local_functions = source_file_function_paths(symbol_node, source)?;
    let method_receiver = go_method_receiver_binding(symbol_node, source)?;
    let parameter_types = go_named_parameter_types(symbol_node, source)?;
    let local_variable_types = go_function_body_local_variable_types(body, source)?;

    let mut bindings = BTreeSet::new();
    collect_function_bindings(symbol_node, source, &mut bindings)?;
    let mut body_bindings = BTreeSet::new();
    collect_body_bindings(body, source, &mut body_bindings)?;
    let context = GoDirectCallContext {
        local_functions: &local_functions,
        bindings: &bindings,
        method_receiver: method_receiver.as_ref(),
        parameter_types: &parameter_types,
        local_variable_types: &local_variable_types,
        function_body_range: (body.start_byte(), body.end_byte()),
        body_bindings: &body_bindings,
    };
    let mut references = BTreeSet::new();
    collect_direct_local_calls_from_node(body, source, deadline, &context, &mut references)?;
    Ok(references)
}

fn go_method_receiver_binding(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<Option<GoMethodReceiver>> {
    if symbol_node.kind() != "method_declaration" {
        return Ok(None);
    }
    let Some(name) = go_symbol_name(symbol_node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = go_semantic_path(symbol_node, source, &name)? else {
        return Ok(None);
    };
    let Some((type_name, _)) = semantic_path.split_once("::") else {
        return Ok(None);
    };
    let Some(receiver) = symbol_node.child_by_field_name("receiver") else {
        return Ok(None);
    };
    let mut cursor = receiver.walk();
    let Some(parameter) = receiver.named_children(&mut cursor).next() else {
        return Ok(None);
    };
    let Some(receiver_name) = parameter.child_by_field_name("name") else {
        return Ok(None);
    };
    let receiver_name = node_text(receiver_name, source)?.trim();
    if receiver_name.is_empty() || receiver_name == "_" {
        return Ok(None);
    }
    Ok(Some(GoMethodReceiver {
        name: receiver_name.to_string(),
        type_name: type_name.to_string(),
    }))
}

fn go_named_parameter_types(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, String>> {
    let Some(parameters) = symbol_node.child_by_field_name("parameters") else {
        return Ok(BTreeMap::new());
    };
    let mut parameter_types = BTreeMap::new();
    let mut ambiguous_names = BTreeSet::new();
    let mut cursor = parameters.walk();
    for parameter in parameters.named_children(&mut cursor) {
        if parameter.kind() != "parameter_declaration" {
            continue;
        }
        let Some(type_node) = parameter.child_by_field_name("type") else {
            continue;
        };
        let Some(type_name) = go_named_local_type(type_node, source)? else {
            continue;
        };
        let mut name_cursor = parameter.walk();
        for name in parameter.children_by_field_name("name", &mut name_cursor) {
            let name = node_text(name, source)?.trim();
            if name.is_empty() || name == "_" {
                continue;
            }
            if parameter_types
                .insert(name.to_string(), type_name.clone())
                .is_some()
            {
                ambiguous_names.insert(name.to_string());
            }
        }
    }
    parameter_types.retain(|name, _| !ambiguous_names.contains(name));
    Ok(parameter_types)
}

fn go_named_local_type(node: Node<'_>, source: &str) -> Result<Option<String>> {
    match node.kind() {
        "type_identifier" => node_text(node, source)
            .map(str::trim)
            .map(str::to_string)
            .map(Some),
        "generic_type" => node
            .child_by_field_name("type")
            .map(|inner| go_named_local_type(inner, source))
            .transpose()
            .map(Option::flatten),
        "pointer_type" | "parenthesized_type" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .next()
                .map(|inner| go_named_local_type(inner, source))
                .transpose()
                .map(Option::flatten)
        }
        _ => Ok(None),
    }
}

fn go_function_body_local_variable_types(
    body: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, GoLocalVariableType>> {
    let mut local_variable_types = BTreeMap::new();
    let mut ambiguous_names = BTreeSet::new();
    let mut body_cursor = body.walk();
    let Some(statement_list) = body
        .named_children(&mut body_cursor)
        .find(|node| node.kind() == "statement_list")
    else {
        return Ok(local_variable_types);
    };
    let mut statement_cursor = statement_list.walk();
    for statement in statement_list.named_children(&mut statement_cursor) {
        match statement.kind() {
            "var_declaration" => collect_go_var_declaration_types(
                statement,
                source,
                &mut local_variable_types,
                &mut ambiguous_names,
            )?,
            "short_var_declaration" => collect_go_short_variable_declaration_types(
                statement,
                source,
                &mut local_variable_types,
                &mut ambiguous_names,
            )?,
            _ => {}
        }
    }
    local_variable_types.retain(|name, _| !ambiguous_names.contains(name));
    Ok(local_variable_types)
}

fn collect_go_var_declaration_types(
    declaration: Node<'_>,
    source: &str,
    local_variable_types: &mut BTreeMap<String, GoLocalVariableType>,
    ambiguous_names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut cursor = declaration.walk();
    for node in declaration.named_children(&mut cursor) {
        match node.kind() {
            "var_spec" => {
                collect_go_var_spec_types(node, source, local_variable_types, ambiguous_names)?
            }
            "var_spec_list" => {
                let mut spec_cursor = node.walk();
                for spec in node.named_children(&mut spec_cursor) {
                    if spec.kind() == "var_spec" {
                        collect_go_var_spec_types(
                            spec,
                            source,
                            local_variable_types,
                            ambiguous_names,
                        )?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_go_var_spec_types(
    spec: Node<'_>,
    source: &str,
    local_variable_types: &mut BTreeMap<String, GoLocalVariableType>,
    ambiguous_names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut name_cursor = spec.walk();
    let names = spec
        .children_by_field_name("name", &mut name_cursor)
        .map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .collect::<Result<Vec<_>>>()?;
    if names.is_empty() || names.iter().any(|name| name.is_empty() || name == "_") {
        return Ok(());
    }

    let type_name = if let Some(type_node) = spec.child_by_field_name("type") {
        go_named_local_type(type_node, source)?
    } else if let Some(type_node) = spec
        .child_by_field_name("value")
        .and_then(go_single_composite_literal_type)
    {
        go_named_local_type(type_node, source)?
    } else {
        None
    };
    let Some(type_name) = type_name else {
        return Ok(());
    };
    for name in names {
        insert_go_local_variable_type(
            local_variable_types,
            ambiguous_names,
            name,
            type_name.clone(),
            spec.end_byte(),
        );
    }
    Ok(())
}

fn collect_go_short_variable_declaration_types(
    declaration: Node<'_>,
    source: &str,
    local_variable_types: &mut BTreeMap<String, GoLocalVariableType>,
    ambiguous_names: &mut BTreeSet<String>,
) -> Result<()> {
    let Some(left) = declaration.child_by_field_name("left") else {
        return Ok(());
    };
    let Some(right) = declaration.child_by_field_name("right") else {
        return Ok(());
    };
    let mut left_cursor = left.walk();
    let names = left
        .named_children(&mut left_cursor)
        .map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .collect::<Result<Vec<_>>>()?;
    let mut right_cursor = right.walk();
    let values = right.named_children(&mut right_cursor).collect::<Vec<_>>();
    if names.is_empty() || names.len() != values.len() {
        return Ok(());
    }
    for (name, value) in names.into_iter().zip(values) {
        if name.is_empty() || name == "_" {
            continue;
        }
        let Some(type_node) = go_single_composite_literal_type(value) else {
            continue;
        };
        let Some(type_name) = go_named_local_type(type_node, source)? else {
            continue;
        };
        insert_go_local_variable_type(
            local_variable_types,
            ambiguous_names,
            name,
            type_name,
            declaration.end_byte(),
        );
    }
    Ok(())
}

fn go_single_composite_literal_type(node: Node<'_>) -> Option<Node<'_>> {
    let literal = if node.kind() == "composite_literal" {
        Some(node)
    } else if node.kind() == "expression_list" {
        let mut cursor = node.walk();
        let mut expressions = node.named_children(&mut cursor);
        let literal = expressions.next()?;
        (expressions.next().is_none() && literal.kind() == "composite_literal").then_some(literal)
    } else {
        None
    };
    literal.and_then(|literal| literal.child_by_field_name("type"))
}

fn insert_go_local_variable_type(
    local_variable_types: &mut BTreeMap<String, GoLocalVariableType>,
    ambiguous_names: &mut BTreeSet<String>,
    name: String,
    type_name: String,
    available_after: usize,
) {
    if local_variable_types
        .insert(
            name.clone(),
            GoLocalVariableType {
                type_name,
                available_after,
            },
        )
        .is_some()
    {
        ambiguous_names.insert(name);
    }
}

fn go_local_variable_type_for_operand(
    operand: Node<'_>,
    source: &str,
    context: &GoDirectCallContext<'_>,
) -> Option<String> {
    if !go_operand_is_in_function_body_scope(operand, context.function_body_range) {
        return None;
    }
    let name = node_text(operand, source).ok()?.trim();
    let local_type = context.local_variable_types.get(name)?;
    (local_type.available_after <= operand.start_byte()).then(|| local_type.type_name.clone())
}

fn go_operand_is_in_function_body_scope(
    operand: Node<'_>,
    function_body_range: (usize, usize),
) -> bool {
    let mut current = operand.parent();
    while let Some(node) = current {
        if node.kind() == "block" && (node.start_byte(), node.end_byte()) != function_body_range {
            return false;
        }
        if matches!(
            node.kind(),
            "if_statement"
                | "for_statement"
                | "expression_switch_statement"
                | "type_switch_statement"
                | "select_statement"
        ) {
            return false;
        }
        current = node.parent();
    }
    true
}

fn source_file_function_paths(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, String>> {
    let mut root = symbol_node;
    while let Some(parent) = root.parent() {
        root = parent;
    }

    let mut paths_by_name = BTreeMap::<String, Vec<String>>::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "function_declaration" {
            continue;
        }
        let Some(name) = go_symbol_name(child, source)? else {
            continue;
        };
        let Some(path) = go_semantic_path(child, source, &name)? else {
            continue;
        };
        paths_by_name.entry(name).or_default().push(path);
    }

    Ok(paths_by_name
        .into_iter()
        .filter_map(|(name, paths)| (paths.len() == 1).then(|| (name, paths[0].clone())))
        .collect())
}

fn collect_function_bindings(
    symbol_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    for field_name in ["receiver", "parameters"] {
        if let Some(parameters) = symbol_node.child_by_field_name(field_name) {
            collect_parameter_bindings(parameters, source, bindings)?;
        }
    }
    if let Some(body) = symbol_node.child_by_field_name("body") {
        collect_body_bindings(body, source, bindings)?;
    }
    Ok(())
}

fn collect_parameter_bindings(
    parameters: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    let mut cursor = parameters.walk();
    for parameter in parameters.named_children(&mut cursor) {
        collect_field_name_bindings(parameter, source, bindings)?;
    }
    Ok(())
}

fn collect_body_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    if node.kind() == "function_literal" {
        return Ok(());
    }
    match node.kind() {
        "var_spec" | "const_spec" | "parameter_declaration" | "variadic_parameter_declaration" => {
            collect_field_name_bindings(node, source, bindings)?
        }
        "short_var_declaration" | "range_clause" => {
            if let Some(left) = node.child_by_field_name("left") {
                collect_expression_list_identifier_bindings(left, source, bindings)?;
            }
        }
        "type_switch_statement" => {
            if let Some(alias) = node.child_by_field_name("alias") {
                collect_expression_list_identifier_bindings(alias, source, bindings)?;
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_body_bindings(child, source, bindings)?;
    }
    Ok(())
}

fn collect_field_name_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    let mut cursor = node.walk();
    for name in node.children_by_field_name("name", &mut cursor) {
        collect_identifier_binding(name, source, bindings)?;
    }
    Ok(())
}

fn collect_expression_list_identifier_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "identifier" {
            collect_identifier_binding(child, source, bindings)?;
        }
    }
    Ok(())
}

fn collect_identifier_binding(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    let name = node_text(node, source)?.trim();
    if !name.is_empty() && name != "_" {
        bindings.insert(name.to_string());
    }
    Ok(())
}

fn go_imported_selector_reference(
    selector: Node<'_>,
    source: &str,
    bindings: &BTreeSet<String>,
) -> Result<Option<String>> {
    let Some(operand) = selector.child_by_field_name("operand") else {
        return Ok(None);
    };
    let Some(field) = selector.child_by_field_name("field") else {
        return Ok(None);
    };
    if operand.kind() != "identifier" || field.kind() != "field_identifier" {
        return Ok(None);
    }

    let local_name = node_text(operand, source)?.trim();
    let imported_name = node_text(field, source)?.trim();
    if local_name.is_empty()
        || imported_name.is_empty()
        || bindings.contains(local_name)
        || !imported_name.chars().next().is_some_and(char::is_uppercase)
    {
        return Ok(None);
    }

    Ok(Some(format!("{local_name}.{imported_name}")))
}

fn go_direct_static_method_receiver_type(node: Node<'_>, source: &str) -> Result<Option<String>> {
    match node.kind() {
        "composite_literal" => node
            .child_by_field_name("type")
            .map(|type_node| go_named_local_type(type_node, source))
            .transpose()
            .map(Option::flatten),
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .next()
                .map(|inner| go_direct_static_method_receiver_type(inner, source))
                .transpose()
                .map(Option::flatten)
        }
        "unary_expression" => {
            let Some(operator) = node.child_by_field_name("operator") else {
                return Ok(None);
            };
            if node_text(operator, source)?.trim() != "&" {
                return Ok(None);
            }
            node.child_by_field_name("operand")
                .map(|operand| go_direct_static_method_receiver_type(operand, source))
                .transpose()
                .map(Option::flatten)
        }
        _ => Ok(None),
    }
}

fn go_direct_method_reference(
    selector: Node<'_>,
    source: &str,
    context: &GoDirectCallContext<'_>,
) -> Result<Option<String>> {
    let Some(operand) = selector.child_by_field_name("operand") else {
        return Ok(None);
    };
    let Some(field) = selector.child_by_field_name("field") else {
        return Ok(None);
    };
    if field.kind() != "field_identifier" {
        return Ok(None);
    }
    let method_name = node_text(field, source)?.trim();
    if method_name.is_empty() {
        return Ok(None);
    }

    let receiver_type =
        if let Some(receiver_type) = go_direct_static_method_receiver_type(operand, source)? {
            Some(receiver_type)
        } else if operand.kind() == "identifier" {
            let receiver_name = node_text(operand, source)?.trim();
            let receiver_type = context.method_receiver.and_then(|receiver| {
                (receiver_name == receiver.name && !context.body_bindings.contains(receiver_name))
                    .then(|| receiver.type_name.clone())
            });
            receiver_type
                .or_else(|| {
                    (!context.body_bindings.contains(receiver_name))
                        .then(|| context.parameter_types.get(receiver_name).cloned())
                        .flatten()
                })
                .or_else(|| go_local_variable_type_for_operand(operand, source, context))
        } else {
            None
        };
    Ok(receiver_type.map(|receiver_type| format!("{receiver_type}::{method_name}")))
}

fn collect_direct_local_calls_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    context: &GoDirectCallContext<'_>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting Go direct calls")?;
    }
    if node.kind() == "function_literal" {
        return Ok(());
    }
    if node.kind() == "call_expression"
        && let Some(function) = node.child_by_field_name("function")
    {
        match function.kind() {
            "identifier" => {
                let name = node_text(function, source)?.trim();
                if !name.is_empty() && !context.bindings.contains(name) {
                    references.insert(
                        context
                            .local_functions
                            .get(name)
                            .cloned()
                            .unwrap_or_else(|| name.to_string()),
                    );
                }
            }
            "selector_expression" => {
                if let Some(reference) =
                    go_imported_selector_reference(function, source, context.bindings)?
                {
                    references.insert(reference);
                } else if let Some(reference) =
                    go_direct_method_reference(function, source, context)?
                {
                    references.insert(reference);
                }
            }
            _ => {}
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_direct_local_calls_from_node(child, source, deadline, context, references)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_go_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_go_named_types_functions_methods_and_unshadowed_direct_calls() {
        let source = r#"
package metrics

type Counter struct { value int }
type Box[T any] struct{}
type Other struct{}
type Alias = Counter

func helper() int { return 1 }
func direct() int { return helper() }
func shadowed_parameter(helper func() int) int { return helper() }
func shadowed_variable() int {
    helper := func() int { return 2 }
    return helper()
}
func NewCounter(value int) Counter { return Counter{value: value} }
func imported() int { return service.Value() }
func shadowed_selector() int {
    service := Counter{}
    return service.Value()
}
func literal_method() int { return Counter{}.Value() }
func (Counter) Value() int { return 3 }
func (Box[T]) Value() int { return 5 }
func (Other) Value() int { return 4 }
func pointer_literal_method_call() int { return (&Counter{}).Value() }
func generic_literal_method_call() int { return Box[int]{}.Value() }
func local_short_call() int { counter := Counter{}; return counter.Value() }
func local_var_call() int { var counter *Counter; return counter.Value() }
func local_var_literal_call() int { var counter = Counter{}; return counter.Value() }
func call_before_local_declaration() int {
    counter.Value()
    counter := Counter{}
    return 0
}
func nested_local_method_call() int {
    counter := Counter{}
    if true {
        counter := Other{}
        return counter.Value()
    }
    return 0
}
func parameter_call(counter Counter) int { return counter.Value() }
func shadowed_parameter_method(counter Counter) int {
    if true {
        counter := Other{}
        return counter.Value()
    }
    return 0
}
func (counter *Counter) receiver_call() int { return counter.Value() }
func (counter *Counter) shadowed_receiver() int {
    if true {
        counter := Other{}
        return counter.Value()
    }
    return 0
}
func (counter *Counter) Increment(amount int) int { return helper() + amount }
"#;
        let path = Path::new("metrics.go");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_go_symbols_with_deadline(path, source, document.tree.root_node(), None).unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Counter",
                "Box",
                "Other",
                "Alias",
                "helper",
                "direct",
                "shadowed_parameter",
                "shadowed_variable",
                "NewCounter",
                "imported",
                "shadowed_selector",
                "literal_method",
                "Counter::Value",
                "Box::Value",
                "Other::Value",
                "pointer_literal_method_call",
                "generic_literal_method_call",
                "local_short_call",
                "local_var_call",
                "local_var_literal_call",
                "call_before_local_declaration",
                "nested_local_method_call",
                "parameter_call",
                "shadowed_parameter_method",
                "Counter::receiver_call",
                "Counter::shadowed_receiver",
                "Counter::Increment",
            ]
        );
        let increment = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Counter::Increment")
            .unwrap();
        assert_eq!(increment.parameters, vec!["amount int"]);
        assert_eq!(increment.return_type.as_deref(), Some("int"));
        assert_eq!(increment.references_by_name, ["helper".to_string()].into());

        for caller_path in ["direct", "Counter::Increment"] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert_eq!(caller.references_by_name, ["helper".to_string()].into());
            assert_eq!(caller.reference_facts.len(), 1);
        }
        for caller_path in [
            "local_short_call",
            "local_var_call",
            "local_var_literal_call",
        ] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert_eq!(
                caller.references_by_name,
                ["Counter::Value".to_string()].into(),
                "{caller_path}"
            );
            assert_eq!(caller.reference_facts.len(), 1, "{caller_path}");
        }
        for caller_path in ["call_before_local_declaration", "nested_local_method_call"] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert!(caller.references_by_name.is_empty(), "{caller_path}");
            assert!(caller.reference_facts.is_empty(), "{caller_path}");
        }

        let parameter_call = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "parameter_call")
            .unwrap();
        assert_eq!(
            parameter_call.references_by_name,
            ["Counter::Value".to_string()].into()
        );
        assert_eq!(parameter_call.reference_facts.len(), 1);
        let shadowed_parameter_method = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "shadowed_parameter_method")
            .unwrap();
        assert!(shadowed_parameter_method.references_by_name.is_empty());
        assert!(shadowed_parameter_method.reference_facts.is_empty());

        let receiver_call = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Counter::receiver_call")
            .unwrap();
        assert_eq!(
            receiver_call.references_by_name,
            ["Counter::Value".to_string()].into()
        );
        assert_eq!(receiver_call.reference_facts.len(), 1);
        let shadowed_receiver = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Counter::shadowed_receiver")
            .unwrap();
        assert!(shadowed_receiver.references_by_name.is_empty());
        assert!(shadowed_receiver.reference_facts.is_empty());

        let generic_literal_method_call = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "generic_literal_method_call")
            .unwrap();
        assert_eq!(
            generic_literal_method_call.references_by_name,
            ["Box::Value".to_string()].into()
        );

        let pointer_literal_method_call = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "pointer_literal_method_call")
            .unwrap();
        assert_eq!(
            pointer_literal_method_call.references_by_name,
            ["Counter::Value".to_string()].into()
        );
        let shadowed_selector = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "shadowed_selector")
            .unwrap();
        assert_eq!(
            shadowed_selector.references_by_name,
            ["Counter::Value".to_string()].into()
        );
        assert_eq!(shadowed_selector.reference_facts.len(), 1);

        let literal_method = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "literal_method")
            .unwrap();
        assert_eq!(
            literal_method.references_by_name,
            ["Counter::Value".to_string()].into()
        );
        assert_eq!(literal_method.reference_facts.len(), 1);

        let imported = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "imported")
            .unwrap();
        assert_eq!(
            imported.references_by_name,
            ["service.Value".to_string()].into()
        );
        assert_eq!(imported.reference_facts.len(), 1);
        for caller_path in ["shadowed_parameter", "shadowed_variable"] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert!(caller.references_by_name.is_empty(), "{caller_path}");
            assert!(caller.reference_facts.is_empty(), "{caller_path}");
        }
    }
}
