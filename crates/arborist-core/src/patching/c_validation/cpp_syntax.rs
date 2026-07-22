use super::cpp_types::cpp_temporary_type_path;

pub(super) fn cpp_constructor_type_text(expression: &str) -> Option<&str> {
    let expression = expression.trim();
    let closing = expression.chars().last()?;
    let opening = match closing {
        ')' => matching_opening_delimiter_index(expression, '(', ')')?,
        '}' => matching_opening_delimiter_index(expression, '{', '}')?,
        _ => return None,
    };
    cpp_type_text(expression[..opening].trim())
}

pub(super) fn cpp_default_initialized_type_path(type_name: &str) -> Option<String> {
    cpp_default_initialized_type_text(type_name).and_then(cpp_temporary_type_path)
}

pub(super) fn cpp_default_initialized_type_text(type_name: &str) -> Option<&str> {
    cpp_type_text(type_name.trim())
}

pub(super) fn cpp_type_text(type_name: &str) -> Option<&str> {
    (!type_name.is_empty()
        && type_name.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '_' | ':' | '<' | '>' | ',' | ' ' | '\t')
        }))
    .then_some(type_name)
}

pub(super) fn cpp_receiver_call_argument<'a>(
    receiver: &'a str,
    function_name: &str,
) -> Option<&'a str> {
    let argument = receiver
        .strip_prefix(function_name)?
        .trim_start()
        .strip_prefix('(')?;
    let argument = argument.trim_end().strip_suffix(')')?.trim();
    parentheses_are_balanced(argument).then_some(argument)
}

pub(super) fn cpp_typed_receiver_call<'a>(
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

pub(super) fn strip_cpp_outer_parentheses(mut expression: &str) -> &str {
    while let Some(inner) = expression
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .filter(|_| parentheses_wrap_entire_expression(expression))
    {
        expression = inner;
    }
    expression
}

pub(super) fn compact_cpp_expression(expression: &str) -> String {
    expression
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

pub(super) fn matching_angle_bracket_index(contents: &str) -> Option<usize> {
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

#[cfg(test)]
mod tests {
    use super::{
        compact_cpp_expression, cpp_constructor_type_text, cpp_default_initialized_type_path,
        cpp_receiver_call_argument, cpp_typed_receiver_call, strip_cpp_outer_parentheses,
    };

    #[test]
    fn strips_only_outer_parentheses() {
        assert_eq!(strip_cpp_outer_parentheses("(((current))"), "(((current))");
        assert_eq!(strip_cpp_outer_parentheses("(((current)))"), "current");
        assert_eq!(
            strip_cpp_outer_parentheses("(current) + value"),
            "(current) + value"
        );
    }

    #[test]
    fn extracts_receiver_call_arguments_with_nested_parentheses() {
        assert_eq!(
            cpp_receiver_call_argument("std::move(factory(value, (other)))", "std::move"),
            Some("factory(value, (other))"),
        );
        assert_eq!(
            cpp_receiver_call_argument("std::move(value))", "std::move"),
            None
        );
    }

    #[test]
    fn extracts_typed_receiver_calls_with_nested_template_arguments() {
        assert_eq!(
            cpp_typed_receiver_call(
                "std::forward<std::pair<api::Value, std::vector<int>>>(current)",
                "std::forward",
            ),
            Some(("std::pair<api::Value, std::vector<int>>", "current")),
        );
        assert_eq!(
            cpp_typed_receiver_call("std::forward<std::vector<int>>(current))", "std::forward"),
            None,
        );
    }

    #[test]
    fn recovers_constructor_and_default_initialized_type_paths() {
        assert_eq!(
            cpp_constructor_type_text(" api::Counter { value } "),
            Some("api::Counter"),
        );
        assert_eq!(cpp_constructor_type_text("value + 1"), None);
        assert_eq!(
            cpp_default_initialized_type_path("const api::Counter"),
            Some("api::Counter".to_string()),
        );
        assert_eq!(cpp_default_initialized_type_path("api::Counter*"), None);
    }

    #[test]
    fn compacts_cpp_expressions() {
        assert_eq!(
            compact_cpp_expression(" std::vector< int > "),
            "std::vector<int>"
        );
    }
}
