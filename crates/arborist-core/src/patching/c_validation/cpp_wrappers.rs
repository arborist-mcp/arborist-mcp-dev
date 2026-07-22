pub(super) fn cpp_standard_smart_pointer_target_type(type_name: &str) -> Option<&str> {
    ["std::unique_ptr", "std::shared_ptr"]
        .into_iter()
        .find_map(|pointer_type| {
            cpp_standard_template_arguments(type_name, pointer_type)
                .and_then(cpp_first_template_argument)
        })
}

pub(super) fn cpp_standard_reference_wrapper_target_type(type_name: &str) -> Option<&str> {
    cpp_standard_template_arguments(type_name, "std::reference_wrapper")
        .filter(|arguments| !cpp_template_arguments_have_top_level_comma(arguments))
        .and_then(cpp_first_template_argument)
}

pub(super) fn cpp_standard_weak_pointer_target_type(type_name: &str) -> Option<&str> {
    cpp_standard_template_arguments(type_name, "std::weak_ptr")
        .filter(|arguments| !cpp_template_arguments_have_top_level_comma(arguments))
        .and_then(cpp_first_template_argument)
}

pub(super) fn cpp_standard_optional_target_type(type_name: &str) -> Option<&str> {
    cpp_standard_template_arguments(type_name, "std::optional")
        .filter(|arguments| !cpp_template_arguments_have_top_level_comma(arguments))
        .and_then(cpp_first_template_argument)
}

pub(super) fn cpp_standard_expected_target_type(type_name: &str) -> Option<&str> {
    let arguments = cpp_standard_template_arguments(type_name, "std::expected")?;
    cpp_has_exactly_two_top_level_template_arguments(arguments)
        .then(|| cpp_first_template_argument(arguments))?
}

pub(super) fn cpp_standard_expected_error_type(type_name: &str) -> Option<&str> {
    let arguments = cpp_standard_template_arguments(type_name, "std::expected")?;
    cpp_has_exactly_two_top_level_template_arguments(arguments)
        .then(|| cpp_second_template_argument(arguments))?
}

pub(super) fn cpp_standard_sequence_element_type(type_name: &str) -> Option<&str> {
    [
        "std::array",
        "std::deque",
        "std::list",
        "std::span",
        "std::vector",
    ]
    .into_iter()
    .find_map(|sequence_type| {
        cpp_standard_template_arguments(type_name, sequence_type)
            .and_then(cpp_first_template_argument)
    })
}

pub(super) fn cpp_standard_indexable_sequence_element_type(type_name: &str) -> Option<&str> {
    ["std::array", "std::deque", "std::span", "std::vector"]
        .into_iter()
        .find_map(|sequence_type| {
            cpp_standard_template_arguments(type_name, sequence_type)
                .and_then(cpp_first_template_argument)
        })
}

pub(super) fn cpp_standard_contiguous_sequence_element_type(type_name: &str) -> Option<&str> {
    ["std::array", "std::span", "std::vector"]
        .into_iter()
        .find_map(|sequence_type| {
            cpp_standard_template_arguments(type_name, sequence_type)
                .and_then(cpp_first_template_argument)
        })
}

pub(super) fn cpp_standard_tuple_element_type(type_name: &str, index: usize) -> Option<&str> {
    ["std::tuple", "std::pair"]
        .into_iter()
        .find_map(|tuple_type| {
            cpp_standard_template_arguments(type_name, tuple_type)
                .and_then(|arguments| cpp_nth_template_argument(arguments, index))
        })
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

fn cpp_standard_template_arguments<'a>(type_name: &'a str, wrapper: &str) -> Option<&'a str> {
    let contents = type_name.trim().strip_prefix(wrapper)?.strip_prefix('<')?;
    let target_end = matching_angle_bracket_index(contents)?;
    contents[target_end + 1..]
        .trim()
        .is_empty()
        .then_some(&contents[..target_end])
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

