use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use tree_sitter::Node;

use super::cpp_types::{
    CppThisMemberReceiver, cpp_pointer_target_path, cpp_temporary_type_path,
    cpp_this_receiver_for_type,
};
use crate::language::{node_text, visit_tree};
use crate::symbol_index_model::{
    CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX, CPP_CONST_LVALUE_THIS_CALL_PREFIX,
    CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_CONST_RVALUE_THIS_CALL_PREFIX, CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_RVALUE_THIS_CALL_PREFIX, CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_TEMPORARY_MEMBER_CALL_SEPARATOR,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum CppMemberAccess {
    Object,
    Pointer,
}

#[derive(Clone)]
struct CppLocalBinding {
    name: String,
    type_name: String,
    receiver: CppThisMemberReceiver,
    access: CppMemberAccess,
    standard_smart_pointer: bool,
    declaration_start: usize,
    scope_range: (usize, usize),
}

pub(super) fn collect_c_local_definitions(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    collect_c_local_definitions_in_node(node, source, names)?;
    collect_cpp_template_parameter_definitions(node, source, names)
}

fn collect_c_local_definitions_in_node(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if let Some(parent) = candidate.parent()
            && candidate.kind() == "identifier"
            && matches!(
                parent.kind(),
                "declaration"
                    | "init_declarator"
                    | "parameter_declaration"
                    | "optional_parameter_declaration"
                    | "variadic_parameter_declaration"
                    | "variadic_declarator"
                    | "function_declarator"
                    | "pointer_declarator"
                    | "array_declarator"
            )
        {
            let _ = node_text(candidate, source).map(|text| names.insert(text.trim().to_string()));
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn collect_cpp_template_parameter_definitions(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "template_declaration" {
            let mut cursor = candidate.walk();
            for child in candidate.named_children(&mut cursor) {
                if child.kind() == "template_parameter_list" {
                    collect_c_local_definitions_in_node(child, source, names)?;
                }
            }
        }
        current = candidate.parent();
    }
    Ok(())
}

pub(crate) fn collect_c_references(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    collect_c_references_with_options(node, source, references, false)
}

pub(crate) fn collect_c_graph_references(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    collect_c_references_with_options(node, source, references, true)
}

fn collect_c_references_with_options(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
    suppress_direct_qualified_call_components: bool,
) -> Result<()> {
    let mut template_parameters = BTreeSet::new();
    collect_cpp_template_parameter_definitions(node, source, &mut template_parameters)?;
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() == "identifier"
            && !is_c_enumerator_name(candidate)
            && !is_cpp_new_type_qualifier_recovery_identifier(candidate, source)
            && (!suppress_direct_qualified_call_components
                || !is_direct_qualified_call_component(candidate))
        {
            let _ = node_text(candidate, source).map(|text| {
                let name = text.trim().to_string();
                if !template_parameters.contains(&name)
                    || is_qualified_identifier_component(candidate)
                {
                    references.insert(name);
                }
            });
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

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

fn encode_cpp_this_member_call_name(name: String, receiver: CppThisMemberReceiver) -> String {
    match receiver {
        CppThisMemberReceiver::Lvalue => name,
        CppThisMemberReceiver::ConstLvalue => {
            format!("{CPP_CONST_LVALUE_THIS_CALL_PREFIX}{name}")
        }
        CppThisMemberReceiver::Rvalue => format!("{CPP_RVALUE_THIS_CALL_PREFIX}{name}"),
        CppThisMemberReceiver::ConstRvalue => {
            format!("{CPP_CONST_RVALUE_THIS_CALL_PREFIX}{name}")
        }
    }
}

fn encode_cpp_temporary_member_call_name(
    type_name: String,
    name: String,
    receiver: CppThisMemberReceiver,
) -> String {
    let prefix = match receiver {
        CppThisMemberReceiver::Lvalue => return name,
        CppThisMemberReceiver::ConstLvalue => CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
        CppThisMemberReceiver::Rvalue => CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
        CppThisMemberReceiver::ConstRvalue => CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    };
    format!("{prefix}{type_name}{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}{type_name}::{name}")
}

fn encode_cpp_local_member_call_name(
    type_name: String,
    name: String,
    receiver: CppThisMemberReceiver,
) -> String {
    let prefix = match receiver {
        CppThisMemberReceiver::Lvalue => CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CppThisMemberReceiver::ConstLvalue => CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CppThisMemberReceiver::Rvalue => CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CppThisMemberReceiver::ConstRvalue => CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    };
    format!("{prefix}{type_name}{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}{type_name}::{name}")
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

fn collect_cpp_local_bindings(node: Node<'_>, source: &str) -> Vec<CppLocalBinding> {
    let mut bindings = Vec::new();
    let mut callback = |candidate: Node<'_>| match candidate.kind() {
        "declaration" => {
            if let Some(binding) = cpp_local_binding(candidate, source) {
                bindings.push(binding);
            }
        }
        "parameter_declaration" | "optional_parameter_declaration" => {
            if let Some(binding) = cpp_parameter_binding(candidate, source) {
                bindings.push(binding);
            }
        }
        "for_range_loop" => {
            if let Some(binding) = cpp_range_for_binding(candidate, source) {
                bindings.push(binding);
            }
        }
        _ => {}
    };
    visit_tree(node, &mut callback);
    bindings
}

fn cpp_local_binding(declaration: Node<'_>, source: &str) -> Option<CppLocalBinding> {
    let scope = cpp_local_binding_scope(declaration)?;
    cpp_object_binding(declaration, source, scope)
}

fn cpp_parameter_binding(parameter: Node<'_>, source: &str) -> Option<CppLocalBinding> {
    let scope = cpp_parameter_binding_scope(parameter)?;
    cpp_object_binding(parameter, source, scope)
}

fn cpp_range_for_binding(loop_node: Node<'_>, source: &str) -> Option<CppLocalBinding> {
    let type_node = loop_node.child_by_field_name("type")?;
    let declarator = loop_node.child_by_field_name("declarator")?;
    let identifier = cpp_declarator_identifier(declarator)?;
    let type_prefix = cpp_range_for_type_prefix(loop_node, source)?;
    let type_suffix = source[type_node.end_byte()..identifier.start_byte()].trim();
    let (type_name, receiver, access, standard_smart_pointer) =
        cpp_binding_type(type_node, &type_prefix, type_suffix, source)?;

    Some(CppLocalBinding {
        name: node_text(identifier, source).ok()?.trim().to_string(),
        type_name,
        receiver,
        access,
        standard_smart_pointer,
        declaration_start: loop_node.start_byte(),
        scope_range: (loop_node.start_byte(), loop_node.end_byte()),
    })
}

fn cpp_range_for_type_prefix(loop_node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = loop_node.walk();
    let qualifiers = loop_node
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "type_qualifier")
        .map(|child| node_text(child, source).ok().map(str::trim))
        .collect::<Option<Vec<_>>>()?;
    qualifiers
        .iter()
        .all(|qualifier| matches!(*qualifier, "const" | "volatile"))
        .then(|| qualifiers.join(" "))
}

fn cpp_object_binding(
    declaration: Node<'_>,
    source: &str,
    scope: Node<'_>,
) -> Option<CppLocalBinding> {
    let type_node = declaration.child_by_field_name("type")?;
    let declarator = cpp_single_declarator(declaration)?;
    let identifier = cpp_declarator_identifier(declarator)?;

    let type_prefix = source[declaration.start_byte()..type_node.start_byte()].trim();
    let type_suffix = source[type_node.end_byte()..identifier.start_byte()].trim();
    let (type_name, receiver, access, standard_smart_pointer) =
        if node_text(type_node, source).ok()?.trim() == "auto" {
            cpp_auto_constructor_binding_type(declarator, type_prefix, source)?
        } else {
            cpp_binding_type(type_node, type_prefix, type_suffix, source)?
        };

    Some(CppLocalBinding {
        name: node_text(identifier, source).ok()?.trim().to_string(),
        type_name,
        receiver,
        access,
        standard_smart_pointer,
        declaration_start: declaration.start_byte(),
        scope_range: (scope.start_byte(), scope.end_byte()),
    })
}

fn cpp_auto_constructor_binding_type(
    declarator: Node<'_>,
    type_prefix: &str,
    source: &str,
) -> Option<(String, CppThisMemberReceiver, CppMemberAccess, bool)> {
    if !cpp_binding_type_prefix_is_supported(type_prefix) {
        return None;
    }
    let initializer = declarator.child_by_field_name("value")?;
    let initializer_text = cpp_auto_constructor_initializer_text(initializer, source)?;
    let type_suffix =
        source[declarator.start_byte()..cpp_declarator_identifier(declarator)?.start_byte()].trim();
    let compact_type_suffix = compact_cpp_expression(type_suffix);
    let declared_access = if cpp_pointer_declarator_suffix(&compact_type_suffix) {
        CppMemberAccess::Pointer
    } else if cpp_object_declarator_suffix(&compact_type_suffix) {
        CppMemberAccess::Object
    } else {
        return None;
    };
    let allocation_initializer = initializer_text.strip_prefix("new").and_then(|remainder| {
        let allocation = remainder.trim_start();
        (allocation.len() < remainder.len()).then_some(allocation)
    });
    let smart_pointer_factory_type = cpp_smart_pointer_factory_type(initializer_text);
    let inferred_pointer_type = allocation_initializer
        .and_then(|allocation| {
            cpp_constructor_type_text(allocation)
                .or_else(|| cpp_default_initialized_type_text(allocation))
        })
        .or(smart_pointer_factory_type);
    let access = match (declared_access, inferred_pointer_type) {
        (CppMemberAccess::Pointer, _) | (CppMemberAccess::Object, Some(_)) => {
            CppMemberAccess::Pointer
        }
        (CppMemberAccess::Object, None) => CppMemberAccess::Object,
    };
    let initializer_type = if access == CppMemberAccess::Pointer {
        inferred_pointer_type
    } else {
        cpp_constructor_type_text(initializer_text)
            .or_else(|| cpp_default_initialized_type_text(initializer_text))
    };
    let type_name = initializer_type
        .and_then(cpp_temporary_type_path)
        .or_else(|| {
            cpp_temporary_type_from_expression(initializer_text).map(|(type_name, _)| type_name)
        })
        .or_else(|| cpp_default_initialized_type_path(initializer_text))?;
    let type_qualifiers =
        if access == CppMemberAccess::Pointer && declared_access == CppMemberAccess::Object {
            String::new()
        } else {
            cpp_binding_type_qualifier_prefix(type_prefix)
        };
    let receiver_type = initializer_type.unwrap_or(&type_name);
    let receiver =
        cpp_this_receiver_for_type(&format!("{type_qualifiers} {receiver_type}"), Some(false))?;

    Some((
        type_name,
        receiver,
        access,
        smart_pointer_factory_type.is_some(),
    ))
}

fn cpp_auto_constructor_initializer_text<'a>(
    initializer: Node<'_>,
    source: &'a str,
) -> Option<&'a str> {
    if initializer.kind() != "initializer_list" {
        return node_text(initializer, source).ok().map(str::trim);
    }
    let mut cursor = initializer.walk();
    let mut values = initializer.named_children(&mut cursor);
    let value = values.next()?;
    (values.next().is_none()).then(|| node_text(value, source).ok().map(str::trim))?
}

fn cpp_smart_pointer_factory_type(expression: &str) -> Option<&str> {
    ["std::make_unique", "std::make_shared"]
        .into_iter()
        .find_map(|factory| {
            cpp_typed_receiver_call(expression, factory).map(|(type_name, _)| type_name)
        })
}

fn cpp_binding_type(
    type_node: Node<'_>,
    type_prefix: &str,
    type_suffix: &str,
    source: &str,
) -> Option<(String, CppThisMemberReceiver, CppMemberAccess, bool)> {
    if !cpp_binding_type_prefix_is_supported(type_prefix) {
        return None;
    }
    let compact_type_suffix = compact_cpp_expression(type_suffix);
    let declared_access = if cpp_pointer_declarator_suffix(&compact_type_suffix) {
        CppMemberAccess::Pointer
    } else if cpp_object_declarator_suffix(&compact_type_suffix) {
        CppMemberAccess::Object
    } else {
        return None;
    };
    let declared_type = node_text(type_node, source).ok()?.trim();
    let smart_pointer_target = cpp_standard_smart_pointer_target_type(declared_type);
    let access = if smart_pointer_target.is_some() {
        CppMemberAccess::Pointer
    } else {
        declared_access
    };
    let type_qualifiers = cpp_binding_type_qualifier_prefix(type_prefix);
    let type_name = format!("{type_qualifiers} {} {type_suffix}", declared_type);
    let receiver = match (access, smart_pointer_target) {
        (CppMemberAccess::Object, _) => cpp_named_binding_receiver_for_type(&type_name)?,
        (CppMemberAccess::Pointer, Some(target)) => {
            cpp_this_receiver_for_type(target, Some(false))?
        }
        (CppMemberAccess::Pointer, None) => cpp_pointer_binding_receiver_for_type(&type_name)?,
    };
    let type_name = match (access, smart_pointer_target) {
        (CppMemberAccess::Object, _) => cpp_temporary_type_path(&type_name)?,
        (CppMemberAccess::Pointer, Some(target)) => cpp_temporary_type_path(target)?,
        (CppMemberAccess::Pointer, None) => cpp_pointer_target_path(&type_name)?,
    };

    Some((type_name, receiver, access, smart_pointer_target.is_some()))
}

fn cpp_standard_smart_pointer_target_type(type_name: &str) -> Option<&str> {
    ["std::unique_ptr", "std::shared_ptr"]
        .into_iter()
        .find_map(|pointer_type| {
            let contents = type_name
                .trim()
                .strip_prefix(pointer_type)?
                .strip_prefix('<')?;
            let target_end = matching_angle_bracket_index(contents)?;
            cpp_first_template_argument(&contents[..target_end])
        })
}

fn cpp_first_template_argument(arguments: &str) -> Option<&str> {
    let mut depth = 0usize;
    for (index, character) in arguments.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                return Some(arguments[..index].trim()).filter(|value| !value.is_empty());
            }
            _ => {}
        }
    }
    Some(arguments.trim()).filter(|value| !value.is_empty())
}

