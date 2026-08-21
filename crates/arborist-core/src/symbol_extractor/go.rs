use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
use crate::semantic::go::{
    go_parameters, go_return_type, go_semantic_path, go_signature, go_symbol_name,
    is_go_symbol_node,
};
use crate::symbol_index_model::{
    GoReferenceDetails, IndexedSymbol, ReferenceFact, ReferenceLanguageDetails, symbol_base_name,
};
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
    let direct_references = collect_direct_local_calls(node, source, deadline)?;
    let references_by_name = direct_references.references_by_name;
    let call_arities_by_name = BTreeMap::new();
    let reference_facts = go_reference_facts(
        &references_by_name,
        &direct_references.type_assertion_method_references,
        &direct_references.type_conversion_method_references,
        &call_arities_by_name,
    );

    Ok(Some(IndexedSymbol {
        extension_receiver: None,
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
        reference_facts,
        references_by_name,
        call_arities_by_name,
    }))
}

struct GoDirectReferences {
    references_by_name: BTreeSet<String>,
    type_assertion_method_references: BTreeSet<String>,
    type_conversion_method_references: BTreeSet<String>,
    suppressed_type_conversion_call_starts: BTreeSet<usize>,
}

enum GoDirectMethodReference {
    Plain(String),
    TypeAssertion(String),
    TypeConversion {
        method_path: String,
        conversion_call_start: usize,
    },
}

struct GoMethodReceiver {
    name: String,
    type_name: String,
}

struct GoLocalVariableType {
    type_name: String,
    available_after: usize,
    scope_range: (usize, usize),
}

struct GoDirectCallContext<'a> {
    local_functions: &'a BTreeMap<String, String>,
    bindings: &'a BTreeSet<String>,
    method_receiver: Option<&'a GoMethodReceiver>,
    parameter_types: &'a BTreeMap<String, String>,
    local_variable_types: &'a BTreeMap<String, Vec<GoLocalVariableType>>,
    function_body_range: (usize, usize),
    body_bindings: &'a BTreeSet<String>,
}

fn go_reference_facts(
    references_by_name: &BTreeSet<String>,
    type_assertion_method_references: &BTreeSet<String>,
    type_conversion_method_references: &BTreeSet<String>,
    call_arities_by_name: &BTreeMap<String, BTreeSet<usize>>,
) -> Vec<ReferenceFact> {
    let mut reference_facts = reference_facts_from_legacy(references_by_name, call_arities_by_name);
    reference_facts.extend(
        type_assertion_method_references
            .iter()
            .filter(|reference| !references_by_name.contains(*reference))
            .map(|spelling| ReferenceFact {
                spelling: spelling.clone(),
                call_arities: None,
                language_details: ReferenceLanguageDetails::Go(GoReferenceDetails {
                    type_conversion: false,
                    type_assertion: true,
                }),
            }),
    );
    reference_facts.extend(
        type_conversion_method_references
            .iter()
            .filter(|reference| !references_by_name.contains(*reference))
            .map(|spelling| ReferenceFact {
                spelling: spelling.clone(),
                call_arities: None,
                language_details: ReferenceLanguageDetails::Go(GoReferenceDetails {
                    type_conversion: true,
                    type_assertion: false,
                }),
            }),
    );
    reference_facts
}

