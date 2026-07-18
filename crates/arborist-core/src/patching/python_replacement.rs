pub(super) fn normalize_python_replacement_indentation(
    source: &str,
    target_start: usize,
    target_end: usize,
    new_code: &str,
) -> String {
    let normalized_line_endings = normalize_line_endings(new_code, source_line_ending(source));
    let dedented = dedent_python_replacement(&normalized_line_endings);
    let ambient_indent = python_target_ambient_indent(source, target_start);
    let indent_unit = python_target_indent_unit(source, target_start, target_end)
        .or_else(|| infer_python_indent_unit(&dedented))
        .unwrap_or_else(|| ambient_indent.clone());

    if indent_unit.is_empty() {
        return reindent_python_replacement(&dedented, &ambient_indent);
    }

    reindent_python_replacement_with_unit(&dedented, &ambient_indent, &indent_unit)
}

pub(super) fn python_replacement_starts_with_decorator(replacement: &str) -> bool {
    replacement
        .lines()
        .map(str::trim_start)
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| line.starts_with('@'))
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
    let mut adjusted = String::with_capacity(replacement.len() + ambient_indent.len());
    for (index, line) in split_preserving_newline(replacement)
        .into_iter()
        .enumerate()
    {
        if index > 0 && !line.trim().is_empty() {
            adjusted.push_str(ambient_indent);
        }
        adjusted.push_str(line);
    }
    adjusted
}

fn reindent_python_replacement_with_unit(
    replacement: &str,
    ambient_indent: &str,
    indent_unit: &str,
) -> String {
    let indent_step = infer_python_indent_step(replacement);
    if indent_step == 0 {
        return reindent_python_replacement(replacement, ambient_indent);
    }

    let mut adjusted = String::with_capacity(
        replacement.len() + ambient_indent.len() + indent_unit.len() * replacement.lines().count(),
    );
    for (index, line) in split_preserving_newline(replacement)
        .into_iter()
        .enumerate()
    {
        let (content, newline) = split_line_ending(line);
        if content.trim().is_empty() {
            adjusted.push_str(content);
            adjusted.push_str(newline);
            continue;
        }

        let leading = leading_indent_len(content);
        let depth = leading / indent_step;
        if index > 0 {
            adjusted.push_str(ambient_indent);
        }
        for _ in 0..depth {
            adjusted.push_str(indent_unit);
        }
        adjusted.push_str(&content[leading..]);
        adjusted.push_str(newline);
    }
    adjusted
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
    let base_indent = python_target_ambient_indent(source, target_start);
    let target_text = &source[target_start..target_end];
    for line in split_preserving_newline(target_text).into_iter().skip(1) {
        let (content, _) = split_line_ending(line);
        if content.trim().is_empty() {
            continue;
        }
        let indent_len = leading_indent_len(content);
        if indent_len > base_indent.len() && content.starts_with(&base_indent) {
            return Some(content[base_indent.len()..indent_len].to_string());
        }
    }
    None
}

fn infer_python_indent_unit(replacement: &str) -> Option<String> {
    for line in split_preserving_newline(replacement) {
        let (content, _) = split_line_ending(line);
        if content.trim().is_empty() {
            continue;
        }
        let indent_len = leading_indent_len(content);
        if indent_len > 0 {
            return Some(content[..indent_len].to_string());
        }
    }
    None
}

fn infer_python_indent_step(replacement: &str) -> usize {
    let mut step = 0usize;
    for line in split_preserving_newline(replacement) {
        let (content, _) = split_line_ending(line);
        if content.trim().is_empty() {
            continue;
        }
        let indent_len = leading_indent_len(content);
        if indent_len == 0 {
            continue;
        }
        step = if step == 0 {
            indent_len
        } else {
            gcd(step, indent_len)
        };
        if step == 1 {
            break;
        }
    }
    step
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

fn gcd(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
