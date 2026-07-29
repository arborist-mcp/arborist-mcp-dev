use super::super::cpp_syntax::matching_angle_bracket_index;

pub(super) fn cpp_standard_template_arguments<'a>(
    type_name: &'a str,
    wrapper: &str,
) -> Option<&'a str> {
    let contents = type_name.trim().strip_prefix(wrapper)?.strip_prefix('<')?;
    let target_end = matching_angle_bracket_index(contents)?;
    contents[target_end + 1..]
        .trim()
        .is_empty()
        .then_some(&contents[..target_end])
}

pub(super) fn cpp_first_template_argument(arguments: &str) -> Option<&str> {
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

pub(super) fn cpp_second_template_argument(arguments: &str) -> Option<&str> {
    let mut angles = 0usize;
    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;
    for (index, character) in arguments.char_indices() {
        match character {
            '<' => angles += 1,
            '>' => angles = angles.checked_sub(1)?,
            '(' => parentheses += 1,
            ')' => parentheses = parentheses.checked_sub(1)?,
            '[' => brackets += 1,
            ']' => brackets = brackets.checked_sub(1)?,
            '{' => braces += 1,
            '}' => braces = braces.checked_sub(1)?,
            ',' if angles == 0 && parentheses == 0 && brackets == 0 && braces == 0 => {
                return Some(arguments[index + character.len_utf8()..].trim())
                    .filter(|value| !value.is_empty());
            }
            _ => {}
        }
    }
    None
}

pub(super) fn cpp_nth_template_argument(arguments: &str, wanted_index: usize) -> Option<&str> {
    let mut angles = 0usize;
    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;
    let mut argument_start = 0usize;
    let mut argument_index = 0usize;
    for (index, character) in arguments.char_indices() {
        match character {
            '<' => angles += 1,
            '>' => angles = angles.checked_sub(1)?,
            '(' => parentheses += 1,
            ')' => parentheses = parentheses.checked_sub(1)?,
            '[' => brackets += 1,
            ']' => brackets = brackets.checked_sub(1)?,
            '{' => braces += 1,
            '}' => braces = braces.checked_sub(1)?,
            ',' if angles == 0 && parentheses == 0 && brackets == 0 && braces == 0 => {
                if argument_index == wanted_index {
                    return Some(arguments[argument_start..index].trim())
                        .filter(|argument| !argument.is_empty());
                }
                argument_index += 1;
                argument_start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    (argument_index == wanted_index)
        .then(|| arguments[argument_start..].trim())
        .filter(|argument| !argument.is_empty())
}

pub(super) fn cpp_template_arguments_have_top_level_comma(arguments: &str) -> bool {
    let mut depth = 0usize;
    for character in arguments.chars() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

pub(super) fn cpp_has_exactly_two_top_level_template_arguments(arguments: &str) -> bool {
    let mut angles = 0usize;
    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;
    let mut argument_start = 0usize;
    let mut argument_count = 0usize;
    for (index, character) in arguments.char_indices() {
        match character {
            '<' => angles += 1,
            '>' => {
                let Some(next) = angles.checked_sub(1) else {
                    return false;
                };
                angles = next;
            }
            '(' => parentheses += 1,
            ')' => {
                let Some(next) = parentheses.checked_sub(1) else {
                    return false;
                };
                parentheses = next;
            }
            '[' => brackets += 1,
            ']' => {
                let Some(next) = brackets.checked_sub(1) else {
                    return false;
                };
                brackets = next;
            }
            '{' => braces += 1,
            '}' => {
                let Some(next) = braces.checked_sub(1) else {
                    return false;
                };
                braces = next;
            }
            ',' if angles == 0 && parentheses == 0 && brackets == 0 && braces == 0 => {
                if arguments[argument_start..index].trim().is_empty() {
                    return false;
                }
                argument_count += 1;
                argument_start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    angles == 0
        && parentheses == 0
        && brackets == 0
        && braces == 0
        && !arguments[argument_start..].trim().is_empty()
        && argument_count + 1 == 2
}
