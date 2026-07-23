use std::path::Path;

use tree_sitter::Node;

use super::find_first_descendant_by_kind;
use crate::language::{node_text, parse_document};

pub(crate) fn cpp_callable_symbol_id(
    semantic_path: &str,
    parameters: &[String],
    signature: Option<&str>,
) -> String {
    let parameter_types = parameters
        .iter()
        .map(|parameter| cpp_parameter_type_identity(parameter))
        .collect::<Vec<_>>();
    let qualifiers = signature
        .and_then(cpp_member_qualifier_identity)
        .unwrap_or_default();
    format!("{semantic_path}({}){qualifiers}", parameter_types.join(","))
}

fn cpp_parameter_type_identity(parameter: &str) -> String {
    if parameter.trim() == "void" {
        return String::new();
    }

    cpp_parameter_type_identity_from_tree(parameter).unwrap_or_else(|| {
        let parameter = strip_cpp_default_argument(parameter).trim();
        compact_cpp_identity_text(&strip_cpp_parameter_name(parameter))
    })
}

fn cpp_parameter_type_identity_from_tree(parameter: &str) -> Option<String> {
    let source = format!("void arborist_identity({parameter});");
    let document = parse_document(Path::new("arborist_identity.cpp"), &source).ok()?;
    let parameter_node = find_first_descendant_by_kind(
        document.tree.root_node(),
        if parameter.trim() == "..." {
            "variadic_parameter_declaration"
        } else {
            "parameter_declaration"
        },
    )?;
    if parameter_node.kind() == "variadic_parameter_declaration" {
        return Some("...".to_string());
    }

    let parameter_text = node_text(parameter_node, &source).ok()?;
    let parameter_text = strip_cpp_default_argument(parameter_text).trim();
    let mut name_ranges = Vec::new();
    collect_cpp_parameter_name_ranges(parameter_node, &mut name_ranges);
    name_ranges.sort_unstable();
    name_ranges.dedup();

    let mut normalized = parameter_text.to_string();
    for (start, end) in name_ranges.into_iter().rev() {
        if start < parameter_node.start_byte()
            || end > parameter_node.start_byte() + parameter_text.len()
        {
            continue;
        }
        let start = start - parameter_node.start_byte();
        let end = end - parameter_node.start_byte();
        normalized.replace_range(start..end, "");
    }
    Some(compact_cpp_identity_text(&normalized))
}

fn collect_cpp_parameter_name_ranges(node: Node<'_>, ranges: &mut Vec<(usize, usize)>) {
    if node.kind() == "parameter_declaration"
        && let Some(declarator) = node.child_by_field_name("declarator")
        && let Some(name) = cpp_declarator_name_node(declarator)
    {
        ranges.push((name.start_byte(), name.end_byte()));
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_cpp_parameter_name_ranges(child, ranges);
    }
}