fn cpp_second_template_argument(arguments: &str) -> Option<&str> {
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

fn cpp_nth_template_argument(arguments: &str, wanted_index: usize) -> Option<&str> {
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

fn cpp_template_arguments_have_top_level_comma(arguments: &str) -> bool {
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

fn cpp_has_exactly_two_top_level_template_arguments(arguments: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::{
        cpp_standard_contiguous_sequence_element_type, cpp_standard_expected_error_type,
        cpp_standard_expected_target_type, cpp_standard_indexable_sequence_element_type,
        cpp_standard_optional_target_type, cpp_standard_reference_wrapper_target_type,
        cpp_standard_sequence_element_type, cpp_standard_smart_pointer_target_type,
        cpp_standard_tuple_element_type,
    };

    #[test]
    fn extracts_standard_wrapper_target_types() {
        assert_eq!(
            cpp_standard_smart_pointer_target_type("std::unique_ptr<Wrapper<Alias, Tag>, Deleter>"),
            Some("Wrapper<Alias, Tag>")
        );
        assert_eq!(
            cpp_standard_smart_pointer_target_type("std::shared_ptr<const Counter>"),
            Some("const Counter")
        );
        assert!(cpp_standard_smart_pointer_target_type("std::unique_ptr<>").is_none());
        assert!(
            cpp_standard_smart_pointer_target_type("std::shared_ptr<Counter> trailing").is_none()
        );

        assert_eq!(
            cpp_standard_reference_wrapper_target_type("std::reference_wrapper<const Counter>"),
            Some("const Counter")
        );
        assert!(cpp_standard_reference_wrapper_target_type("std::reference_wrapper<>").is_none());
        assert!(
            cpp_standard_reference_wrapper_target_type("std::reference_wrapper<Counter, Tag>")
                .is_none()
        );

        assert_eq!(
            cpp_standard_optional_target_type("std::optional<const Wrapper<Counter, Tag>>"),
            Some("const Wrapper<Counter, Tag>")
        );
        assert!(cpp_standard_optional_target_type("std::optional<>").is_none());
        assert!(cpp_standard_optional_target_type("std::optional<Counter> trailing").is_none());
        assert!(cpp_standard_optional_target_type("std::optional<Counter, Tag>").is_none());
    }

    #[test]
    fn extracts_standard_expected_value_and_error_types() {
        assert_eq!(
            cpp_standard_expected_target_type("std::expected<Counter, Error>"),
            Some("Counter")
        );
        assert_eq!(
            cpp_standard_expected_target_type("std::expected<std::vector<int>, Error>"),
            Some("std::vector<int>")
        );
        assert_eq!(
            cpp_standard_expected_target_type("std::expected<Counter, void (*)(int, int)>"),
            Some("Counter")
        );
        assert_eq!(
            cpp_standard_expected_error_type("std::expected<Counter, Error>"),
            Some("Error")
        );
        assert_eq!(
            cpp_standard_expected_error_type("std::expected<Counter, void (*)(int, int)>"),
            Some("void (*)(int, int)")
        );
        assert_eq!(
            cpp_standard_expected_error_type("std::expected<Value, Wrapper<Error, Tag>>"),
            Some("Wrapper<Error, Tag>")
        );
        assert!(cpp_standard_expected_target_type("std::expected<Counter>").is_none());
        assert!(cpp_standard_expected_error_type("std::expected<Counter>").is_none());
        assert!(cpp_standard_expected_target_type("std::expected<Counter, >").is_none());
        assert!(
            cpp_standard_expected_target_type("std::expected<Counter, Error, Extra>").is_none()
        );
    }

    #[test]
    fn extracts_standard_sequence_element_types() {
        assert_eq!(
            cpp_standard_sequence_element_type("std::vector<Wrapper<Alias, Tag>>"),
            Some("Wrapper<Alias, Tag>")
        );
        assert_eq!(
            cpp_standard_sequence_element_type("std::span<const Counter, 4>"),
            Some("const Counter")
        );
        assert_eq!(
            cpp_standard_sequence_element_type("std::array<Counter, 2>"),
            Some("Counter")
        );
        assert!(cpp_standard_sequence_element_type("std::vector<>").is_none());
        assert!(cpp_standard_sequence_element_type("std::set<Counter>").is_none());
        assert_eq!(
            cpp_standard_indexable_sequence_element_type("std::vector<Counter>"),
            Some("Counter")
        );
        assert_eq!(
            cpp_standard_indexable_sequence_element_type("std::span<const Counter, 4>"),
            Some("const Counter")
        );
        assert!(cpp_standard_indexable_sequence_element_type("std::list<Counter>").is_none());
        assert_eq!(
            cpp_standard_contiguous_sequence_element_type("std::array<Counter, 2>"),
            Some("Counter")
        );
        assert!(cpp_standard_contiguous_sequence_element_type("std::deque<Counter>").is_none());
    }

    #[test]
    fn extracts_standard_tuple_element_types() {
        assert_eq!(
            cpp_standard_tuple_element_type("std::tuple<Counter, Wrapper<Alias, Tag>, int>", 1),
            Some("Wrapper<Alias, Tag>")
        );
        assert_eq!(
            cpp_standard_tuple_element_type("std::pair<const Counter, Value>", 0),
            Some("const Counter")
        );
        assert_eq!(
            cpp_standard_tuple_element_type("std::tuple<Value, Counter*>", 1),
            Some("Counter*")
        );
        assert_eq!(
            cpp_standard_tuple_element_type("std::pair<Counter, Value>", 2),
            None
        );
        assert_eq!(
            cpp_standard_tuple_element_type("std::vector<Counter>", 0),
            None
        );
    }
}