fn cpp_binding_type_prefix_is_supported(type_prefix: &str) -> bool {
    type_prefix.split_whitespace().all(|part| {
        matches!(
            part,
            "const"
                | "volatile"
                | "auto"
                | "register"
                | "static"
                | "thread_local"
                | "extern"
                | "mutable"
        )
    })
}

fn cpp_binding_type_qualifier_prefix(type_prefix: &str) -> String {
    type_prefix
        .split_whitespace()
        .filter(|part| matches!(*part, "const" | "volatile"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn cpp_pointer_declarator_suffix(type_suffix: &str) -> bool {
    let type_suffix = cpp_strip_cv_qualifiers(type_suffix);
    let Some(type_suffix) = type_suffix.strip_prefix('*') else {
        return false;
    };
    let qualifiers = cpp_strip_cv_qualifiers(type_suffix);
    let reference_count = qualifiers.chars().count();
    matches!(reference_count, 0..=2) && qualifiers.chars().all(|character| character == '&')
}

fn cpp_object_declarator_suffix(type_suffix: &str) -> bool {
    let type_suffix = cpp_strip_cv_qualifiers(type_suffix);
    matches!(type_suffix, "" | "&" | "&&")
}

fn cpp_strip_cv_qualifiers(mut type_suffix: &str) -> &str {
    loop {
        if let Some(remaining) = type_suffix.strip_prefix("const") {
            type_suffix = remaining;
        } else if let Some(remaining) = type_suffix.strip_prefix("volatile") {
            type_suffix = remaining;
        } else {
            return type_suffix;
        }
    }
}

fn cpp_named_binding_receiver_for_type(type_name: &str) -> Option<CppThisMemberReceiver> {
    let type_name = type_name.trim_end().trim_end_matches('&').trim_end();
    cpp_this_receiver_for_type(type_name, Some(false))
}

fn cpp_pointer_binding_receiver_for_type(type_name: &str) -> Option<CppThisMemberReceiver> {
    let pointee_type = type_name.split_once('*')?.0.trim();
    cpp_this_receiver_for_type(pointee_type, Some(false))
}

fn cpp_single_declarator(declaration: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = declaration.walk();
    let mut declarators = declaration.children_by_field_name("declarator", &mut cursor);
    let declarator = declarators.next()?;
    (declarators.next().is_none()).then_some(declarator)
}

fn cpp_declarator_identifier(declarator: Node<'_>) -> Option<Node<'_>> {
    if declarator.kind() == "identifier" {
        return Some(declarator);
    }
    if let Some(nested_declarator) = declarator.child_by_field_name("declarator") {
        return cpp_declarator_identifier(nested_declarator);
    }
    let mut cursor = declarator.walk();
    let mut identifiers = declarator
        .named_children(&mut cursor)
        .filter_map(cpp_declarator_identifier);
    let identifier = identifiers.next()?;
    (identifiers.next().is_none()).then_some(identifier)
}

fn cpp_local_binding_scope(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "compound_statement"
                | "for_statement"
                | "for_range_loop"
                | "if_statement"
                | "switch_statement"
                | "while_statement"
        ) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn cpp_parameter_binding_scope(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "lambda_expression" {
            return Some(candidate);
        }
        if candidate.kind() == "catch_clause" {
            return Some(candidate);
        }
        if candidate.kind() == "function_definition" {
            return candidate.child_by_field_name("body");
        }
        if matches!(candidate.kind(), "declaration" | "field_declaration") {
            return None;
        }
        current = candidate.parent();
    }
    None
}

fn cpp_local_member_receiver_type(
    argument: Node<'_>,
    source: &str,
    local_bindings: &[CppLocalBinding],
    member_operator: &str,
) -> Result<Option<(String, CppThisMemberReceiver)>> {
    let receiver = strip_cpp_outer_parentheses(node_text(argument, source)?.trim());
    Ok(cpp_local_member_receiver_from_expression(
        receiver,
        argument.start_byte(),
        local_bindings,
        member_operator,
    ))
}

fn cpp_local_member_receiver_from_expression(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
    member_operator: &str,
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if member_operator == "->"
        && let Some(binding_name) = cpp_standard_smart_pointer_get_receiver(expression)
        && let Some(binding) = cpp_visible_local_binding(binding_name, byte_offset, local_bindings)
        && binding.standard_smart_pointer
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if member_operator == "->"
        && let Some(binding_name) = cpp_local_binding_name_from_expression(expression)
        && let Some(binding) = cpp_visible_local_binding(binding_name, byte_offset, local_bindings)
        && binding.access == CppMemberAccess::Pointer
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings) {
        let expected_operator = match binding.access {
            CppMemberAccess::Object => ".",
            CppMemberAccess::Pointer => "->",
        };
        if member_operator != expected_operator {
            return None;
        }
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if member_operator == "."
        && let Some(pointer_name) = expression.strip_prefix('*').map(str::trim)
        && let Some(binding) = cpp_visible_local_binding(pointer_name, byte_offset, local_bindings)
        && binding.access == CppMemberAccess::Pointer
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if member_operator != "." {
        return None;
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_local_member_receiver_from_expression(
            argument,
            byte_offset,
            local_bindings,
            member_operator,
        )
        .map(|(type_name, receiver)| {
            let receiver = match receiver {
                CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                    CppThisMemberReceiver::Rvalue
                }
                CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                    CppThisMemberReceiver::ConstRvalue
                }
            };
            (type_name, receiver)
        });
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_local_member_receiver_from_expression(
            argument,
            byte_offset,
            local_bindings,
            member_operator,
        )
        .map(|(type_name, _)| (type_name, CppThisMemberReceiver::ConstLvalue));
    }
    for function_name in ["static_cast", "std::forward"] {
        if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, function_name) {
            cpp_local_member_receiver_from_expression(
                argument,
                byte_offset,
                local_bindings,
                member_operator,
            )?;
            return Some((
                cpp_temporary_type_path(type_name)?,
                cpp_this_receiver_for_type(type_name, Some(true))?,
            ));
        }
    }
    None
}

