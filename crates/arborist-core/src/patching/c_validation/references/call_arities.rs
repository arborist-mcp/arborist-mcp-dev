use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use tree_sitter::Node;

use super::super::cpp_syntax::{
    cpp_constructor_type_text, cpp_default_initialized_type_text, strip_cpp_outer_parentheses,
};
use super::super::cpp_types::{CppThisMemberReceiver, cpp_temporary_type_path};
use super::member_call_names::{
    encode_cpp_local_member_call_name, encode_cpp_temporary_member_call_name,
    encode_cpp_this_member_call_name,
};
use super::types::CppLocalBinding;
use super::{
    collect_cpp_local_bindings, cpp_local_member_receiver_type, cpp_temporary_type_from_expression,
    cpp_this_receiver_from_expression,
};
use crate::language::{node_text, visit_tree};

pub(crate) fn collect_c_call_arities(
    node: Node<'_>,
    source: &str,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| collect_c_call_arity(candidate, source, call_arities);
    visit_tree(node, &mut callback);
    Ok(())
}

pub(crate) fn collect_cpp_call_arities(
    node: Node<'_>,
    source: &str,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
) -> Result<()> {
    let local_bindings = collect_cpp_local_bindings(node, source);
    let mut callback = |candidate: Node<'_>| match candidate.kind() {
        "call_expression" => {
            collect_cpp_call_arity(candidate, source, call_arities, &local_bindings)
        }
        "compound_literal_expression" => {
            collect_cpp_braced_call_arity(candidate, source, call_arities)
        }
        "init_declarator" => collect_cpp_braced_initializer_arity(candidate, source, call_arities),
        "new_expression" => collect_cpp_new_call_arity(candidate, source, call_arities),
        _ => {}
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn collect_cpp_call_arity(
    candidate: Node<'_>,
    source: &str,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
    local_bindings: &[CppLocalBinding],
) {
    let Some(function) = candidate.child_by_field_name("function") else {
        return;
    };
    let Some(arguments) = candidate.child_by_field_name("arguments") else {
        return;
    };
    let Ok(Some(name)) = direct_cpp_call_name(function, source, local_bindings) else {
        return;
    };

    record_c_call_arity(name, arguments, call_arities);
}

fn collect_c_call_arity(
    candidate: Node<'_>,
    source: &str,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
) {
    if candidate.kind() != "call_expression" {
        return;
    }
    let Some(function) = candidate.child_by_field_name("function") else {
        return;
    };
    let Some(arguments) = candidate.child_by_field_name("arguments") else {
        return;
    };
    let Ok(Some(name)) = direct_c_call_name(function, source) else {
        return;
    };

    record_c_call_arity(name, arguments, call_arities);
}

fn collect_cpp_braced_call_arity(
    candidate: Node<'_>,
    source: &str,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
) {
    let mut cursor = candidate.walk();
    let children = candidate.named_children(&mut cursor).collect::<Vec<_>>();
    let type_node = candidate
        .child_by_field_name("type")
        .or_else(|| children.first().copied());
    let initializer = candidate.child_by_field_name("value").or_else(|| {
        children
            .iter()
            .copied()
            .find(|child| child.kind() == "initializer_list")
    });
    let (Some(type_node), Some(initializer)) = (type_node, initializer) else {
        return;
    };
    let Ok(Some(name)) = direct_c_call_name(type_node, source) else {
        return;
    };

    record_c_call_arity(name, initializer, call_arities);
}

fn collect_cpp_braced_initializer_arity(
    candidate: Node<'_>,
    source: &str,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
) {
    let Some(declaration) = candidate
        .parent()
        .filter(|parent| parent.kind() == "declaration")
    else {
        return;
    };
    let Some(declarator) = candidate.child_by_field_name("declarator") else {
        return;
    };
    if declarator.kind() != "identifier" {
        return;
    }
    let Some(initializer) = candidate
        .child_by_field_name("value")
        .filter(|value| value.kind() == "initializer_list")
    else {
        return;
    };
    let Some(type_node) = declaration.child_by_field_name("type") else {
        return;
    };
    let Ok(Some(name)) = direct_c_call_name(type_node, source) else {
        return;
    };

    record_c_call_arity(name, initializer, call_arities);
}

fn collect_cpp_new_call_arity(
    candidate: Node<'_>,
    source: &str,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
) {
    let Some(name) =
        cpp_new_allocation_type_text(candidate, source).and_then(cpp_temporary_type_path)
    else {
        return;
    };

    let arity = candidate
        .child_by_field_name("arguments")
        .map(named_child_count)
        .unwrap_or_default();
    record_c_call_arity_with_count(name, arity, call_arities);
}

fn cpp_new_allocation_type_text<'a>(candidate: Node<'_>, source: &'a str) -> Option<&'a str> {
    let allocation = node_text(candidate, source).ok()?.trim();
    let allocation = allocation.strip_prefix("new")?.trim_start();
    cpp_constructor_type_text(allocation).or_else(|| cpp_default_initialized_type_text(allocation))
}

fn record_c_call_arity(
    name: String,
    arguments: Node<'_>,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
) {
    record_c_call_arity_with_count(name, named_child_count(arguments), call_arities);
}

fn record_c_call_arity_with_count(
    name: String,
    arity: usize,
    call_arities: &mut BTreeMap<String, BTreeSet<usize>>,
) {
    call_arities
        .entry(name.trim().to_string())
        .or_default()
        .insert(arity);
}

fn named_child_count(node: Node<'_>) -> usize {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).count()
}

fn direct_c_call_name(function: Node<'_>, source: &str) -> Result<Option<String>> {
    match function.kind() {
        "identifier" | "type_identifier" | "template_type" => {
            Ok(Some(node_text(function, source)?.trim().to_string()))
        }
        "qualified_identifier" => qualified_c_call_name(function, source),
        "template_function" => template_call_name(function, source),
        _ => Ok(None),
    }
}

fn direct_cpp_call_name(
    function: Node<'_>,
    source: &str,
    local_bindings: &[CppLocalBinding],
) -> Result<Option<String>> {
    if let Some(name) = direct_c_call_name(function, source)? {
        return Ok(Some(name));
    }
    if function.kind() != "field_expression" {
        return Ok(None);
    }

    let Some(argument) = function.child_by_field_name("argument") else {
        return Ok(None);
    };
    let Some(field) = function.child_by_field_name("field") else {
        return Ok(None);
    };
    let Some(member_operator) = cpp_field_expression_operator(function, argument, field, source)?
    else {
        return Ok(None);
    };
    let Some(name) = cpp_member_call_name(field, source)? else {
        return Ok(None);
    };
    if let Some(receiver) = cpp_this_member_receiver(argument, source)? {
        return Ok(Some(encode_cpp_this_member_call_name(name, receiver)));
    }
    if let Some((type_name, receiver)) =
        cpp_local_member_receiver_type(argument, source, local_bindings, member_operator)?
    {
        return Ok(Some(encode_cpp_local_member_call_name(
            type_name, name, receiver,
        )));
    }
    if let Some((type_name, receiver)) = cpp_temporary_member_receiver_type(argument, source)? {
        return Ok(Some(encode_cpp_temporary_member_call_name(
            type_name, name, receiver,
        )));
    }
    Ok(None)
}

fn cpp_field_expression_operator<'a>(
    function: Node<'_>,
    argument: Node<'_>,
    field: Node<'_>,
    source: &'a str,
) -> Result<Option<&'a str>> {
    if let Some(operator) = function.child_by_field_name("operator") {
        return node_text(operator, source).map(|operator| Some(operator.trim()));
    }
    let operator = source[argument.end_byte()..field.start_byte()].trim();
    Ok(matches!(operator, "." | "->").then_some(operator))
}

