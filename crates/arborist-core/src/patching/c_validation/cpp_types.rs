#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CppThisMemberReceiver {
    Lvalue,
    ConstLvalue,
    Rvalue,
    ConstRvalue,
}

pub(super) fn cpp_temporary_type_path(type_name: &str) -> Option<String> {
    let type_name = type_name.trim_end().trim_end_matches('&').trim_end();
    let path = type_name
        .split_whitespace()
        .filter(|part| !matches!(*part, "const" | "volatile" | "&" | "&&"))
        .collect::<String>();
    (!path.is_empty() && !path.contains('*')).then(|| path.to_string())
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