fn cpp_standard_smart_pointer_get_receiver(expression: &str) -> Option<&str> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    cpp_local_binding_name_from_expression(receiver)
}

fn cpp_local_binding_name_from_expression(expression: &str) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if is_cpp_identifier(expression) {
        return Some(expression);
    }
    ["std::move", "std::as_const"]
        .into_iter()
        .find_map(|wrapper| {
            cpp_receiver_call_argument(expression, wrapper)
                .and_then(cpp_local_binding_name_from_expression)
        })
        .or_else(|| {
            cpp_typed_receiver_call(expression, "std::forward")
                .and_then(|(_, argument)| cpp_local_binding_name_from_expression(argument))
        })
}

fn cpp_visible_local_binding<'a>(
    name: &str,
    byte_offset: usize,
    local_bindings: &'a [CppLocalBinding],
) -> Option<&'a CppLocalBinding> {
    if !is_cpp_identifier(name) {
        return None;
    }
    local_bindings
        .iter()
        .filter(|binding| {
            binding.name == name
                && binding.declaration_start < byte_offset
                && (binding.scope_range.0..binding.scope_range.1).contains(&byte_offset)
        })
        .min_by_key(|binding| {
            (
                binding.scope_range.1.saturating_sub(binding.scope_range.0),
                usize::MAX.saturating_sub(binding.declaration_start),
            )
        })
}

