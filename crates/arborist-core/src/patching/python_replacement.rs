use std::path::Path;

use tree_sitter::Node;

use crate::language::parse_document;

pub(super) fn normalize_python_replacement_indentation(
    source: &str,
    target_start: usize,
    target_end: usize,
    target_is_decorated: bool,
    new_code: &str,
) -> String {
    let normalized_line_endings = normalize_line_endings(new_code, source_line_ending(source));
    let ambient_indent = python_target_ambient_indent(source, target_start);
    let relative_replacement = normalize_decorated_absolute_indentation(
        &normalized_line_endings,
        &ambient_indent,
        target_is_decorated,
    );
    let dedented = dedent_python_replacement(&relative_replacement);
    let replacement_indent_unit = python_replacement_indent_unit(&dedented);
    let indent_unit = python_target_indent_unit(source, target_start, target_end)
        .or_else(|| replacement_indent_unit.clone())
        .unwrap_or_else(|| ambient_indent.clone());

    if indent_unit.is_empty()
        || replacement_indent_unit.is_none()
        || replacement_indent_unit.as_deref() == Some(indent_unit.as_str())
    {
        return reindent_python_replacement(&dedented, &ambient_indent);
    }

    reindent_python_replacement_with_unit(
        &dedented,
        &ambient_indent,
        replacement_indent_unit.as_deref().unwrap_or(&indent_unit),
        &indent_unit,
    )
}

pub(super) fn python_replacement_starts_with_decorator(replacement: &str) -> bool {
    replacement
        .lines()
        .map(str::trim_start)
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| line.starts_with('@'))
}

fn normalize_decorated_absolute_indentation(
    replacement: &str,
    ambient_indent: &str,
    target_is_decorated: bool,
) -> String {
    if ambient_indent.is_empty()
        || !target_is_decorated
        || !python_replacement_starts_with_decorator(replacement)
        || !decorated_replacement_uses_absolute_definition_indentation(replacement, ambient_indent)
    {
        return replacement.to_string();
    }

    let multiline_string_rows = multiline_string_content_rows(replacement);
    let mut adjusted = String::with_capacity(replacement.len());
    for (row, line) in split_preserving_newline(replacement)
        .into_iter()
        .enumerate()
    {
        if row == 0 || line.trim().is_empty() || row_is_in_ranges(row, &multiline_string_rows) {
            adjusted.push_str(line);
            continue;
        }
        adjusted.push_str(line.strip_prefix(ambient_indent).unwrap_or(line));
    }
    adjusted
}

fn decorated_replacement_uses_absolute_definition_indentation(
    replacement: &str,
    ambient_indent: &str,
) -> bool {
    decorated_definition_columns(replacement).is_some_and(|columns| {
        columns.decorator_columns.first() == Some(&0)
            && columns
                .decorator_columns
                .iter()
                .skip(1)
                .all(|column| *column == ambient_indent.len())
            && columns.definition_column == ambient_indent.len()
    })
}

struct DecoratedDefinitionColumns {
    decorator_columns: Vec<usize>,
    definition_column: usize,
}

fn decorated_definition_columns(source: &str) -> Option<DecoratedDefinitionColumns> {
    let document = parse_document(Path::new("replacement.py"), source).ok()?;
    let decorated = find_first_node(document.tree.root_node(), "decorated_definition")?;
    let mut cursor = decorated.walk();
    let mut decorator_columns = Vec::new();
    let mut definition_column = None;
    for child in decorated.named_children(&mut cursor) {
        match child.kind() {
            "decorator" => {
                decorator_columns.push(child.start_position().column);
            }
            "function_definition" | "class_definition" => {
                definition_column = Some(child.start_position().column);
            }
            _ => {}
        }
    }
    (!decorator_columns.is_empty()).then_some(DecoratedDefinitionColumns {
        decorator_columns,
        definition_column: definition_column?,
    })
}

