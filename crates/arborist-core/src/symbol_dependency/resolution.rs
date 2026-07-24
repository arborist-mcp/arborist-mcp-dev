mod cpp_callables;
mod path_groups;
mod python;
mod type_alias;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use super::c::{CIncludeContext, c_include_context_for_file, c_symbol_family_anchor};
use crate::language::{detect_language, is_c_header_path};
use crate::model::{LanguageId, SymbolMeta, SymbolMetaInit};
use crate::patching::resolve_local_python_imported_symbol;
use crate::semantic::cpp_callable_symbol_id;
use crate::symbol_index_model::{
    CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX, CPP_CONST_LVALUE_THIS_CALL_PREFIX,
    CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_CONST_RVALUE_THIS_CALL_PREFIX, CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_RVALUE_THIS_CALL_PREFIX, CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_TEMPORARY_MEMBER_CALL_SEPARATOR, IndexedSymbol, symbol_kind_rank,
};

use cpp_callables::{
    cpp_callable_accepts_arity, cpp_const_member_candidates, cpp_lvalue_member_candidates,
    cpp_rvalue_member_candidates, is_cpp_callable,
};
use path_groups::{cpp_qualified_reference_path_groups, cpp_unqualified_call_candidate_groups};
use python::{python_reference_lookup, python_symbol_matches_module_hint};
use type_alias::{
    cpp_constructor_path, cpp_type_alias_member_candidates, cpp_type_alias_target_indexes,
    is_cpp_constructible_type,
};

#[derive(Clone, Copy)]
struct CallResolutionContext {
    arity: Option<usize>,
    rvalue_this_receiver: bool,
    const_this_receiver: bool,
    explicit_member_receiver: bool,
}

impl CallResolutionContext {
    fn cpp(
        arity: usize,
        rvalue_this_receiver: bool,
        const_this_receiver: bool,
        explicit_member_receiver: bool,
    ) -> Self {
        Self {
            arity: Some(arity),
            rvalue_this_receiver,
            const_this_receiver,
            explicit_member_receiver,
        }
    }

    fn non_call() -> Self {
        Self {
            arity: None,
            rvalue_this_receiver: false,
            const_this_receiver: false,
            explicit_member_receiver: false,
        }
    }
}

pub(crate) fn assign_symbol_ids(raw_symbols: &mut [IndexedSymbol]) -> Result<()> {
    let symbol_ids = (0..raw_symbols.len())
        .map(|index| symbol_id_for_index(index, raw_symbols))
        .collect::<Result<Vec<_>>>()?;

    for (symbol, symbol_id) in raw_symbols.iter_mut().zip(symbol_ids) {
        symbol.symbol_id = symbol_id;
    }

    Ok(())
}

pub(crate) fn resolve_symbol_dependencies(raw_symbols: &[IndexedSymbol]) -> Vec<SymbolMeta> {
    resolve_symbol_dependencies_with_overrides(raw_symbols, None)
}