fn is_cpp_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some(character) if character.is_ascii_alphabetic() || character == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn cpp_temporary_type_from_expression(expression: &str) -> Option<(String, CppThisMemberReceiver)> {
    let expression = expression.trim();
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_temporary_type_from_expression(strip_cpp_outer_parentheses(argument)).map(
            |(type_name, receiver)| {
                let receiver = match receiver {
                    CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                        CppThisMemberReceiver::Rvalue
                    }
                    CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                        CppThisMemberReceiver::ConstRvalue
                    }
                };
                (type_name, receiver)
            },
        );
    }
    for function_name in ["static_cast", "std::forward"] {
        if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, function_name) {
            cpp_temporary_type_from_expression(strip_cpp_outer_parentheses(argument))?;
            return Some((
                cpp_temporary_type_path(type_name)?,
                cpp_this_receiver_for_type(type_name, Some(true))?,
            ));
        }
    }
    cpp_constructor_type_text(expression).map(|type_name| {
        (
            compact_cpp_expression(type_name),
            CppThisMemberReceiver::Rvalue,
        )
    })
}

fn cpp_constructor_type_text(expression: &str) -> Option<&str> {
    let expression = expression.trim();
    let closing = expression.chars().last()?;
    let opening = match closing {
        ')' => matching_opening_delimiter_index(expression, '(', ')')?,
        '}' => matching_opening_delimiter_index(expression, '{', '}')?,
        _ => return None,
    };
    cpp_type_text(expression[..opening].trim())
}