fn cpp_declarator_name_node(node: Node<'_>) -> Option<Node<'_>> {
    if matches!(node.kind(), "identifier" | "field_identifier") {
        return Some(node);
    }
    if let Some(declarator) = node.child_by_field_name("declarator")
        && let Some(name) = cpp_declarator_name_node(declarator)
    {
        return Some(name);
    }

    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    children
        .into_iter()
        .rev()
        .find_map(cpp_declarator_name_node)
}

fn strip_cpp_default_argument(parameter: &str) -> &str {
    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;

    for (index, character) in parameter.char_indices() {
        match character {
            '(' => parentheses += 1,
            ')' => parentheses = parentheses.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            '=' if parentheses == 0 && brackets == 0 && braces == 0 => {
                return &parameter[..index];
            }
            _ => {}
        }
    }

    parameter
}

fn strip_cpp_parameter_name(parameter: &str) -> String {
    let identifiers = cpp_identifier_spans(parameter);
    let Some((start, end)) = cpp_parameter_name_span(parameter, &identifiers) else {
        return parameter.to_string();
    };
    format!("{}{}", &parameter[..start], &parameter[end..])
}

fn cpp_parameter_name_span(
    parameter: &str,
    identifiers: &[(usize, usize)],
) -> Option<(usize, usize)> {
    let pointer_name = identifiers.iter().rev().find(|(start, _)| {
        let prefix = parameter[..*start].trim_end();
        prefix.ends_with('*') || prefix.ends_with('&')
    });
    if let Some((start, end)) = pointer_name {
        return Some((*start, *end));
    }

    let (start, end) = *identifiers
        .iter()
        .rev()
        .find(|(start, _)| !cpp_identifier_is_inside_template_argument(parameter, *start))?;
    let prefix = parameter[..start].trim_end();
    if prefix.ends_with("::") || cpp_identifier_is_unnamed_type(parameter, identifiers) {
        return None;
    }
    Some((start, end))
}

fn cpp_identifier_is_inside_template_argument(parameter: &str, identifier_start: usize) -> bool {
    let mut depth = 0usize;
    for character in parameter[..identifier_start].chars() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth > 0
}

fn cpp_identifier_is_unnamed_type(parameter: &str, identifiers: &[(usize, usize)]) -> bool {
    if identifiers.len() == 1 {
        return true;
    }

    let (last_start, _) = identifiers[identifiers.len() - 1];
    let mut preceding_identifiers = identifiers[..identifiers.len() - 1]
        .iter()
        .map(|(start, end)| &parameter[*start..*end]);
    preceding_identifiers.all(|identifier| {
        matches!(
            identifier,
            "const" | "volatile" | "typename" | "class" | "struct" | "enum"
        )
    }) && !parameter[..last_start].contains(['*', '&', '[', ']'])
}

fn cpp_identifier_spans(input: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = None;

    for (index, character) in input.char_indices() {
        if character == '_' || character.is_ascii_alphanumeric() {
            start.get_or_insert(index);
        } else if let Some(identifier_start) = start.take() {
            spans.push((identifier_start, index));
        }
    }
    if let Some(identifier_start) = start {
        spans.push((identifier_start, input.len()));
    }
    spans
}

fn compact_cpp_identity_text(input: &str) -> String {
    let mut compact = String::new();
    let mut pending_space = false;

    for character in input.trim().chars() {
        if character.is_whitespace() {
            pending_space = !compact.is_empty();
            continue;
        }

        let punctuation = matches!(
            character,
            '*' | '&' | ',' | '<' | '>' | '(' | ')' | '[' | ']' | ':'
        );
        if pending_space
            && !punctuation
            && !compact.chars().last().is_some_and(|previous| {
                matches!(previous, '*' | '&' | ',' | '<' | '>' | '(' | '[' | ':')
            })
        {
            compact.push(' ');
        }
        if punctuation && compact.ends_with(' ') {
            compact.pop();
        }
        compact.push(character);
        pending_space = false;
    }

    compact
}

fn cpp_member_qualifier_identity(signature: &str) -> Option<String> {
    let mut depth = 0usize;
    let mut parameter_end = None;

    for (index, character) in signature.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    parameter_end = Some(index + character.len_utf8());
                }
            }
            _ => {}
        }
    }

    let parameter_end = parameter_end?;
    let tail = signature.get(parameter_end..)?.trim_start();
    let mut qualifiers = Vec::new();
    let mut remaining = tail;
    loop {
        if let Some(rest) = remaining.strip_prefix("const") {
            qualifiers.push("const");
            remaining = rest.trim_start();
        } else if let Some(rest) = remaining.strip_prefix("volatile") {
            qualifiers.push("volatile");
            remaining = rest.trim_start();
        } else if let Some(rest) = remaining.strip_prefix("&&") {
            qualifiers.push("&&");
            remaining = rest.trim_start();
        } else if let Some(rest) = remaining.strip_prefix('&') {
            qualifiers.push("&");
            remaining = rest.trim_start();
        } else {
            break;
        }
    }

    (!qualifiers.is_empty()).then(|| format!(" {}", qualifiers.join(" ")))
}
