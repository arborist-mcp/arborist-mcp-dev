mod cpp_callables;
mod indexes;
mod path_groups;
mod python;
mod ranking;
mod symbol_ids;
mod template_paths;
mod type_alias;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use super::c::c_include_context_for_file;
use crate::language::detect_language;
use crate::model::{LanguageId, SymbolMeta, SymbolMetaInit};
use crate::patching::resolve_local_python_imported_symbol;
use crate::symbol_index_model::{
    CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX, CPP_CONST_LVALUE_THIS_CALL_PREFIX,
    CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_CONST_RVALUE_THIS_CALL_PREFIX, CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_RVALUE_THIS_CALL_PREFIX, CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_TEMPORARY_MEMBER_CALL_SEPARATOR, IndexedSymbol,
};
use crate::workspace_scan::WorkspaceScanDeadline;

use cpp_callables::{
    cpp_callable_accepts_arity, cpp_const_member_candidates, cpp_lvalue_member_candidates,
    cpp_rvalue_member_candidates, is_cpp_callable,
};
pub(super) use indexes::{build_name_index, build_semantic_path_index, raw_symbol_indexes_by_id};
use path_groups::{cpp_qualified_reference_path_groups, cpp_unqualified_call_candidate_groups};
use python::{python_reference_lookup, python_symbol_matches_module_hint};
pub(super) use ranking::{indexed_symbol_candidate_rank, indexed_symbol_rank};
pub(super) use template_paths::{
    cpp_template_argument_closes, cpp_template_base_path, symbol_indexes_for_paths,
    symbol_indexes_for_paths_with_template_fallback,
};
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
    symbol_ids::assign_symbol_ids(raw_symbols)
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

pub(crate) fn resolve_symbol_dependencies_with_overrides_with_deadline(
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: &WorkspaceScanDeadline,
) -> Result<Vec<SymbolMeta>> {
    deadline.check("resolving symbol dependencies")?;
    let name_index = build_name_index(raw_symbols);
    let semantic_path_index = build_semantic_path_index(raw_symbols);
    let symbol_indexes = raw_symbol_indexes_by_id(raw_symbols);
    let mut dependency_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for (symbol_id, indexes) in &symbol_indexes {
        deadline.check("resolving symbol dependencies")?;
        let dependencies = dependency_map.entry(symbol_id.clone()).or_default();
        for index in indexes {
            dependencies.extend(resolve_dependencies_for_symbol_with_deadline(
                &raw_symbols[*index],
                raw_symbols,
                &name_index,
                &semantic_path_index,
                file_overrides,
                Some(deadline),
            )?);
            deadline.check("resolving symbol dependencies")?;
        }
    }

    let mut reference_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (caller, callees) in &dependency_map {
        deadline.check("building symbol references")?;
        for callee in callees {
            reference_map
                .entry(callee.clone())
                .or_default()
                .insert(caller.clone());
        }
    }

    let mut resolved_symbols = Vec::with_capacity(raw_symbols.len());
    for symbol in raw_symbols {
        deadline.check("materializing resolved symbols")?;
        resolved_symbols.push(SymbolMeta::new(SymbolMetaInit {
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
        }));
    }

    Ok(resolved_symbols)
}

pub(super) fn resolve_dependencies_for_symbol(
    symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Vec<String> {
    resolve_dependencies_for_symbol_with_deadline(
        symbol,
        raw_symbols,
        name_index,
        semantic_path_index,
        file_overrides,
        None,
    )
    .expect("dependency resolution without a deadline cannot fail")
}

pub(super) fn resolve_dependencies_for_symbol_with_deadline(
    symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<String>> {
    let mut dependencies = BTreeSet::new();
    for encoded_reference_name in &symbol.references_by_name {
        if let Some(deadline) = deadline {
            deadline.check("resolving symbol references")?;
        }
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
                if let Some(deadline) = deadline {
                    deadline.check("resolving symbol call arities")?;
                }
                if let Some(target_symbol_id) = resolve_reference_path_with_deadline(
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
                    deadline,
                )? && target_symbol_id != symbol.symbol_id
                {
                    dependencies.insert(target_symbol_id);
                }
            }
        } else if let Some(target_symbol_id) = resolve_reference_path_with_deadline(
            reference_name,
            CallResolutionContext::non_call(),
            symbol,
            raw_symbols,
            name_index,
            semantic_path_index,
            file_overrides,
            deadline,
        )? && target_symbol_id != symbol.symbol_id
        {
            dependencies.insert(target_symbol_id);
        }
    }
    if let Some(deadline) = deadline {
        deadline.check("resolving symbol references")?;
    }
    Ok(dependencies.into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
fn resolve_reference_path_with_deadline(
    reference_name: &str,
    call_context: CallResolutionContext,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if let Some(deadline) = deadline {
        deadline.check("resolving reference candidates")?;
    }
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
        let Some(candidates) = name_index.get(lookup_name) else {
            return Ok(None);
        };
        (candidates.clone(), false)
    };
    if candidates.is_empty() {
        return Ok(None);
    }
    if let Some(deadline) = deadline {
        deadline.check("filtering reference candidates")?;
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
    if let Some(deadline) = deadline {
        deadline.check("filtering reference candidates")?;
    }
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
    if let Some(deadline) = deadline {
        deadline.check("ranking reference candidates")?;
    }
    let include_context = c_include_context_for_file(&source_symbol.file_path).ok();

    let mut selected_index = None;
    let mut selected_rank = 0;
    for index in arity_candidates {
        if let Some(deadline) = deadline {
            deadline.check("ranking reference candidates")?;
        }
        let rank = indexed_symbol_candidate_rank(
            &raw_symbols[index],
            source_symbol,
            Some(&source_symbol.file_path),
            include_context.as_ref(),
        );
        if selected_index.is_none() || rank >= selected_rank {
            selected_index = Some(index);
            selected_rank = rank;
        }
    }
    let selected = selected_index.map(|index| raw_symbols[index].symbol_id.clone());
    if let Some(deadline) = deadline {
        deadline.check("ranking reference candidates")?;
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::{Duration, Instant};

    use super::{
        resolve_dependencies_for_symbol_with_deadline,
        resolve_symbol_dependencies_with_overrides_with_deadline,
    };
    use crate::symbol_index_model::IndexedSymbol;
    use crate::workspace_scan::WorkspaceScanDeadline;

    #[test]
    fn deadline_resolver_rejects_expired_empty_input() {
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = resolve_symbol_dependencies_with_overrides_with_deadline(&[], None, &deadline)
            .expect_err("expired dependency resolution should fail before indexing");
        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }

    #[test]
    fn deadline_resolver_checks_each_symbol_reference() {
        let symbol = IndexedSymbol {
            symbol_id: "caller".to_string(),
            semantic_path: "caller".to_string(),
            base_name: "caller".to_string(),
            scope_path: None,
            file_path: "caller.py".to_string(),
            node_kind: "function_definition".to_string(),
            byte_range: (0, 1),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
            references_by_name: BTreeSet::from(["callee".to_string()]),
            call_arities_by_name: BTreeMap::new(),
        };
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = resolve_dependencies_for_symbol_with_deadline(
            &symbol,
            std::slice::from_ref(&symbol),
            &BTreeMap::new(),
            &BTreeMap::new(),
            None,
            Some(&deadline),
        )
        .expect_err("expired reference resolution should fail before lookup");
        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }
}