fn cpp_default_initialized_type_path(type_name: &str) -> Option<String> {
    cpp_default_initialized_type_text(type_name).and_then(cpp_temporary_type_path)
}

fn cpp_default_initialized_type_text(type_name: &str) -> Option<&str> {
    cpp_type_text(type_name.trim())
}

fn cpp_type_text(type_name: &str) -> Option<&str> {
    (!type_name.is_empty()
        && type_name.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '_' | ':' | '<' | '>' | ',' | ' ' | '\t')
        }))
    .then_some(type_name)
}

fn is_cpp_new_type_qualifier_recovery_identifier(candidate: Node<'_>, source: &str) -> bool {
    let Some(error) = candidate.parent().filter(|parent| parent.is_error()) else {
        return false;
    };
    if error
        .parent()
        .is_none_or(|parent| parent.kind() != "new_expression")
    {
        return false;
    }
    let qualifier_prefix = source[error.start_byte()..candidate.start_byte()].trim();
    !qualifier_prefix.is_empty()
        && qualifier_prefix
            .split_whitespace()
            .all(|part| matches!(part, "const" | "volatile"))
}

fn matching_opening_delimiter_index(
    expression: &str,
    opening: char,
    closing: char,
) -> Option<usize> {
    let mut depth = 0usize;
    for (index, character) in expression.char_indices().rev() {
        match character {
            character if character == closing => depth += 1,
            character if character == opening => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn cpp_this_receiver_from_expression(receiver: &str) -> Option<CppThisMemberReceiver> {
    let receiver = strip_cpp_outer_parentheses(receiver.trim());
    match compact_cpp_expression(receiver).as_str() {
        "this" | "*this" => return Some(CppThisMemberReceiver::Lvalue),
        _ => {}
    }

    if let Some(argument) = cpp_receiver_call_argument(receiver, "std::move") {
        return cpp_this_receiver_from_expression(argument).map(|receiver| match receiver {
            CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                CppThisMemberReceiver::Rvalue
            }
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                CppThisMemberReceiver::ConstRvalue
            }
        });
    }
    if let Some(argument) = cpp_receiver_call_argument(receiver, "std::as_const") {
        return cpp_this_receiver_from_expression(argument)
            .map(|_| CppThisMemberReceiver::ConstLvalue);
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(receiver, "static_cast") {
        cpp_this_receiver_from_expression(argument)?;
        return cpp_this_receiver_for_type(type_name, None);
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(receiver, "std::forward") {
        cpp_this_receiver_from_expression(argument)?;
        return cpp_this_receiver_for_type(type_name, Some(true));
    }
    None
}

fn cpp_receiver_call_argument<'a>(receiver: &'a str, function_name: &str) -> Option<&'a str> {
    let argument = receiver
        .strip_prefix(function_name)?
        .trim_start()
        .strip_prefix('(')?;
    let argument = argument.trim_end().strip_suffix(')')?.trim();
    if parentheses_are_balanced(argument) {
        Some(argument)
    } else {
        None
    }
}

fn cpp_typed_receiver_call<'a>(
    receiver: &'a str,
    function_name: &str,
) -> Option<(&'a str, &'a str)> {
    let contents = receiver
        .strip_prefix(function_name)?
        .trim_start()
        .strip_prefix('<')?;
    let type_end = matching_angle_bracket_index(contents)?;
    let type_name = contents[..type_end].trim();
    let argument = contents[type_end + 1..]
        .trim_start()
        .strip_prefix('(')?
        .trim_end()
        .strip_suffix(')')?;
    let argument = argument.trim();
    if type_name.is_empty() || !parentheses_are_balanced(argument) {
        return None;
    }
    Some((type_name, argument))
}

