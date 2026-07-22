#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CppThisMemberReceiver {
    Lvalue,
    ConstLvalue,
    Rvalue,
    ConstRvalue,
}

pub(super) fn cpp_temporary_type_path(type_name: &str) -> Option<String> {
    let type_name = type_name.trim_end().trim_end_matches('&').trim_end();
    let path = cpp_strip_top_level_cv_qualifiers(type_name);
    (!path.is_empty() && !cpp_type_has_top_level_pointer(&path)).then_some(path)
}

fn cpp_strip_top_level_cv_qualifiers(type_name: &str) -> String {
    let mut template_depth = 0usize;
    let mut path = String::with_capacity(type_name.len());
    let mut characters = type_name.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        match character {
            '<' => {
                template_depth += 1;
                path.push(character);
            }
            '>' => {
                template_depth = template_depth.saturating_sub(1);
                path.push(character);
            }
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
                let word = &type_name[index..end];
                if template_depth != 0 || !matches!(word, "const" | "volatile") {
                    path.push_str(word);
                }
            }
            _ => path.push(character),
        }
    }
    path.trim().to_string()
}

fn cpp_type_has_top_level_pointer(type_name: &str) -> bool {
    let mut template_depth = 0usize;
    for character in type_name.chars() {
        match character {
            '<' => template_depth += 1,
            '>' => template_depth = template_depth.saturating_sub(1),
            '*' if template_depth == 0 => return true,
            _ => {}
        }
    }
    false
}

pub(super) fn cpp_pointer_target_path(type_name: &str) -> Option<String> {
    cpp_temporary_type_path(type_name.split_once('*')?.0)
}

pub(super) fn cpp_this_receiver_for_type(
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

pub(super) fn cpp_type_is_top_level_const(type_name: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::cpp_temporary_type_path;

    #[test]
    fn preserves_pointers_inside_template_arguments() {
        assert_eq!(
            cpp_temporary_type_path("std::tuple<Value, Counter*>"),
            Some("std::tuple<Value, Counter*>".to_string())
        );
        assert_eq!(
            cpp_temporary_type_path("const std::expected<Value, const Counter>&"),
            Some("std::expected<Value, const Counter>".to_string())
        );
        assert!(cpp_temporary_type_path("Counter*").is_none());
    }
}