fn collect_direct_local_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<GoDirectReferences> {
    if !matches!(
        symbol_node.kind(),
        "function_declaration" | "method_declaration"
    ) {
        return Ok(GoDirectReferences {
            references_by_name: BTreeSet::new(),
            type_assertion_method_references: BTreeSet::new(),
            type_conversion_method_references: BTreeSet::new(),
            suppressed_type_conversion_call_starts: BTreeSet::new(),
        });
    }
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(GoDirectReferences {
            references_by_name: BTreeSet::new(),
            type_assertion_method_references: BTreeSet::new(),
            type_conversion_method_references: BTreeSet::new(),
            suppressed_type_conversion_call_starts: BTreeSet::new(),
        });
    };
    let local_functions = source_file_function_paths(symbol_node, source)?;
    let local_type_names = source_file_type_names(symbol_node, source)?;
    let local_type_alias_targets = source_file_type_alias_targets(symbol_node, source)?;
    let local_factory_return_types = source_file_function_return_types(
        symbol_node,
        source,
        &local_type_names,
        &local_type_alias_targets,
    )?;
    let method_receiver = go_method_receiver_binding(symbol_node, source)?;
    let parameter_types = go_named_parameter_types(symbol_node, source)?;
    let mut bindings = BTreeSet::new();
    collect_function_bindings(symbol_node, source, &mut bindings)?;
    let local_variable_types = go_function_body_local_variable_types(
        body,
        source,
        &local_type_names,
        &local_factory_return_types,
        &bindings,
    )?;
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
    let mut references = GoDirectReferences {
        references_by_name: BTreeSet::new(),
        type_assertion_method_references: BTreeSet::new(),
        type_conversion_method_references: BTreeSet::new(),
        suppressed_type_conversion_call_starts: BTreeSet::new(),
    };
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
        "qualified_type" => {
            let package = node.child_by_field_name("package");
            let name = node.child_by_field_name("name");
            match (package, name) {
                (Some(package), Some(name)) => {
                    let package = node_text(package, source)?.trim();
                    let name = node_text(name, source)?.trim();
                    if package.is_empty() || name.is_empty() {
                        Ok(None)
                    } else {
                        Ok(Some(format!("{package}.{name}")))
                    }
                }
                _ => Ok(None),
            }
        }
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
    local_type_names: &BTreeSet<String>,
    local_factory_return_types: &BTreeMap<String, String>,
    bindings: &BTreeSet<String>,
) -> Result<BTreeMap<String, Vec<GoLocalVariableType>>> {
    let mut local_variable_types = BTreeMap::new();
    let mut ambiguous_names = BTreeSet::new();
    let function_body_range = (body.start_byte(), body.end_byte());
    collect_go_local_variable_types_in_scope(
        body,
        source,
        function_body_range,
        &mut local_variable_types,
        &mut ambiguous_names,
        local_type_names,
        local_factory_return_types,
        bindings,
    )?;
    for name in ambiguous_names {
        local_variable_types.remove(&name);
    }
    Ok(local_variable_types)
}

fn collect_go_local_variable_types_in_scope(
    node: Node<'_>,
    source: &str,
    scope_range: (usize, usize),
    local_variable_types: &mut BTreeMap<String, Vec<GoLocalVariableType>>,
    ambiguous_names: &mut BTreeSet<String>,
    local_type_names: &BTreeSet<String>,
    local_factory_return_types: &BTreeMap<String, String>,
    bindings: &BTreeSet<String>,
) -> Result<()> {
    if node.kind() == "function_literal" {
        return Ok(());
    }
    if node.kind() == "var_declaration" {
        collect_go_var_declaration_types(
            node,
            source,
            local_variable_types,
            ambiguous_names,
            scope_range,
            local_type_names,
            local_factory_return_types,
            bindings,
        )?;
    } else if node.kind() == "short_var_declaration" {
        collect_go_short_variable_declaration_types(
            node,
            source,
            local_variable_types,
            ambiguous_names,
            scope_range,
            local_type_names,
            local_factory_return_types,
            bindings,
        )?;
    }
    let next_scope_range = if node.kind() == "block" {
        (node.start_byte(), node.end_byte())
    } else {
        scope_range
    };
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_go_local_variable_types_in_scope(
            child,
            source,
            next_scope_range,
            local_variable_types,
            ambiguous_names,
            local_type_names,
            local_factory_return_types,
            bindings,
        )?;
    }
    Ok(())
}

