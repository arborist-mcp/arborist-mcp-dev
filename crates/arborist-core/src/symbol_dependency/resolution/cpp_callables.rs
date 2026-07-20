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
    let Some((_, mut suffix)) = symbol
        .signature
        .as_deref()
        .and_then(|signature| signature.rsplit_once(')'))
    else {
        return false;
    };
    loop {
        suffix = suffix.trim_start();
        if suffix.starts_with("const") {
            return true;
        }
        if let Some(remaining) = suffix.strip_prefix("volatile") {
            suffix = remaining;
        } else {
            return false;
        }
    }
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    fn cpp_callable(parameters: &[&str]) -> IndexedSymbol {
        IndexedSymbol {
            symbol_id: "api::convert".to_string(),
            base_name: "convert".to_string(),
            semantic_path: "api::convert".to_string(),
            scope_path: Some("api".to_string()),
            file_path: "api.cpp".to_string(),
            node_kind: "function_definition".to_string(),
            byte_range: (0, 0),
            signature: None,
            parameters: parameters
                .iter()
                .map(|parameter| (*parameter).to_string())
                .collect(),
            return_type: None,
            docstring: None,
            references_by_name: BTreeSet::new(),
            call_arities_by_name: BTreeMap::new(),
        }
    }

    #[test]
    fn cpp_callable_arity_allows_defaulted_parameters() {
        let callable = cpp_callable(&["int value", "int radix = 10"]);

        assert!(cpp_callable_accepts_arity(&callable, 1));
        assert!(cpp_callable_accepts_arity(&callable, 2));
        assert!(!cpp_callable_accepts_arity(&callable, 0));
        assert!(!cpp_callable_accepts_arity(&callable, 3));
    }

    #[test]
    fn cpp_callable_arity_allows_variadic_arguments() {
        let callable = cpp_callable(&["int first", "..."]);

        assert!(!cpp_callable_accepts_arity(&callable, 0));
        assert!(cpp_callable_accepts_arity(&callable, 1));
        assert!(cpp_callable_accepts_arity(&callable, 4));
    }

    #[test]
    fn cpp_callable_arity_does_not_treat_parameter_packs_as_variadic_calls() {
        let callable = cpp_callable(&["Args... values"]);

        assert!(cpp_callable_accepts_arity(&callable, 1));
        assert!(!cpp_callable_accepts_arity(&callable, 2));
    }

    #[test]
    fn cpp_const_qualification_comes_after_the_parameter_list() {
        let mut const_member = cpp_callable(&[]);
        const_member.signature = Some("const int convert() const;".to_string());
        let mut const_return = cpp_callable(&[]);
        const_return.signature = Some("const int convert();".to_string());
        let mut volatile_const_member = cpp_callable(&[]);
        volatile_const_member.signature = Some("int convert() volatile const &;".to_string());

        assert!(cpp_callable_is_const_qualified(&const_member));
        assert!(!cpp_callable_is_const_qualified(&const_return));
        assert!(cpp_callable_is_const_qualified(&volatile_const_member));
    }
}