pub(crate) fn resolve_symbol_dependencies_with_overrides(
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Vec<SymbolMeta> {
    let name_index = build_name_index(raw_symbols);
    let semantic_path_index = build_semantic_path_index(raw_symbols);
    let symbol_indexes = raw_symbol_indexes_by_id(raw_symbols);
    let mut dependency_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for (symbol_id, indexes) in &symbol_indexes {
        let dependencies = dependency_map.entry(symbol_id.clone()).or_default();
        for index in indexes {
            dependencies.extend(resolve_dependencies_for_symbol(
                &raw_symbols[*index],
                raw_symbols,
                &name_index,
                &semantic_path_index,
                file_overrides,
            ));
        }
    }

    let mut reference_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (caller, callees) in &dependency_map {
        for callee in callees {
            reference_map
                .entry(callee.clone())
                .or_default()
                .insert(caller.clone());
        }
    }

    raw_symbols
        .iter()
        .map(|symbol| {
            SymbolMeta::new(SymbolMetaInit {
                symbol_id: symbol.symbol_id.clone(),
                semantic_path: symbol.semantic_path.clone(),
                scope_path: symbol.scope_path.clone(),
                file_path: symbol.file_path.clone(),
                node_kind: symbol.node_kind.clone(),
                origin_type: "workspace_symbol".to_string(),
                byte_range: symbol.byte_range,
                signature: symbol.signature.clone(),
                parameters: symbol.parameters.clone(),
                return_type: symbol.return_type.clone(),
                docstring: symbol.docstring.clone(),
                dependencies: dependency_map
                    .get(&symbol.symbol_id)
                    .map(|dependencies| dependencies.iter().cloned().collect())
                    .unwrap_or_default(),
                references: reference_map
                    .get(&symbol.symbol_id)
                    .map(|references| references.iter().cloned().collect())
                    .unwrap_or_default(),
            })
        })
        .collect()
}

pub(super) fn build_name_index(raw_symbols: &[IndexedSymbol]) -> BTreeMap<String, Vec<usize>> {
    let mut name_index = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        name_index
            .entry(symbol.base_name.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    name_index
}

pub(super) fn build_semantic_path_index(
    raw_symbols: &[IndexedSymbol],
) -> BTreeMap<String, Vec<usize>> {
    let mut semantic_path_index = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        semantic_path_index
            .entry(symbol.semantic_path.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    semantic_path_index
}

pub(super) fn raw_symbol_indexes_by_id(
    raw_symbols: &[IndexedSymbol],
) -> BTreeMap<String, Vec<usize>> {
    let mut indexes = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        indexes
            .entry(symbol.symbol_id.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    indexes
}

pub(super) fn resolve_dependencies_for_symbol(
    symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Vec<String> {
    let mut dependencies = BTreeSet::new();
    for encoded_reference_name in &symbol.references_by_name {
        let (reference_name, rvalue_this_receiver, const_this_receiver, explicit_member_receiver) =
            encoded_reference_name
                .strip_prefix(CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX)
                .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                .map(|(_, name)| (name, false, false, true))
                .or_else(|| {
                    encoded_reference_name
                        .strip_prefix(CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX)
                        .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                        .map(|(_, name)| (name, false, true, true))
                })
                .or_else(|| {
                    encoded_reference_name
                        .strip_prefix(CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX)
                        .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                        .map(|(_, name)| (name, true, false, true))
                })
                .or_else(|| {
                    encoded_reference_name
                        .strip_prefix(CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX)
                        .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                        .map(|(_, name)| (name, true, true, true))
                })
                .or_else(|| {
                    encoded_reference_name
                        .strip_prefix(CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX)
                        .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                        .map(|(_, name)| (name, true, false, true))
                })
                .or_else(|| {
                    encoded_reference_name
                        .strip_prefix(CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX)
                        .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                        .map(|(_, name)| (name, true, true, true))
                })
                .or_else(|| {
                    encoded_reference_name
                        .strip_prefix(CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX)
                        .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                        .map(|(_, name)| (name, false, true, true))
                })
                .or_else(|| {
                    encoded_reference_name
                        .strip_prefix(CPP_CONST_RVALUE_THIS_CALL_PREFIX)
                        .map(|name| (name, true, true, true))
                })
                .or_else(|| {
                    encoded_reference_name
                        .strip_prefix(CPP_CONST_LVALUE_THIS_CALL_PREFIX)
                        .map(|name| (name, false, true, true))
                })
                .or_else(|| {
                    encoded_reference_name
                        .strip_prefix(CPP_RVALUE_THIS_CALL_PREFIX)
                        .map(|name| (name, true, false, true))
                })
                .unwrap_or((encoded_reference_name.as_str(), false, false, false));
        let call_arities = symbol.call_arities_by_name.get(encoded_reference_name);
        if detect_language(Path::new(&symbol.file_path)).ok() == Some(LanguageId::Cpp)
            && let Some(call_arities) = call_arities
        {
            for call_arity in call_arities {
                if let Some(target_symbol_id) = resolve_reference_path(
                    reference_name,
                    CallResolutionContext::cpp(
                        *call_arity,
                        rvalue_this_receiver,
                        const_this_receiver,
                        explicit_member_receiver,
                    ),
                    symbol,
                    raw_symbols,
                    name_index,
                    semantic_path_index,
                    file_overrides,
                ) && target_symbol_id != symbol.symbol_id
                {
                    dependencies.insert(target_symbol_id);
                }
            }
        } else if let Some(target_symbol_id) = resolve_reference_path(
            reference_name,
            CallResolutionContext::non_call(),
            symbol,
            raw_symbols,
            name_index,
            semantic_path_index,
            file_overrides,
        ) && target_symbol_id != symbol.symbol_id
        {
            dependencies.insert(target_symbol_id);
        }
    }
    dependencies.into_iter().collect()
}

pub(super) fn indexed_symbol_rank(symbol: &IndexedSymbol) -> usize {
    symbol_kind_rank(&symbol.node_kind)
}

fn symbol_id_for_index(index: usize, raw_symbols: &[IndexedSymbol]) -> Result<String> {
    let symbol = &raw_symbols[index];
    let path = Path::new(&symbol.file_path);
    if detect_language(path).ok() == Some(LanguageId::Cpp)
        && matches!(
            symbol.node_kind.as_str(),
            "function_definition" | "declaration" | "field_declaration"
        )
    {
        return Ok(cpp_callable_symbol_id(
            &symbol.semantic_path,
            &symbol.parameters,
            symbol.signature.as_deref(),
        ));
    }
    if !matches!(
        detect_language(path).ok(),
        Some(LanguageId::C | LanguageId::Cpp)
    ) || symbol.semantic_path.contains("::")
    {
        return Ok(symbol.semantic_path.clone());
    }

    let anchor = if is_c_header_path(path) {
        symbol.file_path.clone()
    } else {
        c_symbol_family_anchor(symbol, raw_symbols)?
    };

    Ok(format!("{anchor}::{}", symbol.base_name))
}

fn resolve_reference_path(
    reference_name: &str,
    call_context: CallResolutionContext,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Option<String> {
    let call_arity = call_context.arity;
    let language_id = detect_language(Path::new(&source_symbol.file_path)).ok();
    let (lookup_name, module_hint) = if language_id == Some(LanguageId::Python) {
        python_reference_lookup(reference_name)
    } else {
        (reference_name, None)
    };
    let qualified_cpp_reference =
        language_id == Some(LanguageId::Cpp) && lookup_name.contains("::");
    let scoped_cpp_direct_call =
        language_id == Some(LanguageId::Cpp) && call_arity.is_some() && !qualified_cpp_reference;
    let (candidates, scoped_cpp_candidates) = if qualified_cpp_reference {
        cpp_qualified_reference_path_groups(lookup_name, source_symbol, raw_symbols, file_overrides)
            .into_iter()
            .find_map(|qualified_paths| {
                let candidates = symbol_indexes_for_paths_with_template_fallback(
                    &qualified_paths,
                    semantic_path_index,
                );
                (!candidates.is_empty()).then_some(candidates)
            })
            .or_else(|| {
                cpp_type_alias_member_candidates(
                    lookup_name,
                    source_symbol,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                )
            })
            .map(|candidates| (candidates, false))
            .unwrap_or_default()
    } else if scoped_cpp_direct_call {
        let scoped_candidates = cpp_unqualified_call_candidate_groups(
            lookup_name,
            source_symbol,
            raw_symbols,
            file_overrides,
        )
        .into_iter()
        .find_map(|paths| {
            let candidates =
                symbol_indexes_for_paths_with_template_fallback(&paths, semantic_path_index);
            (!candidates.is_empty()).then_some(candidates)
        });
        match scoped_candidates {
            Some(candidates) => (candidates, true),
            None => (
                name_index.get(lookup_name).cloned().unwrap_or_default(),
                false,
            ),
        }
    } else {
        (name_index.get(lookup_name)?.clone(), false)
    };
    if candidates.is_empty() {
        return None;
    }
    let visible_candidates = if qualified_cpp_reference || scoped_cpp_candidates {
        candidates.clone()
    } else {
        candidates
            .iter()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.file_path == source_symbol.file_path
                    || !candidate.semantic_path.contains("::")
            })
            .collect()
    };
    let candidate_slice = if visible_candidates.is_empty() {
        candidates
    } else {
        visible_candidates
    };
    let hinted_candidates = if let Some(module_hint) = module_hint {
        let imported_summary = resolve_local_python_imported_symbol(
            Path::new(&source_symbol.file_path),
            module_hint,
            lookup_name,
        )
        .ok()
        .flatten();
        let class_method_path = format!("{module_hint}.{lookup_name}");
        let filtered = candidate_slice
            .iter()
            .copied()
            .filter(|index| {
                raw_symbols[*index].semantic_path == class_method_path
                    || python_symbol_matches_module_hint(
                        source_symbol,
                        &raw_symbols[*index],
                        module_hint,
                        imported_summary.as_ref(),
                    )
            })
            .collect::<Vec<_>>();
        if filtered.is_empty() {
            candidate_slice.clone()
        } else {
            filtered
        }
    } else {
        candidate_slice
    };
    let arity_candidates = if let Some(call_arity) = call_arity {
        let type_alias_candidates = cpp_type_alias_target_indexes(
            &hinted_candidates,
            source_symbol,
            raw_symbols,
            semantic_path_index,
            file_overrides,
        );
        let callable_candidates = hinted_candidates
            .iter()
            .copied()
            .filter(|index| is_cpp_callable(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        let constructible_candidates = hinted_candidates
            .iter()
            .copied()
            .chain(type_alias_candidates)
            .filter(|index| is_cpp_constructible_type(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if callable_candidates.is_empty() && !constructible_candidates.is_empty() {
            let constructor_paths = constructible_candidates
                .into_iter()
                .filter_map(|index| cpp_constructor_path(&raw_symbols[index].semantic_path))
                .collect::<Vec<_>>();
            symbol_indexes_for_paths(&constructor_paths, semantic_path_index)
                .into_iter()
                .filter(|index| {
                    is_cpp_callable(&raw_symbols[*index])
                        && cpp_callable_accepts_arity(&raw_symbols[*index], call_arity)
                })
                .collect()
        } else if callable_candidates.is_empty() {
            hinted_candidates
                .into_iter()
                .filter(|index| {
                    !matches!(
                        raw_symbols[*index].node_kind.as_str(),
                        "alias_declaration" | "type_definition" | "using_declaration"
                    )
                })
                .collect()
        } else {
            callable_candidates
                .into_iter()
                .filter(|index| cpp_callable_accepts_arity(&raw_symbols[*index], call_arity))
                .collect()
        }
    } else {
        hinted_candidates
    };
    let arity_candidates = if call_context.rvalue_this_receiver {
        cpp_rvalue_member_candidates(
            arity_candidates,
            source_symbol,
            raw_symbols,
            call_context.explicit_member_receiver,
        )
    } else {
        cpp_lvalue_member_candidates(
            arity_candidates,
            source_symbol,
            raw_symbols,
            call_context.explicit_member_receiver,
        )
    };
    let arity_candidates = cpp_const_member_candidates(
        arity_candidates,
        source_symbol,
        raw_symbols,
        call_context.const_this_receiver,
        call_context.explicit_member_receiver,
    );
    let include_context = c_include_context_for_file(&source_symbol.file_path).ok();

    arity_candidates
        .iter()
        .copied()
        .max_by_key(|index| {
            indexed_symbol_candidate_rank(
                &raw_symbols[*index],
                source_symbol,
                Some(&source_symbol.file_path),
                include_context.as_ref(),
            )
        })
        .map(|index| raw_symbols[index].symbol_id.clone())
}

pub(super) fn symbol_indexes_for_paths(
    paths: &[String],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Vec<usize> {
    paths
        .iter()
        .flat_map(|path| semantic_path_index.get(path).into_iter().flatten().copied())
        .collect()
}

pub(super) fn symbol_indexes_for_paths_with_template_fallback(
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

pub(super) fn cpp_template_base_path(path: &str) -> Option<String> {
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

pub(super) fn cpp_template_argument_closes(remaining: &[char]) -> bool {
    matches!(
        remaining
            .iter()
            .copied()
            .find(|character| !character.is_whitespace()),
        None | Some('>' | ',' | ')' | ']' | '}' | ':' | '.')
    )
}

fn indexed_symbol_candidate_rank(
    symbol: &IndexedSymbol,
    source_symbol: &IndexedSymbol,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> usize {
    let mut rank = indexed_symbol_rank(symbol);

    if let Some(context_file) = context_file {
        if symbol.file_path == context_file {
            rank += 1000;
        } else if symbol.semantic_path.contains("::") {
            rank = rank.saturating_sub(100);
        }
    }

    if source_symbol_scope_matches(source_symbol, symbol) {
        rank += 500;
    }

    if let Some(include_context) = include_context {
        if include_context.include_paths.contains(&symbol.file_path) {
            rank += 200;
        }
        if include_context
            .companion_source_paths
            .contains(&symbol.file_path)
        {
            rank += 300;
        }
    }

    rank
}

fn source_symbol_scope_matches(source_symbol: &IndexedSymbol, candidate: &IndexedSymbol) -> bool {
    detect_language(Path::new(&source_symbol.file_path)).ok() == Some(LanguageId::Cpp)
        && source_symbol.scope_path.is_some()
        && source_symbol.scope_path == candidate.scope_path
}

#[cfg(test)]
mod tests {
    use super::*;

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