fn collect_go_var_declaration_types(
    declaration: Node<'_>,
    source: &str,
    local_variable_types: &mut BTreeMap<String, Vec<GoLocalVariableType>>,
    ambiguous_names: &mut BTreeSet<String>,
    scope_range: (usize, usize),
    local_type_names: &BTreeSet<String>,
    local_factory_return_types: &BTreeMap<String, String>,
    bindings: &BTreeSet<String>,
) -> Result<()> {
    let mut cursor = declaration.walk();
    for node in declaration.named_children(&mut cursor) {
        match node.kind() {
            "var_spec" => collect_go_var_spec_types(
                node,
                source,
                local_variable_types,
                ambiguous_names,
                scope_range,
                local_type_names,
                local_factory_return_types,
                bindings,
            )?,
            "var_spec_list" => {
                let mut spec_cursor = node.walk();
                for spec in node.named_children(&mut spec_cursor) {
                    if spec.kind() == "var_spec" {
                        collect_go_var_spec_types(
                            spec,
                            source,
                            local_variable_types,
                            ambiguous_names,
                            scope_range,
                            local_type_names,
                            local_factory_return_types,
                            bindings,
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
    local_variable_types: &mut BTreeMap<String, Vec<GoLocalVariableType>>,
    ambiguous_names: &mut BTreeSet<String>,
    scope_range: (usize, usize),
    local_type_names: &BTreeSet<String>,
    local_factory_return_types: &BTreeMap<String, String>,
    bindings: &BTreeSet<String>,
) -> Result<()> {
    let mut name_cursor = spec.walk();
    let names = spec
        .children_by_field_name("name", &mut name_cursor)
        .map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .collect::<Result<Vec<_>>>()?;
    if names.is_empty() || names.iter().any(|name| name.is_empty() || name == "_") {
        return Ok(());
    }

    if let Some(type_node) = spec.child_by_field_name("type") {
        let Some(type_name) = go_named_local_type(type_node, source)? else {
            return Ok(());
        };
        for name in names {
            insert_go_local_variable_type(
                local_variable_types,
                ambiguous_names,
                name,
                type_name.clone(),
                spec.end_byte(),
                scope_range,
            );
        }
        return Ok(());
    }

    let Some(value) = spec.child_by_field_name("value") else {
        return Ok(());
    };
    let mut value_cursor = value.walk();
    let values = if value.kind() == "expression_list" {
        value.named_children(&mut value_cursor).collect::<Vec<_>>()
    } else {
        vec![value]
    };
    if values.len() != names.len() {
        return Ok(());
    }
    for (name, value) in names.into_iter().zip(values) {
        let Some(type_name) = go_single_local_initializer_type(
            value,
            source,
            local_type_names,
            local_factory_return_types,
            bindings,
        )?
        else {
            continue;
        };
        insert_go_local_variable_type(
            local_variable_types,
            ambiguous_names,
            name,
            type_name,
            spec.end_byte(),
            scope_range,
        );
    }
    Ok(())
}

fn collect_go_short_variable_declaration_types(
    declaration: Node<'_>,
    source: &str,
    local_variable_types: &mut BTreeMap<String, Vec<GoLocalVariableType>>,
    ambiguous_names: &mut BTreeSet<String>,
    scope_range: (usize, usize),
    local_type_names: &BTreeSet<String>,
    local_factory_return_types: &BTreeMap<String, String>,
    bindings: &BTreeSet<String>,
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
        let Some(type_name) = go_single_local_initializer_type(
            value,
            source,
            local_type_names,
            local_factory_return_types,
            bindings,
        )?
        else {
            continue;
        };
        insert_go_local_variable_type(
            local_variable_types,
            ambiguous_names,
            name,
            type_name,
            declaration.end_byte(),
            scope_range,
        );
    }
    Ok(())
}

fn go_single_local_initializer_type(
    node: Node<'_>,
    source: &str,
    local_type_names: &BTreeSet<String>,
    local_factory_return_types: &BTreeMap<String, String>,
    bindings: &BTreeSet<String>,
) -> Result<Option<String>> {
    if let Some(type_node) = go_single_composite_literal_type(node) {
        return go_named_local_type(type_node, source);
    }
    if node.kind() == "expression_list" {
        let mut cursor = node.walk();
        let mut expressions = node.named_children(&mut cursor);
        let Some(value) = expressions.next() else {
            return Ok(None);
        };
        if expressions.next().is_some() {
            return Ok(None);
        }
        return go_single_local_initializer_type(
            value,
            source,
            local_type_names,
            local_factory_return_types,
            bindings,
        );
    }
    if node.kind() == "call_expression"
        && let Some(function) = node.child_by_field_name("function")
        && let Some(function_name) = go_local_function_name(function, source)?
        && !bindings.contains(&function_name)
        && !local_type_names.contains(&function_name)
        && let Some(return_type) = local_factory_return_types.get(&function_name)
    {
        return Ok(Some(return_type.clone()));
    }
    let type_name = if node.kind() == "type_conversion_expression" {
        node.child_by_field_name("type")
            .map(|type_node| go_named_local_type(type_node, source))
            .transpose()?
            .flatten()
    } else {
        if node.kind() != "call_expression" {
            return Ok(None);
        }
        let Some(function) = node.child_by_field_name("function") else {
            return Ok(None);
        };
        match function.kind() {
            "generic_type" => go_named_local_type(function, source)?,
            _ => go_ambiguous_type_conversion_function_name(function, source)?,
        }
    };
    let Some(type_name) = type_name else {
        return Ok(None);
    };
    let local_name = type_name
        .rsplit_once('.')
        .map_or(type_name.as_str(), |(_, name)| name);
    Ok((!type_name.contains('.') && local_type_names.contains(local_name)).then_some(type_name))
}

fn go_local_function_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    match node.kind() {
        "identifier" => {
            let name = node_text(node, source)?.trim();
            Ok((!name.is_empty()).then(|| name.to_string()))
        }
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .next()
                .map(|inner| go_local_function_name(inner, source))
                .transpose()
                .map(Option::flatten)
        }
        _ => Ok(None),
    }
}

fn go_single_composite_literal_type(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "composite_literal" {
        return node.child_by_field_name("type");
    }
    if matches!(node.kind(), "parenthesized_expression" | "unary_expression") {
        let operator_is_address = node.kind() != "unary_expression"
            || node.child(0).is_some_and(|operator| operator.kind() == "&");
        return operator_is_address
            .then(|| {
                node.child_by_field_name("operand").or_else(|| {
                    let mut cursor = node.walk();
                    node.named_children(&mut cursor).next()
                })
            })
            .flatten()
            .and_then(go_single_composite_literal_type);
    }
    if node.kind() == "expression_list" {
        let mut cursor = node.walk();
        let mut expressions = node.named_children(&mut cursor);
        let literal = expressions.next()?;
        if expressions.next().is_some() {
            return None;
        }
        return go_single_composite_literal_type(literal);
    }
    None
}

fn insert_go_local_variable_type(
    local_variable_types: &mut BTreeMap<String, Vec<GoLocalVariableType>>,
    ambiguous_names: &mut BTreeSet<String>,
    name: String,
    type_name: String,
    available_after: usize,
    scope_range: (usize, usize),
) {
    let entries = local_variable_types.entry(name.clone()).or_default();
    if entries.iter().any(|entry| entry.scope_range == scope_range) {
        ambiguous_names.insert(name);
        return;
    }
    entries.push(GoLocalVariableType {
        type_name,
        available_after,
        scope_range,
    });
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
    let candidates = context.local_variable_types.get(name)?;
    let mut candidates = candidates
        .iter()
        .filter(|entry| {
            entry.available_after <= operand.start_byte()
                && entry.scope_range.0 <= operand.start_byte()
                && operand.end_byte() <= entry.scope_range.1
        })
        .collect::<Vec<_>>();
    let best_scope_size = candidates
        .iter()
        .map(|entry| entry.scope_range.1.saturating_sub(entry.scope_range.0))
        .min()?;
    candidates
        .retain(|entry| entry.scope_range.1.saturating_sub(entry.scope_range.0) == best_scope_size);
    (candidates.len() == 1).then(|| candidates[0].type_name.clone())
}

fn go_operand_is_in_function_body_scope(
    operand: Node<'_>,
    function_body_range: (usize, usize),
) -> bool {
    let mut current = operand.parent();
    while let Some(node) = current {
        if node.kind() == "function_literal" {
            return false;
        }
        if node.kind() == "block" && (node.start_byte(), node.end_byte()) == function_body_range {
            return true;
        }
        current = node.parent();
    }
    false
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

fn source_file_type_names(symbol_node: Node<'_>, source: &str) -> Result<BTreeSet<String>> {
    let mut root = symbol_node;
    while let Some(parent) = root.parent() {
        root = parent;
    }

    let mut type_names = BTreeSet::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "type_alias" => {
                if let Some(name) = child.child_by_field_name("name") {
                    let name = node_text(name, source)?.trim();
                    if !name.is_empty() {
                        type_names.insert(name.to_string());
                    }
                }
            }
            "type_declaration" => {
                let mut declaration_cursor = child.walk();
                for spec in child.named_children(&mut declaration_cursor) {
                    if !matches!(spec.kind(), "type_spec" | "type_alias") {
                        continue;
                    }
                    let Some(name) = spec.child_by_field_name("name") else {
                        continue;
                    };
                    let name = node_text(name, source)?.trim();
                    if !name.is_empty() {
                        type_names.insert(name.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    Ok(type_names)
}

fn source_file_type_alias_targets(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, String>> {
    let mut root = symbol_node;
    while let Some(parent) = root.parent() {
        root = parent;
    }

    let mut targets_by_name = BTreeMap::<String, Vec<String>>::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        let mut aliases = Vec::new();
        match child.kind() {
            "type_alias" => aliases.push(child),
            "type_declaration" => {
                let mut declaration_cursor = child.walk();
                aliases.extend(
                    child
                        .named_children(&mut declaration_cursor)
                        .filter(|spec| spec.kind() == "type_alias"),
                );
            }
            _ => {}
        }
        for alias in aliases {
            let (Some(name), Some(value)) = (
                alias.child_by_field_name("name"),
                alias.child_by_field_name("type"),
            ) else {
                continue;
            };
            let name = node_text(name, source)?.trim();
            let Some(target) = go_named_local_type(value, source)? else {
                continue;
            };
            if name.is_empty() || target.contains('.') {
                continue;
            }
            targets_by_name
                .entry(name.to_string())
                .or_default()
                .push(target);
        }
    }

    Ok(targets_by_name
        .into_iter()
        .filter_map(|(name, targets)| (targets.len() == 1).then(|| (name, targets[0].clone())))
        .collect())
}

fn go_resolve_local_type_alias(
    type_name: &str,
    local_type_names: &BTreeSet<String>,
    local_type_alias_targets: &BTreeMap<String, String>,
) -> Option<String> {
    let mut current = type_name.to_string();
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(current.clone()) {
            return None;
        }
        let Some(target) = local_type_alias_targets.get(&current) else {
            return local_type_names.contains(&current).then_some(current);
        };
        current = target.clone();
    }
}

fn go_single_function_result_type(result: Node<'_>, source: &str) -> Result<Option<String>> {
    if let Some(type_name) = go_named_local_type(result, source)? {
        return Ok(Some(type_name));
    }
    if result.kind() != "parameter_list" {
        return Ok(None);
    }

    let mut cursor = result.walk();
    let parameters = result
        .named_children(&mut cursor)
        .filter(|parameter| parameter.kind() == "parameter_declaration")
        .collect::<Vec<_>>();
    if parameters.len() != 1 {
        return Ok(None);
    }
    let parameter = parameters[0];
    let mut name_cursor = parameter.walk();
    if parameter
        .children_by_field_name("name", &mut name_cursor)
        .count()
        > 1
    {
        return Ok(None);
    }
    parameter
        .child_by_field_name("type")
        .map(|type_node| go_named_local_type(type_node, source))
        .transpose()
        .map(Option::flatten)
}

fn source_file_function_return_types(
    symbol_node: Node<'_>,
    source: &str,
    local_type_names: &BTreeSet<String>,
    local_type_alias_targets: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let mut root = symbol_node;
    while let Some(parent) = root.parent() {
        root = parent;
    }

    let mut return_types_by_name = BTreeMap::<String, Vec<String>>::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "function_declaration" {
            continue;
        }
        let Some(name) = go_symbol_name(child, source)? else {
            continue;
        };
        let Some(result) = child.child_by_field_name("result") else {
            continue;
        };
        let Some(type_name) = go_single_function_result_type(result, source)? else {
            continue;
        };
        let local_name = type_name
            .rsplit_once('.')
            .map_or(type_name.as_str(), |(_, name)| name);
        if type_name.contains('.') || !local_type_names.contains(local_name) {
            continue;
        }
        let Some(type_name) =
            go_resolve_local_type_alias(&type_name, local_type_names, local_type_alias_targets)
        else {
            continue;
        };
        return_types_by_name
            .entry(name)
            .or_default()
            .push(type_name);
    }

    Ok(return_types_by_name
        .into_iter()
        .filter_map(|(name, return_types)| {
            (return_types.len() == 1).then(|| (name, return_types[0].clone()))
        })
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
    if node.kind() == "identifier" {
        return collect_identifier_binding(node, source, bindings);
    }
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
) -> Result<Option<GoDirectMethodReference>> {
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
    if let Some(receiver_type) = receiver_type {
        if go_type_name_is_shadowed(&receiver_type, context.bindings) {
            return Ok(None);
        }
        return Ok(Some(GoDirectMethodReference::Plain(format!(
            "{receiver_type}::{method_name}"
        ))));
    }
    if let Some(type_name) = go_type_assertion_receiver(operand, source)? {
        if go_type_name_is_shadowed(&type_name, context.bindings) {
            return Ok(None);
        }
        return Ok(Some(GoDirectMethodReference::TypeAssertion(format!(
            "{type_name}::{method_name}"
        ))));
    }
    let Some((type_name, conversion_call_start)) = go_type_conversion_receiver(operand, source)?
    else {
        return Ok(None);
    };
    if go_type_name_is_shadowed(&type_name, context.bindings) {
        return Ok(None);
    }
    Ok(Some(GoDirectMethodReference::TypeConversion {
        method_path: format!("{type_name}::{method_name}"),
        conversion_call_start,
    }))
}

fn go_type_name_is_shadowed(type_name: &str, bindings: &BTreeSet<String>) -> bool {
    bindings.contains(type_name)
        || type_name
            .split_once('.')
            .is_some_and(|(package_name, _)| bindings.contains(package_name))
}

fn go_type_assertion_receiver(operand: Node<'_>, source: &str) -> Result<Option<String>> {
    if operand.kind() != "type_assertion_expression" {
        return Ok(None);
    }
    operand
        .child_by_field_name("type")
        .map(|type_node| go_named_local_type(type_node, source))
        .transpose()
        .map(Option::flatten)
}

fn go_type_conversion_receiver(operand: Node<'_>, source: &str) -> Result<Option<(String, usize)>> {
    let type_name = match operand.kind() {
        "call_expression" => {
            let Some(function) = operand.child_by_field_name("function") else {
                return Ok(None);
            };
            go_ambiguous_type_conversion_function_name(function, source)?
        }
        "type_conversion_expression" => operand
            .child_by_field_name("type")
            .map(|type_node| go_named_local_type(type_node, source))
            .transpose()?
            .flatten(),
        _ => None,
    };
    Ok(type_name.map(|type_name| (type_name, operand.start_byte())))
}

fn go_ambiguous_type_conversion_function_name(
    node: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    match node.kind() {
        "identifier" => {
            let name = node_text(node, source)?.trim();
            Ok((!name.is_empty()).then(|| name.to_string()))
        }
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .next()
                .map(|inner| go_ambiguous_type_conversion_function_name(inner, source))
                .transpose()
                .map(Option::flatten)
        }
        "selector_expression" => {
            let Some(package) = node.child_by_field_name("operand") else {
                return Ok(None);
            };
            let Some(name) = node.child_by_field_name("field") else {
                return Ok(None);
            };
            if package.kind() != "identifier" || name.kind() != "field_identifier" {
                return Ok(None);
            }
            let package = node_text(package, source)?.trim();
            let name = node_text(name, source)?.trim();
            if package.is_empty() || name.is_empty() {
                return Ok(None);
            }
            Ok(Some(format!("{package}.{name}")))
        }
        "unary_expression" => {
            if !node_text(node, source)?.trim().starts_with('*') {
                return Ok(None);
            }
            node.child_by_field_name("operand")
                .map(|inner| go_ambiguous_type_conversion_function_name(inner, source))
                .transpose()
                .map(Option::flatten)
        }
        _ => Ok(None),
    }
}

fn collect_direct_local_calls_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    context: &GoDirectCallContext<'_>,
    references: &mut GoDirectReferences,
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
                if !name.is_empty()
                    && !context.bindings.contains(name)
                    && !references
                        .suppressed_type_conversion_call_starts
                        .contains(&node.start_byte())
                {
                    references.references_by_name.insert(
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
                    if !references
                        .suppressed_type_conversion_call_starts
                        .contains(&node.start_byte())
                    {
                        references.references_by_name.insert(reference);
                    }
                } else if let Some(reference) =
                    go_direct_method_reference(function, source, context)?
                {
                    match reference {
                        GoDirectMethodReference::Plain(reference) => {
                            references.references_by_name.insert(reference);
                        }
                        GoDirectMethodReference::TypeAssertion(method_path) => {
                            references
                                .type_assertion_method_references
                                .insert(method_path);
                        }
                        GoDirectMethodReference::TypeConversion {
                            method_path,
                            conversion_call_start,
                        } => {
                            references
                                .type_conversion_method_references
                                .insert(method_path);
                            references
                                .suppressed_type_conversion_call_starts
                                .insert(conversion_call_start);
                        }
                    }
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
    use crate::symbol_index_model::{GoReferenceDetails, ReferenceLanguageDetails};

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

    #[test]
    fn indexes_go_qualified_type_method_reference_facts() {
        let source = r#"
package main

import svc "example.com/project/internal/service"

func composite() int { return svc.Counter{}.Value() }
func conversion(value int) int { return svc.Scalar(value).Value() }
func assertion(value any) int { return value.(svc.Counter).Value() }
"#;
        let path = Path::new("main.go");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_go_symbols_with_deadline(path, source, document.tree.root_node(), None).unwrap();

        let composite = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "composite")
            .unwrap();
        assert_eq!(composite.reference_facts.len(), 1);
        assert_eq!(composite.reference_facts[0].spelling, "svc.Counter::Value");
        assert_eq!(
            composite.reference_facts[0].language_details,
            ReferenceLanguageDetails::None
        );

        let conversion = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "conversion")
            .unwrap();
        assert_eq!(conversion.reference_facts.len(), 1);
        assert_eq!(conversion.reference_facts[0].spelling, "svc.Scalar::Value");
        assert_eq!(
            conversion.reference_facts[0].language_details,
            ReferenceLanguageDetails::Go(GoReferenceDetails {
                type_conversion: true,
                type_assertion: false,
            })
        );

        let assertion = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "assertion")
            .unwrap();
        assert_eq!(assertion.reference_facts.len(), 1);
        assert_eq!(assertion.reference_facts[0].spelling, "svc.Counter::Value");
        assert_eq!(
            assertion.reference_facts[0].language_details,
            ReferenceLanguageDetails::Go(GoReferenceDetails {
                type_conversion: false,
                type_assertion: true,
            })
        );
    }

    #[test]
    fn indexes_go_type_conversion_method_reference_facts_without_legacy_call_edges() {
        let source = r#"
package metrics

type Scalar int
type Box[T ~int] int
type Result struct{}

func (Scalar) Value() int { return 1 }
func (Box[T]) Value() int { return 2 }
func (Result) Value() int { return 3 }
func Factory(value int) Result { return Result{} }
func scalar_conversion(value int) int { return Scalar(value).Value() }
func pointer_conversion(value *Scalar) int { return (*Scalar)(value).Value() }
func parenthesized_conversion(value int) int { return (Scalar)(value).Value() }
func generic_conversion(value int) int { return Box[int](value).Value() }
func factory_method(value int) int { return Factory(value).Value() }
func parenthesized_factory_method(value int) int { return (Factory)(value).Value() }
func shadowed_conversion(Scalar func(int) Result, value int) int { return Scalar(value).Value() }
func asserted_method(value any) int { return value.(Scalar).Value() }
func shadowed_assertion(Scalar any, value any) int { return value.(Scalar).Value() }
"#;
        let path = Path::new("metrics.go");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_go_symbols_with_deadline(path, source, document.tree.root_node(), None).unwrap();

        for (caller_path, method_path) in [
            ("scalar_conversion", "Scalar::Value"),
            ("pointer_conversion", "Scalar::Value"),
            ("parenthesized_conversion", "Scalar::Value"),
            ("generic_conversion", "Box::Value"),
            ("factory_method", "Factory::Value"),
            ("parenthesized_factory_method", "Factory::Value"),
        ] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert!(caller.references_by_name.is_empty(), "{caller_path}");
            assert_eq!(caller.reference_facts.len(), 1, "{caller_path}");
            assert_eq!(
                caller.reference_facts[0].spelling, method_path,
                "{caller_path}"
            );
            assert_eq!(
                caller.reference_facts[0].call_arities, None,
                "{caller_path}"
            );
            assert_eq!(
                caller.reference_facts[0].language_details,
                ReferenceLanguageDetails::Go(GoReferenceDetails {
                    type_conversion: true,
                    type_assertion: false,
                }),
                "{caller_path}"
            );
        }

        let shadowed_conversion = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "shadowed_conversion")
            .unwrap();
        assert!(shadowed_conversion.references_by_name.is_empty());
        assert!(shadowed_conversion.reference_facts.is_empty());

        let asserted_method = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "asserted_method")
            .unwrap();
        assert!(asserted_method.references_by_name.is_empty());
        assert_eq!(asserted_method.reference_facts.len(), 1);
        assert_eq!(asserted_method.reference_facts[0].spelling, "Scalar::Value");
        assert_eq!(
            asserted_method.reference_facts[0].language_details,
            ReferenceLanguageDetails::Go(GoReferenceDetails {
                type_conversion: false,
                type_assertion: true,
            })
        );

        let shadowed_assertion = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "shadowed_assertion")
            .unwrap();
        assert!(shadowed_assertion.references_by_name.is_empty());
        assert!(shadowed_assertion.reference_facts.is_empty());
    }
}
