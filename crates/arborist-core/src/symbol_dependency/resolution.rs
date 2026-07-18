use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use anyhow::Result;

use super::c::{
    CIncludeContext, c_include_context_for_file, c_include_context_for_file_before_with_overrides,
    c_symbol_family_anchor,
};
use crate::language::{detect_language, is_c_header_path, normalize_path};
use crate::model::{LanguageId, SymbolMeta, SymbolMetaInit, SymbolSummary};
use crate::patching::{resolve_local_python_imported_symbol, resolve_local_python_module_path};
use crate::semantic::cpp_callable_symbol_id;
use crate::symbol_index_model::{IndexedSymbol, symbol_kind_rank};

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
    for reference_name in &symbol.references_by_name {
        let call_arities = symbol.call_arities_by_name.get(reference_name);
        if detect_language(Path::new(&symbol.file_path)).ok() == Some(LanguageId::Cpp)
            && let Some(call_arities) = call_arities
        {
            for call_arity in call_arities {
                if let Some(target_symbol_id) = resolve_reference_path(
                    reference_name,
                    Some(*call_arity),
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
            None,
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
    call_arity: Option<usize>,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Option<String> {
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
        cpp_qualified_reference_path_groups(lookup_name, source_symbol, raw_symbols)
            .into_iter()
            .find_map(|qualified_paths| {
                let candidates = symbol_indexes_for_paths_with_template_fallback(
                    &qualified_paths,
                    semantic_path_index,
                );
                (!candidates.is_empty()).then_some(candidates)
            })
            .map(|candidates| (candidates, false))
            .unwrap_or_default()
    } else if scoped_cpp_direct_call {
        let scoped_candidates =
            cpp_unqualified_call_candidate_groups(lookup_name, source_symbol, raw_symbols)
                .into_iter()
                .find_map(|paths| {
                    let candidates = symbol_indexes_for_paths_with_template_fallback(
                        &paths,
                        semantic_path_index,
                    );
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

fn cpp_constructor_path(type_path: &str) -> Option<String> {
    let constructor_name = type_path.rsplit("::").next()?;
    let constructor_name =
        cpp_template_base_path(constructor_name).unwrap_or_else(|| constructor_name.to_string());
    (!constructor_name.is_empty()).then(|| format!("{type_path}::{constructor_name}"))
}

fn cpp_type_alias_target_indexes(
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
            for path in cpp_qualified_reference_path_group(path, raw_symbols, Some(alias)) {
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

fn is_cpp_constructible_type(symbol: &IndexedSymbol) -> bool {
    detect_language(Path::new(&symbol.file_path)).ok() == Some(LanguageId::Cpp)
        && matches!(
            symbol.node_kind.as_str(),
            "class_specifier" | "struct_specifier" | "union_specifier"
        )
}

fn symbol_indexes_for_paths(
    paths: &[String],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Vec<usize> {
    paths
        .iter()
        .flat_map(|path| semantic_path_index.get(path).into_iter().flatten().copied())
        .collect()
}

fn symbol_indexes_for_paths_with_template_fallback(
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

fn cpp_template_argument_closes(remaining: &[char]) -> bool {
    matches!(
        remaining
            .iter()
            .copied()
            .find(|character| !character.is_whitespace()),
        None | Some('>' | ',' | ')' | ']' | '}' | ':' | '.')
    )
}

fn cpp_qualified_reference_path_groups(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
) -> Vec<Vec<String>> {
    cpp_lexical_qualified_reference_paths(reference_name, source_symbol)
        .into_iter()
        .map(|reference_path| cpp_qualified_reference_path_group(reference_path, raw_symbols, None))
        .collect()
}

fn cpp_unqualified_call_candidate_groups(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
) -> Vec<Vec<String>> {
    let scopes = source_symbol
        .scope_path
        .as_deref()
        .map(|scope_path| {
            let components = scope_path.split("::").collect::<Vec<_>>();
            (1..=components.len())
                .rev()
                .map(|length| components[..length].join("::"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![String::new()]);
    scopes
        .into_iter()
        .map(|length| {
            let scope = length;
            let scoped_reference_path = if scope.is_empty() {
                reference_name.to_string()
            } else {
                format!("{scope}::{reference_name}")
            };
            let mut paths = if scope.is_empty() {
                vec![reference_name.to_string()]
            } else {
                vec![format!("{scope}::{reference_name}")]
            };
            for directive in raw_symbols.iter().filter(|symbol| {
                symbol.node_kind == "using_declaration"
                    && if scope.is_empty() {
                        symbol.scope_path.is_none()
                    } else {
                        symbol.scope_path.as_deref() == Some(scope.as_str())
                    }
                    && symbol.file_path == source_symbol.file_path
                    && symbol.byte_range.0 < source_symbol.byte_range.0
            }) {
                let Some(target) = cpp_using_namespace_target(directive) else {
                    if directive.semantic_path != scoped_reference_path {
                        continue;
                    }
                    let Some(target) = cpp_using_declaration_target(directive) else {
                        continue;
                    };
                    paths.extend(
                        cpp_lexical_qualified_reference_paths(&target, directive)
                            .into_iter()
                            .flat_map(|path| {
                                cpp_qualified_reference_path_group(
                                    path,
                                    raw_symbols,
                                    Some(directive),
                                )
                            }),
                    );
                    continue;
                };
                paths.extend(
                    cpp_lexical_qualified_reference_paths(&target, directive)
                        .into_iter()
                        .flat_map(|path| {
                            cpp_qualified_reference_path_group(path, raw_symbols, Some(directive))
                        })
                        .map(|path| format!("{path}::{reference_name}")),
                );
            }
            paths
        })
        .collect()
}

fn cpp_qualified_reference_path_group(
    reference_path: String,
    raw_symbols: &[IndexedSymbol],
    visibility_source: Option<&IndexedSymbol>,
) -> Vec<String> {
    let mut pending = VecDeque::from([reference_path]);
    let mut paths = Vec::new();
    let mut visited = BTreeSet::new();

    while let Some(path) = pending.pop_front() {
        if !visited.insert(path.clone()) {
            continue;
        }
        paths.push(path.clone());
        for using_path in cpp_using_declaration_paths(&path, raw_symbols)
            .into_iter()
            .rev()
        {
            pending.push_front(using_path);
        }
        for alias_path in cpp_namespace_alias_paths(&path, raw_symbols, visibility_source)
            .into_iter()
            .rev()
        {
            pending.push_front(alias_path);
        }
    }

    paths
}

fn cpp_lexical_qualified_reference_paths(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
) -> Vec<String> {
    let absolute = reference_name.starts_with("::");
    let reference_name = reference_name.trim_start_matches("::");
    if absolute {
        return vec![reference_name.to_string()];
    }

    let mut paths = Vec::new();
    if let Some(scope_path) = &source_symbol.scope_path {
        let scope_components = scope_path.split("::").collect::<Vec<_>>();
        for length in (1..=scope_components.len()).rev() {
            paths.push(format!(
                "{}::{reference_name}",
                scope_components[..length].join("::")
            ));
        }
    }
    paths.push(reference_name.to_string());
    paths
}

fn cpp_namespace_alias_paths(
    reference_path: &str,
    raw_symbols: &[IndexedSymbol],
    visibility_source: Option<&IndexedSymbol>,
) -> Vec<String> {
    let components = reference_path.split("::").collect::<Vec<_>>();
    for length in (1..=components.len()).rev() {
        let alias_path = components[..length].join("::");
        let suffix = components[length..].join("::");
        let Some(alias) = raw_symbols.iter().find(|symbol| {
            symbol.node_kind == "namespace_alias_definition"
                && symbol.semantic_path == alias_path
                && visibility_source.is_none_or(|source| {
                    symbol.file_path == source.file_path
                        && symbol.byte_range.0 < source.byte_range.0
                })
        }) else {
            continue;
        };
        let Some(target) = cpp_namespace_alias_target(alias) else {
            continue;
        };

        return cpp_lexical_qualified_reference_paths(&target, alias)
            .into_iter()
            .map(|target_path| {
                if suffix.is_empty() {
                    target_path
                } else {
                    format!("{target_path}::{suffix}")
                }
            })
            .collect();
    }
    Vec::new()
}

fn cpp_using_declaration_paths(reference_path: &str, raw_symbols: &[IndexedSymbol]) -> Vec<String> {
    raw_symbols
        .iter()
        .filter(|symbol| {
            symbol.node_kind == "using_declaration" && symbol.semantic_path == reference_path
        })
        .filter_map(|declaration| {
            let target = cpp_using_declaration_target(declaration)?;
            Some(cpp_lexical_qualified_reference_paths(&target, declaration))
        })
        .flatten()
        .collect()
}

fn cpp_using_declaration_target(declaration: &IndexedSymbol) -> Option<String> {
    let declaration = declaration.signature.as_deref()?.trim();
    let target = declaration.strip_prefix("using")?.trim();
    let target = target.trim_end_matches(';').trim();
    (target.contains("::") && !target.starts_with("namespace ")).then_some(target.to_string())
}

fn cpp_using_namespace_target(declaration: &IndexedSymbol) -> Option<String> {
    let declaration = declaration.signature.as_deref()?.trim();
    let target = declaration.strip_prefix("using namespace")?.trim();
    let target = target.trim_end_matches(';').trim();
    (!target.is_empty()).then_some(target.to_string())
}

fn cpp_namespace_alias_target(alias: &IndexedSymbol) -> Option<String> {
    let declaration = alias.signature.as_deref()?;
    let (_, target) = declaration.split_once('=')?;
    let target = target.trim().trim_end_matches(';').trim();
    (!target.is_empty()).then_some(target.to_string())
}

fn cpp_type_alias_target(alias: &IndexedSymbol) -> Option<String> {
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

fn cpp_type_alias_is_visible(
    alias: &IndexedSymbol,
    source_symbol: &IndexedSymbol,
    include_context: Option<&CIncludeContext>,
) -> bool {
    cpp_is_type_alias(alias)
        && ((alias.file_path == source_symbol.file_path
            && alias.byte_range.0 < source_symbol.byte_range.0)
            || include_context
                .is_some_and(|context| context.include_paths.contains(&alias.file_path)))
}

fn cpp_is_type_alias(symbol: &IndexedSymbol) -> bool {
    matches!(
        symbol.node_kind.as_str(),
        "alias_declaration" | "type_definition"
    )
}

fn cpp_constructible_type_alias_target(target: &str) -> Option<String> {
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

fn cpp_type_alias_target_has_top_level_indirection(target: &str) -> bool {
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

fn is_cpp_callable(symbol: &IndexedSymbol) -> bool {
    detect_language(Path::new(&symbol.file_path)).ok() == Some(LanguageId::Cpp)
        && matches!(
            symbol.node_kind.as_str(),
            "function_definition" | "declaration" | "field_declaration"
        )
}

fn cpp_callable_accepts_arity(symbol: &IndexedSymbol, call_arity: usize) -> bool {
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

fn python_reference_lookup(reference_name: &str) -> (&str, Option<&str>) {
    reference_name
        .rsplit_once('.')
        .map(|(module_hint, symbol_name)| (symbol_name, Some(module_hint)))
        .unwrap_or((reference_name, None))
}

fn python_symbol_matches_module_hint(
    source_symbol: &IndexedSymbol,
    symbol: &IndexedSymbol,
    module_hint: &str,
    imported_summary: Option<&SymbolSummary>,
) -> bool {
    if let Some(imported_summary) = imported_summary {
        return imported_summary.file_path == symbol.file_path
            && imported_summary.semantic_path == symbol.semantic_path;
    }

    let Some(resolved_module_path) =
        resolve_local_python_module_path(Path::new(&source_symbol.file_path), module_hint)
    else {
        return false;
    };

    normalize_path(&resolved_module_path) == symbol.file_path
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
