use std::collections::BTreeMap;

pub(crate) fn symbol_indexes_for_paths(
    paths: &[String],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Vec<usize> {
    paths
        .iter()
        .flat_map(|path| semantic_path_index.get(path).into_iter().flatten().copied())
        .collect()
}

pub(crate) fn symbol_indexes_for_paths_with_template_fallback(
    paths: &[String],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Vec<usize> {
    let candidates = symbol_indexes_for_paths(paths, semantic_path_index);
    if !candidates.is_empty() {
        return candidates;
    }

    let template_base_paths = paths
        .iter()
        .filter_map(|path| cpp_template_base_path(path))
        .collect::<Vec<_>>();
    symbol_indexes_for_paths(&template_base_paths, semantic_path_index)
}

pub(crate) fn cpp_template_base_path(path: &str) -> Option<String> {
    let mut depth = 0usize;
    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;
    let mut base_path = String::with_capacity(path.len());
    let characters = path.chars().collect::<Vec<_>>();

    for (index, character) in characters.iter().copied().enumerate() {
        match character {
            '<' if parentheses == 0 && brackets == 0 && braces == 0 => depth += 1,
            '>' if depth > 0
                && parentheses == 0
                && brackets == 0
                && braces == 0
                && cpp_template_argument_closes(&characters[index + 1..]) =>
            {
                depth -= 1;
            }
            '(' => parentheses += 1,
            ')' => parentheses = parentheses.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            _ if depth == 0 => base_path.push(character),
            _ => {}
        }
    }

    (depth == 0 && parentheses == 0 && brackets == 0 && braces == 0 && base_path != path)
        .then_some(base_path)
}

pub(crate) fn cpp_template_argument_closes(remaining: &[char]) -> bool {
    matches!(
        remaining
            .iter()
            .copied()
            .find(|character| !character.is_whitespace()),
        None | Some('>' | ',' | ')' | ']' | '}' | ':' | '.')
    )
}

#[cfg(test)]
mod tests {
    use super::cpp_template_base_path;

    #[test]
    fn cpp_template_base_path_preserves_nested_non_type_arguments() {
        assert_eq!(
            cpp_template_base_path("api::Box<detail::Tag>").as_deref(),
            Some("api::Box")
        );
        assert_eq!(
            cpp_template_base_path("api::Box<(1 > 0)>").as_deref(),
            Some("api::Box")
        );
    }
}
