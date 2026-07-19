use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, visit_tree};
use crate::symbol_index_model::{
    CPP_CONST_LVALUE_THIS_CALL_PREFIX, CPP_CONST_RVALUE_THIS_CALL_PREFIX,
    CPP_RVALUE_THIS_CALL_PREFIX,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum CppThisMemberReceiver {
    Lvalue,
    ConstLvalue,
    Rvalue,
    ConstRvalue,
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
    let mut callback = |candidate: Node<'_>| match candidate.kind() {
        "call_expression" => collect_cpp_call_arity(candidate, source, call_arities),
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
) {
    let Some(function) = candidate.child_by_field_name("function") else {
        return;
    };
    let Some(arguments) = candidate.child_by_field_name("arguments") else {
        return;
    };
    let Ok(Some((name, receiver))) = direct_cpp_call_name(function, source) else {
        return;
    };

    let name = encode_cpp_this_member_call_name(name, receiver);
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
    let Some(type_node) = candidate.child_by_field_name("type") else {
        return;
    };
    let Ok(Some(name)) = direct_c_call_name(type_node, source) else {
        return;
    };

    let arity = candidate
        .child_by_field_name("arguments")
        .map(named_child_count)
        .unwrap_or_default();
    record_c_call_arity_with_count(name, arity, call_arities);
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
) -> Result<Option<(String, CppThisMemberReceiver)>> {
    if let Some(name) = direct_c_call_name(function, source)? {
        return Ok(Some((name, CppThisMemberReceiver::Lvalue)));
    }
    if function.kind() != "field_expression" {
        return Ok(None);
    }

    let Some(argument) = function.child_by_field_name("argument") else {
        return Ok(None);
    };
    let Some(receiver) = cpp_this_member_receiver(argument, source)? else {
        return Ok(None);
    };

    let Some(field) = function.child_by_field_name("field") else {
        return Ok(None);
    };
    cpp_member_call_name(field, source).map(|name| name.map(|name| (name, receiver)))
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

fn cpp_this_receiver_for_type(
    type_name: &str,
    default_rvalue: Option<bool>,
) -> Option<CppThisMemberReceiver> {
    let normalized_type_name = type_name
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let rvalue = if normalized_type_name.ends_with("&&") {
        true
    } else if normalized_type_name.ends_with('&') {
        false
    } else {
        default_rvalue?
    };
    let const_qualified = cpp_type_is_top_level_const(type_name);
    Some(match (const_qualified, rvalue) {
        (false, false) => CppThisMemberReceiver::Lvalue,
        (true, false) => CppThisMemberReceiver::ConstLvalue,
        (false, true) => CppThisMemberReceiver::Rvalue,
        (true, true) => CppThisMemberReceiver::ConstRvalue,
    })
}

fn cpp_type_is_top_level_const(type_name: &str) -> bool {
    let mut template_depth = 0usize;
    let mut characters = type_name.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        match character {
            '<' => template_depth += 1,
            '>' => template_depth = template_depth.saturating_sub(1),
            character if character.is_ascii_alphabetic() || character == '_' => {
                let mut end = index + character.len_utf8();
                while let Some((next_index, next_character)) = characters.peek().copied() {
                    if next_character.is_ascii_alphanumeric() || next_character == '_' {
                        end = next_index + next_character.len_utf8();
                        characters.next();
                    } else {
                        break;
                    }
                }
                if template_depth == 0 && &type_name[index..end] == "const" {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
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

    use super::{collect_cpp_call_arities, cpp_type_is_top_level_const};

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
    fn collects_this_member_call_arities_without_inferring_other_objects() {
        let source = "class Counter { int adjust(int value) { return value; } int caller(Counter* other) { return this->adjust(1) + (*this).adjust(1, 2) + other->adjust(1, 2, 3); } };";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([("adjust".to_string(), BTreeSet::from([1, 2]))])
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
    fn identifies_only_top_level_cpp_const_qualifiers() {
        assert!(cpp_type_is_top_level_const("const Counter&&"));
        assert!(cpp_type_is_top_level_const("Counter const &"));
        assert!(!cpp_type_is_top_level_const("constCounter&&"));
        assert!(!cpp_type_is_top_level_const("Wrapper<const Counter>&&"));
    }
}
