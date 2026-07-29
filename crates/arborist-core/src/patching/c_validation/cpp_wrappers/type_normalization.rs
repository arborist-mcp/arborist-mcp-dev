fn compact_cpp_type_text(type_name: &str) -> String {
    type_name
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

pub(super) fn normalized_cpp_typed_get_type(type_name: &str) -> String {
    if type_name.contains('&') {
        return compact_cpp_type_text(type_name);
    }
    if let Some(normalized) = normalized_cpp_single_pointer_type(type_name) {
        return normalized;
    }
    if cpp_type_has_top_level_pointer(type_name) {
        return compact_cpp_type_text(type_name);
    }
    normalized_cpp_non_pointer_type(type_name)
}

fn normalized_cpp_single_pointer_type(type_name: &str) -> Option<String> {
    let mut template_depth = 0usize;
    let mut pointer_index = None;
    for (index, character) in type_name.char_indices() {
        match character {
            '<' => template_depth += 1,
            '>' => template_depth = template_depth.saturating_sub(1),
            '*' if template_depth == 0 => {
                if pointer_index.is_some() {
                    return None;
                }
                pointer_index = Some(index);
            }
            _ => {}
        }
    }
    let pointer_index = pointer_index?;
    let pointee = type_name[..pointer_index].trim();
    let pointer_suffix = type_name[pointer_index + '*'.len_utf8()..].trim();
    if pointee.is_empty() || cpp_type_has_top_level_pointer(pointee) {
        return None;
    }
    if pointer_suffix
        .split_whitespace()
        .any(|token| token != "const" && token != "volatile")
    {
        return None;
    }

    let mut normalized = normalized_cpp_non_pointer_type(pointee);
    normalized.push('*');
    if pointer_suffix
        .split_whitespace()
        .any(|token| token == "const")
    {
        normalized.push_str("#const");
    }
    if pointer_suffix
        .split_whitespace()
        .any(|token| token == "volatile")
    {
        normalized.push_str("#volatile");
    }
    Some(normalized)
}

fn normalized_cpp_non_pointer_type(type_name: &str) -> String {
    let mut template_depth = 0usize;
    let mut normalized = String::with_capacity(type_name.len());
    let mut const_qualified = false;
    let mut volatile_qualified = false;
    let mut characters = type_name.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        if character == '<' {
            template_depth += 1;
            normalized.push(character);
            continue;
        }
        if character == '>' {
            template_depth = template_depth.saturating_sub(1);
            normalized.push(character);
            continue;
        }
        if template_depth == 0 && (character.is_ascii_alphabetic() || character == '_') {
            let mut end = index + character.len_utf8();
            while let Some((next_index, next)) = characters.peek().copied() {
                if next.is_ascii_alphanumeric() || next == '_' {
                    end = next_index + next.len_utf8();
                    characters.next();
                } else {
                    break;
                }
            }
            match &type_name[index..end] {
                "const" => const_qualified = true,
                "volatile" => volatile_qualified = true,
                _ => normalized.push_str(&type_name[index..end]),
            }
            continue;
        }
        if !character.is_whitespace() {
            normalized.push(character);
        }
    }
    if const_qualified {
        normalized.push_str("#const");
    }
    if volatile_qualified {
        normalized.push_str("#volatile");
    }
    normalized
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