fn cpp_member_call_name(field: Node<'_>, source: &str) -> Result<Option<String>> {
    if field.kind() == "template_method" {
        return template_call_name(field, source);
    }
    if field.kind() == "dependent_name" {
        let mut cursor = field.walk();
        if let Some(template_method) = field
            .named_children(&mut cursor)
            .find(|child| child.kind() == "template_method")
        {
            return template_call_name(template_method, source);
        }
    }
    node_text(field, source).map(|field| Some(field.trim().to_string()))
}

fn cpp_this_member_receiver(
    argument: Node<'_>,
    source: &str,
) -> Result<Option<CppThisMemberReceiver>> {
    let receiver_text = node_text(argument, source)?;
    Ok(cpp_this_receiver_from_expression(receiver_text))
}

fn cpp_temporary_member_receiver_type(
    argument: Node<'_>,
    source: &str,
) -> Result<Option<(String, CppThisMemberReceiver)>> {
    let receiver = strip_cpp_outer_parentheses(node_text(argument, source)?.trim());
    Ok(cpp_temporary_type_from_expression(receiver))
}

fn qualified_c_call_name(function: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut cursor = function.walk();
    let template_function = function
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "template_function")
        .last();
    let Some(template_function) = template_function else {
        return Ok(Some(node_text(function, source)?.trim().to_string()));
    };
    let Some(name) = template_call_name(template_function, source)? else {
        return Ok(None);
    };

    let prefix = source[function.start_byte()..template_function.start_byte()].trim_end();
    let prefix = prefix.strip_suffix("template").unwrap_or(prefix).trim_end();
    Ok(Some(format!("{prefix}{name}")))
}

fn template_call_name(function: Node<'_>, source: &str) -> Result<Option<String>> {
    node_text(function, source).map(|name| {
        Some(
            name.chars()
                .filter(|character| !character.is_whitespace())
                .collect(),
        )
    })
}
