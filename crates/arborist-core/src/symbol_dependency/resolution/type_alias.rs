use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use super::super::c::{CIncludeContext, c_include_context_for_file_before_with_overrides};
use crate::language::detect_language;
use crate::model::LanguageId;
use crate::symbol_index_model::IndexedSymbol;

use super::path_groups::{
    cpp_lexical_qualified_reference_paths, cpp_qualified_reference_path_group,
    cpp_qualified_reference_path_groups, cpp_symbol_is_visible_before,
};
use super::{
    cpp_template_argument_closes, cpp_template_base_path,
    symbol_indexes_for_paths_with_template_fallback,
};

pub(super) fn cpp_type_alias_member_candidates(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Option<Vec<usize>> {
    let (alias_name, member_name) = reference_name.rsplit_once("::")?;
    let alias_indexes =
        cpp_qualified_reference_path_groups(alias_name, source_symbol, raw_symbols, file_overrides)
            .into_iter()
            .flat_map(|paths| {
                symbol_indexes_for_paths_with_template_fallback(&paths, semantic_path_index)
            })
            .collect::<Vec<_>>();
    let member_paths = cpp_type_alias_target_indexes(
        &alias_indexes,
        source_symbol,
        raw_symbols,
        semantic_path_index,
        file_overrides,
    )
    .into_iter()
    .map(|index| format!("{}::{member_name}", raw_symbols[index].semantic_path))
    .collect::<Vec<_>>();
    let candidates =
        symbol_indexes_for_paths_with_template_fallback(&member_paths, semantic_path_index);
    (!candidates.is_empty()).then_some(candidates)
}

pub(super) fn cpp_constructor_path(type_path: &str) -> Option<String> {
    let constructor_name = type_path.rsplit("::").next()?;
    let constructor_name =
        cpp_template_base_path(constructor_name).unwrap_or_else(|| constructor_name.to_string());
    (!constructor_name.is_empty()).then(|| format!("{type_path}::{constructor_name}"))
}

pub(super) fn cpp_type_alias_target_indexes(
    candidates: &[usize],
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Vec<usize> {
    let include_context = c_include_context_for_file_before_with_overrides(
        &source_symbol.file_path,
        source_symbol.byte_range.0,
        file_overrides,
    )
    .ok();
    let mut pending = candidates
        .iter()
        .copied()
        .filter(|index| {
            cpp_type_alias_is_visible(
                &raw_symbols[*index],
                source_symbol,
                include_context.as_ref(),
            )
        })
        .collect::<VecDeque<_>>();
    let mut visited_aliases = BTreeSet::new();
    let mut target_indexes = BTreeSet::new();

    while let Some(alias_index) = pending.pop_front() {
        if !visited_aliases.insert(alias_index) {
            continue;
        }
        let alias = &raw_symbols[alias_index];
        let Some(target) = cpp_type_alias_target(alias) else {
            continue;
        };

        for path in cpp_lexical_qualified_reference_paths(&target, alias) {
            for path in cpp_qualified_reference_path_group(path, raw_symbols, alias, file_overrides)
            {
                for target_index in
                    symbol_indexes_for_paths_with_template_fallback(&[path], semantic_path_index)
                {
                    let target = &raw_symbols[target_index];
                    if cpp_is_type_alias(target) {
                        if cpp_type_alias_is_visible(
                            target,
                            source_symbol,
                            include_context.as_ref(),
                        ) {
                            pending.push_back(target_index);
                        }
                    } else {
                        target_indexes.insert(target_index);
                    }
                }
            }
        }
    }

    target_indexes.into_iter().collect()
}

pub(super) fn is_cpp_constructible_type(symbol: &IndexedSymbol) -> bool {
    detect_language(Path::new(&symbol.file_path)).ok() == Some(LanguageId::Cpp)
        && matches!(
            symbol.node_kind.as_str(),
            "class_specifier" | "struct_specifier" | "union_specifier"
        )
}

pub(super) fn cpp_type_alias_target(alias: &IndexedSymbol) -> Option<String> {
    let declaration = alias.signature.as_deref()?.trim();
    let target = match alias.node_kind.as_str() {
        "alias_declaration" => {
            let declaration = declaration.strip_prefix("using")?.trim();
            let (_, target) = declaration.split_once('=')?;
            let target = target.trim().trim_end_matches(';').trim();
            Some(target)
        }
        "type_definition" => {
            let declaration = declaration.strip_prefix("typedef")?.trim();
            let target = declaration.trim_end_matches(';').trim();
            let target = target.strip_suffix(&alias.base_name)?.trim();
            Some(target)
        }
        _ => None,
    }?;
    cpp_constructible_type_alias_target(target)
}

pub(super) fn cpp_type_alias_is_visible(
    alias: &IndexedSymbol,
    source_symbol: &IndexedSymbol,
    include_context: Option<&CIncludeContext>,
) -> bool {
    cpp_is_type_alias(alias) && cpp_symbol_is_visible_before(alias, source_symbol, include_context)
}

pub(super) fn cpp_is_type_alias(symbol: &IndexedSymbol) -> bool {
    matches!(
        symbol.node_kind.as_str(),
        "alias_declaration" | "type_definition"
    )
}

pub(super) fn cpp_constructible_type_alias_target(target: &str) -> Option<String> {
    let mut target = target.trim();
    while let Some(stripped) = target
        .strip_prefix("const ")
        .or_else(|| target.strip_prefix("volatile "))
        .or_else(|| target.strip_prefix("typename "))
        .or_else(|| target.strip_prefix("class "))
        .or_else(|| target.strip_prefix("struct "))
    {
        target = stripped.trim_start();
    }
    while let Some(stripped) = target
        .strip_suffix(" const")
        .or_else(|| target.strip_suffix(" volatile"))
    {
        target = stripped.trim_end();
    }

    (!target.is_empty() && !cpp_type_alias_target_has_top_level_indirection(target))
        .then_some(target.to_string())
}

pub(super) fn cpp_type_alias_target_has_top_level_indirection(target: &str) -> bool {
    let characters = target.chars().collect::<Vec<_>>();
    let mut template_depth = 0usize;
    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;

    for (index, character) in characters.iter().copied().enumerate() {
        match character {
            '<' if parentheses == 0 && brackets == 0 && braces == 0 => template_depth += 1,
            '>' if template_depth > 0
                && parentheses == 0
                && brackets == 0
                && braces == 0
                && cpp_template_argument_closes(&characters[index + 1..]) =>
            {
                template_depth -= 1;
            }
            '(' => parentheses += 1,
            ')' => parentheses = parentheses.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            '*' | '&'
                if template_depth == 0 && parentheses == 0 && brackets == 0 && braces == 0 =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}
