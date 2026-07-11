use tree_sitter::Node;

use crate::language::{contains_node, node_text};

pub(super) fn python_match_capture_names(case_pattern: Node<'_>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = case_pattern.walk();
    for child in case_pattern.named_children(&mut cursor) {
        python_collect_direct_match_capture_names(child, source, &mut names);
    }
    names
}

fn python_splat_pattern_capture_name(splat_pattern: Node<'_>, source: &str) -> Option<String> {
    let identifier = only_named_child(splat_pattern)?;
    let name = node_text(identifier, source).ok()?.trim();
    is_python_capture_name_text(name).then(|| name.to_string())
}

fn python_as_pattern_alias_name(as_pattern: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = as_pattern.walk();
    as_pattern
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "identifier" || child.kind() == "as_pattern_target")
        .last()
        .and_then(|alias| node_text(alias, source).ok())
        .map(str::trim)
        .filter(|name| is_python_capture_name_text(name))
        .map(str::to_string)
}

fn python_collect_direct_match_capture_names(
    node: Node<'_>,
    source: &str,
    names: &mut Vec<String>,
) {
    match node.kind() {
        "case_pattern" => {}
        "as_pattern" => {
            if let Some(name) = python_as_pattern_alias_name(node, source) {
                push_python_match_capture_name(names, &name);
            }
        }
        "keyword_pattern" => {
            let mut cursor = node.walk();
            if let Some(value) = node.named_children(&mut cursor).last() {
                python_collect_direct_match_capture_names(value, source, names);
            }
        }
        "splat_pattern" => {
            if let Some(name) = python_splat_pattern_capture_name(node, source) {
                push_python_match_capture_name(names, &name);
            }
        }
        "pattern" => {
            if let Ok(name) = node_text(node, source) {
                push_python_match_capture_name(names, name.trim());
            }
        }
        "dotted_name" => {
            if let Some(identifier) = only_named_child(node)
                && let Ok(name) = node_text(identifier, source)
            {
                push_python_match_capture_name(names, name.trim());
            }
        }
        "class_pattern" => {
            let mut cursor = node.walk();
            for (index, child) in node.named_children(&mut cursor).enumerate() {
                if index == 0 || child.kind() == "case_pattern" {
                    continue;
                }
                python_collect_direct_match_capture_names(child, source, names);
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "case_pattern" {
                    continue;
                }
                python_collect_direct_match_capture_names(child, source, names);
            }
        }
    }
}

fn push_python_match_capture_name(names: &mut Vec<String>, name: &str) {
    if !is_python_capture_name_text(name) {
        return;
    }
    if !names.iter().any(|existing| existing == name) {
        names.push(name.to_string());
    }
}

pub(super) fn python_enclosing_case_clause(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "case_clause" {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn only_named_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    let mut children = node.named_children(&mut cursor);
    let child = children.next()?;
    children.next().is_none().then_some(child)
}

fn is_python_capture_name_text(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first == '_' && chars.clone().next().is_some()
        || first.is_ascii_alphabetic() && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

pub(super) fn is_python_match_capture_name(node: Node<'_>, source: &str) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    let mut current = Some(parent);
    while let Some(candidate) = current {
        if candidate.kind() == "case_pattern" {
            return python_match_capture_names(candidate, source)
                .into_iter()
                .any(|name| {
                    node_text(node, source)
                        .ok()
                        .is_some_and(|node_name| node_name.trim() == name)
                });
        }

        if matches!(
            candidate.kind(),
            "case_clause" | "function_definition" | "class_definition" | "module"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

pub(super) fn is_python_match_keyword_name(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != "keyword_pattern" {
        return false;
    }

    let mut cursor = parent.walk();
    parent
        .named_children(&mut cursor)
        .next()
        .is_some_and(|keyword| keyword.id() == node.id())
}

pub(super) fn is_python_as_pattern_alias(node: Node<'_>, ancestor: Node<'_>, source: &str) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != "as_pattern" || !contains_node(ancestor, parent) {
        return false;
    }

    let Some(pattern_text) = source.get(parent.start_byte()..parent.end_byte()) else {
        return false;
    };
    let Some(as_index) = pattern_text.rfind(" as ") else {
        return false;
    };
    let relative_start = node.start_byte().saturating_sub(parent.start_byte());
    relative_start > as_index
}