fn find_first_node<'tree>(root: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.kind() == kind {
            return Some(node);
        }
        let mut cursor = node.walk();
        let children = node.named_children(&mut cursor).collect::<Vec<_>>();
        pending.extend(children.into_iter().rev());
    }
    None
}

fn dedent_python_replacement(new_code: &str) -> String {
    let indent = split_preserving_newline(new_code)
        .iter()
        .filter_map(|line| {
            let content = line.trim_end_matches(['\r', '\n']);
            (!content.trim().is_empty()).then(|| leading_indent_len(content))
        })
        .min()
        .unwrap_or(0);

    if indent == 0 {
        return new_code.to_string();
    }

    let mut dedented = String::with_capacity(new_code.len());
    for line in split_preserving_newline(new_code) {
        let remove = indent.min(leading_indent_len(line));
        dedented.push_str(&line[remove..]);
    }
    dedented
}

fn reindent_python_replacement(replacement: &str, ambient_indent: &str) -> String {
    let multiline_string_rows = multiline_string_content_rows(replacement);
    let mut adjusted = String::with_capacity(replacement.len() + ambient_indent.len());
    for (row, line) in split_preserving_newline(replacement)
        .into_iter()
        .enumerate()
    {
        if row > 0 && !line.trim().is_empty() && !row_is_in_ranges(row, &multiline_string_rows) {
            adjusted.push_str(ambient_indent);
        }
        adjusted.push_str(line);
    }
    adjusted
}

fn reindent_python_replacement_with_unit(
    replacement: &str,
    ambient_indent: &str,
    replacement_indent_unit: &str,
    target_indent_unit: &str,
) -> String {
    let multiline_string_rows = multiline_string_content_rows(replacement);
    let decorator_continuation_rows = decorator_continuation_rows(replacement);
    let mut adjusted = String::with_capacity(
        replacement.len()
            + ambient_indent.len()
            + target_indent_unit.len() * replacement.lines().count(),
    );
    for (row, line) in split_preserving_newline(replacement)
        .into_iter()
        .enumerate()
    {
        let (content, newline) = split_line_ending(line);
        if content.trim().is_empty() {
            adjusted.push_str(content);
            adjusted.push_str(newline);
            continue;
        }

        if row_is_in_ranges(row, &multiline_string_rows) {
            adjusted.push_str(content);
            adjusted.push_str(newline);
            continue;
        }
        if row > 0 {
            adjusted.push_str(ambient_indent);
        }
        if row_is_in_ranges(row, &decorator_continuation_rows) {
            adjusted.push_str(content);
            adjusted.push_str(newline);
            continue;
        }

        let leading = leading_indent_len(content);
        adjusted.push_str(&convert_indent_prefix(
            &content[..leading],
            replacement_indent_unit,
            target_indent_unit,
        ));
        adjusted.push_str(&content[leading..]);
        adjusted.push_str(newline);
    }
    adjusted
}

fn convert_indent_prefix(prefix: &str, source_unit: &str, target_unit: &str) -> String {
    if source_unit.is_empty() || source_unit == target_unit {
        return prefix.to_string();
    }

    let mut converted = String::with_capacity(prefix.len());
    let mut remaining = prefix;
    while let Some(rest) = remaining.strip_prefix(source_unit) {
        converted.push_str(target_unit);
        remaining = rest;
    }
    converted.push_str(remaining);
    converted
}