fn strip_cpp_outer_parentheses(mut expression: &str) -> &str {
    while let Some(inner) = expression
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .filter(|_| parentheses_wrap_entire_expression(expression))
    {
        expression = inner;
    }
    expression
}

fn compact_cpp_expression(expression: &str) -> String {
    expression
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn parentheses_wrap_entire_expression(expression: &str) -> bool {
    let mut depth = 0usize;
    for (index, character) in expression.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                let Some(next_depth) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next_depth;
                if depth == 0 && index + character.len_utf8() != expression.len() {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn parentheses_are_balanced(expression: &str) -> bool {
    let mut depth = 0usize;
    for character in expression.chars() {
        match character {
            '(' => depth += 1,
            ')' => {
                let Some(next_depth) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next_depth;
            }
            _ => {}
        }
    }
    depth == 0
}

fn matching_angle_bracket_index(contents: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (index, character) in contents.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
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

fn is_qualified_identifier_component(node: Node<'_>) -> bool {
    node.parent()
        .is_some_and(|parent| parent.kind() == "qualified_identifier")
}

fn is_direct_qualified_call_component(node: Node<'_>) -> bool {
    let Some(qualified_identifier) = node.parent() else {
        return false;
    };
    is_direct_qualified_call(qualified_identifier)
}

fn is_direct_qualified_call(qualified_identifier: Node<'_>) -> bool {
    if qualified_identifier.kind() != "qualified_identifier" {
        return false;
    }
    qualified_identifier.parent().is_some_and(|parent| {
        parent.kind() == "call_expression"
            && parent
                .child_by_field_name("function")
                .is_some_and(|function| function == qualified_identifier)
    })
}

fn is_c_enumerator_name(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        parent.kind() == "enumerator"
            && parent
                .child_by_field_name("name")
                .is_some_and(|name| name == node)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    use crate::language::parse_document;

    use super::super::cpp_types::cpp_type_is_top_level_const;
    use super::{
        CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX, CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CPP_TEMPORARY_MEMBER_CALL_SEPARATOR, collect_cpp_call_arities,
        cpp_standard_smart_pointer_target_type, cpp_this_receiver_from_expression,
    };

    #[test]
    fn collects_only_object_braced_initializers() {
        let source = "namespace api { class Counter { public: Counter(int value) {} }; }\nint caller(api::Counter* existing, api::Counter& current) { api::Counter counter{1}; api::Counter* pointer{existing}; api::Counter& reference{current}; return 0; }\n";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([("api::Counter".to_string(), BTreeSet::from([1]))])
        );
    }

    #[test]
    fn collects_this_and_typed_pointer_member_call_arities() {
        let source = "class Counter { int adjust(int value) { return value; } int caller(Counter* other) { return this->adjust(1) + (*this).adjust(1, 2) + other->adjust(1, 2, 3) + (*other).adjust(1, 2, 3, 4); } };";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([
                ("adjust".to_string(), BTreeSet::from([1, 2])),
                (
                    format!(
                        "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
                    ),
                    BTreeSet::from([3, 4]),
                ),
            ])
        );
    }

    #[test]
    fn ignores_parameters_of_local_function_prototypes() {
        let source = "class Counter { public: int adjust(int value) & { return value; } }; int caller(int value) { int declared(Counter current); return current.adjust(value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert!(!arities.keys().any(|name| {
            name.contains("Counter::adjust")
                && name.starts_with(CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX)
        }));
    }

    #[test]
    fn collects_catch_parameter_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } }; int caller(int value) { try { throw value; } catch (Counter current) { return current.adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn collects_this_member_template_call_arities() {
        let source = "class Counter { template <typename T> T adjust(T value) { return value; } int caller(int value) { return this->template adjust<int>(value); } };";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([("adjust<int>".to_string(), BTreeSet::from([1]))])
        );
    }

    #[test]
    fn collects_temporary_member_call_arities() {
        let source = "namespace api { class Counter { public: int adjust(int value) && { return value; } }; int caller(int value) { return Counter{}.adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([
                ("Counter".to_string(), BTreeSet::from([0])),
                (
                    format!(
                        "{CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
                    ),
                    BTreeSet::from([1]),
                ),
            ])
        );
    }

    #[test]
    fn collects_local_variable_member_call_arities() {
        let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } int adjust(int value) && { return value; } }; int caller(int value) { Counter current{}; const Counter locked{}; return current.adjust(value) + locked.adjust(value) + std::move(current).adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn collects_wrapped_pointer_member_call_arities() {
        let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(Counter* current, int value) { return std::as_const(current)->adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert!(!arities.contains_key(&format!(
            "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
        )));
    }

    #[test]
    fn scopes_range_for_bindings_to_the_loop() {
        let source = "class Counter { public: int adjust(int value) & { return value; } }; int caller() { for (Counter current : values) { current.adjust(1); } return current.adjust(1, 2); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn identifies_only_top_level_cpp_const_qualifiers() {
        assert!(cpp_type_is_top_level_const("const Counter&&"));
        assert!(cpp_type_is_top_level_const("Counter const &"));
        assert!(!cpp_type_is_top_level_const("constCounter&&"));
        assert!(!cpp_type_is_top_level_const("Wrapper<const Counter>&&"));
    }

    #[test]
    fn extracts_first_standard_smart_pointer_template_argument() {
        assert_eq!(
            cpp_standard_smart_pointer_target_type("std::unique_ptr<Wrapper<Alias, Tag>, Deleter>"),
            Some("Wrapper<Alias, Tag>")
        );
        assert_eq!(
            cpp_standard_smart_pointer_target_type("std::shared_ptr<const Counter>"),
            Some("const Counter")
        );
        assert!(cpp_standard_smart_pointer_target_type("std::unique_ptr<>").is_none());
    }

    #[test]
    fn rejects_non_this_and_malformed_cpp_member_receivers() {
        assert!(cpp_this_receiver_from_expression("std::move(other)").is_none());
        assert!(cpp_this_receiver_from_expression("std::forward<Counter&>(other)").is_none());
        assert!(cpp_this_receiver_from_expression("static_cast<Counter&&>(*this").is_none());
    }
}
