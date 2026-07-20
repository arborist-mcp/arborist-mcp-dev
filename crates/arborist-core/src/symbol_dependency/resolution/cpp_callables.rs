use std::path::Path;

use crate::language::detect_language;
use crate::model::LanguageId;
use crate::symbol_index_model::IndexedSymbol;

pub(super) fn is_cpp_callable(symbol: &IndexedSymbol) -> bool {
    detect_language(Path::new(&symbol.file_path)).ok() == Some(LanguageId::Cpp)
        && matches!(
            symbol.node_kind.as_str(),
            "function_definition" | "declaration" | "field_declaration"
        )
}

pub(super) fn cpp_const_member_candidates(
    candidates: Vec<usize>,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    const_this_receiver: bool,
    explicit_member_receiver: bool,
) -> Vec<usize> {
    let prefer_const_members = if explicit_member_receiver {
        const_this_receiver
    } else {
        const_this_receiver || cpp_callable_is_const_qualified(source_symbol)
    };
    let compatible_members = candidates
        .iter()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            (explicit_member_receiver
                || (source_symbol.scope_path.is_some()
                    && source_symbol.scope_path == candidate.scope_path))
                && is_cpp_callable(candidate)
                && cpp_callable_is_const_qualified(candidate) == prefer_const_members
        })
        .collect::<Vec<_>>();

    if compatible_members.is_empty() {
        candidates
    } else {
        compatible_members
    }
}

pub(super) fn cpp_callable_is_const_qualified(symbol: &IndexedSymbol) -> bool {
    symbol.signature.as_deref().is_some_and(|signature| {
        signature
            .rsplit_once(')')
            .is_some_and(|(_, suffix)| suffix.trim_start().starts_with("const"))
    })
}

pub(super) fn cpp_lvalue_member_candidates(
    candidates: Vec<usize>,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    explicit_member_receiver: bool,
) -> Vec<usize> {
    let lvalue_members = candidates
        .iter()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            (explicit_member_receiver
                || (source_symbol.scope_path.is_some()
                    && source_symbol.scope_path == candidate.scope_path))
                && is_cpp_callable(candidate)
                && cpp_callable_ref_qualifier(candidate) == Some("&")
        })
        .collect::<Vec<_>>();

    if lvalue_members.is_empty() {
        candidates
    } else {
        lvalue_members
    }
}

pub(super) fn cpp_rvalue_member_candidates(
    candidates: Vec<usize>,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    explicit_member_receiver: bool,
) -> Vec<usize> {
    let rvalue_members = candidates
        .iter()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            (explicit_member_receiver
                || (source_symbol.scope_path.is_some()
                    && source_symbol.scope_path == candidate.scope_path))
                && is_cpp_callable(candidate)
                && cpp_callable_ref_qualifier(candidate) == Some("&&")
        })
        .collect::<Vec<_>>();

    if rvalue_members.is_empty() {
        candidates
    } else {
        rvalue_members
    }
}

fn cpp_callable_ref_qualifier(symbol: &IndexedSymbol) -> Option<&'static str> {
    let (_, mut suffix) = symbol.signature.as_deref()?.rsplit_once(')')?;
    loop {
        suffix = suffix.trim_start();
        if let Some(remaining) = suffix.strip_prefix("const") {
            suffix = remaining;
        } else if let Some(remaining) = suffix.strip_prefix("volatile") {
            suffix = remaining;
        } else {
            break;
        }
    }
    let suffix = suffix.trim_start();
    if suffix.starts_with("&&") {
        Some("&&")
    } else if suffix.starts_with('&') {
        Some("&")
    } else {
        None
    }
}

pub(super) fn cpp_callable_accepts_arity(symbol: &IndexedSymbol, call_arity: usize) -> bool {
    let parameters = if symbol.parameters.len() == 1 && symbol.parameters[0].trim() == "void" {
        &[]
    } else {
        symbol.parameters.as_slice()
    };
    let variadic = parameters
        .last()
        .is_some_and(|parameter| parameter.trim() == "...");
    let fixed_parameters = if variadic {
        &parameters[..parameters.len().saturating_sub(1)]
    } else {
        parameters
    };
    let required_parameters = fixed_parameters
        .iter()
        .filter(|parameter| !cpp_parameter_has_default(parameter))
        .count();

    call_arity >= required_parameters && (variadic || call_arity <= fixed_parameters.len())
}

fn cpp_parameter_has_default(parameter: &str) -> bool {
    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;

    for character in parameter.chars() {
        match character {
            '(' => parentheses += 1,
            ')' => parentheses = parentheses.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            '=' if parentheses == 0 && brackets == 0 && braces == 0 => return true,
            _ => {}
        }
    }
    false
}