fn multiline_string_content_rows(source: &str) -> Vec<(usize, usize)> {
    let Ok(document) = parse_document(Path::new("replacement.py"), source) else {
        return Vec::new();
    };
    let mut ranges = Vec::new();
    let mut pending = vec![document.tree.root_node()];
    while let Some(node) = pending.pop() {
        let start_row = node.start_position().row;
        let end_row = node.end_position().row;
        if node.kind() == "string" && end_row > start_row {
            ranges.push((start_row + 1, end_row));
            continue;
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    ranges
}

fn decorator_continuation_rows(source: &str) -> Vec<(usize, usize)> {
    let Ok(document) = parse_document(Path::new("replacement.py"), source) else {
        return Vec::new();
    };
    let mut ranges = Vec::new();
    let mut pending = vec![document.tree.root_node()];
    while let Some(node) = pending.pop() {
        let start_row = node.start_position().row;
        let end_row = node.end_position().row;
        if node.kind() == "decorator" && end_row > start_row {
            ranges.push((start_row + 1, end_row));
            continue;
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    ranges
}

fn row_is_in_ranges(row: usize, ranges: &[(usize, usize)]) -> bool {
    ranges
        .iter()
        .any(|(start, end)| row >= *start && row <= *end)
}

fn python_target_ambient_indent(source: &str, target_start: usize) -> String {
    let line_start = source[..target_start]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let prefix = &source[line_start..target_start];
    if prefix.chars().all(|ch| ch == ' ' || ch == '\t') {
        prefix.to_string()
    } else {
        String::new()
    }
}

fn python_target_indent_unit(
    source: &str,
    target_start: usize,
    target_end: usize,
) -> Option<String> {
    let document = parse_document(Path::new("source.py"), source).ok()?;
    let target = find_node_by_byte_range(document.tree.root_node(), target_start, target_end)?;
    definition_indent_unit(source, target)
}

fn python_replacement_indent_unit(replacement: &str) -> Option<String> {
    let document = parse_document(Path::new("replacement.py"), replacement).ok()?;
    let root = document.tree.root_node();
    let target = find_first_node(root, "decorated_definition")
        .or_else(|| find_first_node(root, "function_definition"))
        .or_else(|| find_first_node(root, "class_definition"))?;
    definition_indent_unit(replacement, target)
}

fn definition_indent_unit(source: &str, target: Node<'_>) -> Option<String> {
    let definition = definition_node(target)?;
    let body = definition.child_by_field_name("body")?;
    let first_statement = body.named_child(0)?;
    if first_statement.start_position().row == definition.start_position().row {
        return None;
    }

    let definition_indent = line_indent_at(source, definition.start_byte())?;
    let statement_indent = line_indent_at(source, first_statement.start_byte())?;
    let indent_unit = statement_indent.strip_prefix(definition_indent)?;
    (!indent_unit.is_empty()).then(|| indent_unit.to_string())
}

fn definition_node(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "function_definition" | "class_definition" => Some(node),
        "decorated_definition" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| matches!(child.kind(), "function_definition" | "class_definition"))
        }
        _ => None,
    }
}

fn line_indent_at(source: &str, byte: usize) -> Option<&str> {
    let line_start = source[..byte].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &source[line_start..byte];
    prefix
        .bytes()
        .all(|byte| byte == b' ' || byte == b'\t')
        .then_some(prefix)
}

fn find_node_by_byte_range(root: Node<'_>, start_byte: usize, end_byte: usize) -> Option<Node<'_>> {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.start_byte() == start_byte
            && node.end_byte() == end_byte
            && definition_node(node).is_some()
        {
            return Some(node);
        }
        if node.start_byte() > start_byte || node.end_byte() < end_byte {
            continue;
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    None
}

fn source_line_ending(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn normalize_line_endings(value: &str, line_ending: &str) -> String {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    if line_ending == "\n" {
        normalized
    } else {
        normalized.replace('\n', line_ending)
    }
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix("\r\n") {
        (body, "\r\n")
    } else if let Some(body) = line.strip_suffix('\n') {
        (body, "\n")
    } else {
        (line, "")
    }
}

fn split_preserving_newline(value: &str) -> Vec<&str> {
    if value.is_empty() {
        return vec![""];
    }

    let mut lines = value.split_inclusive('\n').collect::<Vec<_>>();
    if !value.ends_with('\n')
        && let Some(last_newline) = value.rfind('\n')
        && last_newline + 1 < value.len()
        && lines.is_empty()
    {
        lines.push(&value[last_newline + 1..]);
    }
    lines
}

pub(super) fn leading_indent_len(line: &str) -> usize {
    line.as_bytes()
        .iter()
        .take_while(|byte| **byte == b' ' || **byte == b'\t')
        .count()
}
