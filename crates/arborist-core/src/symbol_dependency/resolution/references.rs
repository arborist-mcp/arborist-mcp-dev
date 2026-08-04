use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::Result;

use super::super::c::{CIncludeContext, c_include_context_for_file_with_overrides_and_deadline};
use super::super::go::{GoImportContext, resolve_go_import_binding_for_reference};
use super::super::java::{JavaImportContext, resolve_java_import_binding_for_reference};
use super::super::javascript::{
    JavaScriptImportContext, resolve_javascript_named_import_binding_for_reference,
};
use super::cpp_callables::{
    cpp_callable_accepts_arity, cpp_const_member_candidates, cpp_lvalue_member_candidates,
    cpp_rvalue_member_candidates, is_cpp_callable,
};
use super::path_groups::{
    cpp_qualified_reference_path_groups, cpp_unqualified_call_candidate_groups,
};
use super::python::{python_reference_lookup, python_symbol_matches_module_hint};
use super::ranking::indexed_symbol_candidate_rank;
use super::template_paths::{
    symbol_indexes_for_paths, symbol_indexes_for_paths_with_template_fallback,
};
use super::type_alias::{
    cpp_constructor_path, cpp_type_alias_member_candidates, cpp_type_alias_target_indexes,
    is_cpp_constructible_type,
};
use crate::language::detect_language;
use crate::model::LanguageId;
use crate::patching::resolve_local_python_imported_symbol;
use crate::symbol_index_model::{IndexedSymbol, ReferenceLanguageDetails};
use crate::symbol_reference_compat::effective_reference_facts;
use crate::workspace_scan::WorkspaceScanDeadline;

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

    fn direct_call(arity: usize) -> Self {
        Self {
            arity: Some(arity),
            rvalue_this_receiver: false,
            const_this_receiver: false,
            explicit_member_receiver: false,
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

#[allow(clippy::too_many_arguments)]
pub(in crate::symbol_dependency) fn resolve_dependencies_for_symbol<'a>(
    symbol: &'a IndexedSymbol,
    raw_symbols: &'a [IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    languages_by_file: &mut HashMap<&'a str, Option<LanguageId>>,
    include_contexts_by_file: &mut HashMap<&'a str, Option<CIncludeContext>>,
    javascript_import_contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
) -> Vec<String> {
    resolve_dependencies_for_symbol_with_deadline(
        symbol,
        raw_symbols,
        name_index,
        semantic_path_index,
        file_overrides,
        languages_by_file,
        include_contexts_by_file,
        javascript_import_contexts_by_file,
        go_import_contexts_by_file,
        java_import_contexts_by_file,
        None,
    )
    .expect("dependency resolution without a deadline cannot fail")
}

#[allow(clippy::too_many_arguments)]
pub(in crate::symbol_dependency) fn resolve_dependencies_for_symbol_with_deadline<'a>(
    symbol: &'a IndexedSymbol,
    raw_symbols: &'a [IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    languages_by_file: &mut HashMap<&'a str, Option<LanguageId>>,
    include_contexts_by_file: &mut HashMap<&'a str, Option<CIncludeContext>>,
    javascript_import_contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<String>> {
    let mut dependencies = BTreeSet::new();
    let language_id = *languages_by_file
        .entry(symbol.file_path.as_str())
        .or_insert_with(|| detect_language(Path::new(&symbol.file_path)).ok());
    for reference in effective_reference_facts(symbol).iter() {
        if let Some(deadline) = deadline {
            deadline.check("resolving symbol references")?;
        }
        let (rvalue_this_receiver, const_this_receiver, explicit_member_receiver) =
            match &reference.language_details {
                ReferenceLanguageDetails::None => (false, false, false),
                ReferenceLanguageDetails::Cpp(details) => (
                    details.rvalue_receiver,
                    details.const_receiver,
                    details.explicit_member_receiver,
                ),
            };
        if matches!(language_id, Some(LanguageId::Cpp | LanguageId::Java))
            && let Some(call_arities) = reference.call_arities.as_ref()
        {
            for call_arity in call_arities {
                if let Some(deadline) = deadline {
                    deadline.check("resolving symbol call arities")?;
                }
                let call_context = if language_id == Some(LanguageId::Cpp) {
                    CallResolutionContext::cpp(
                        *call_arity,
                        rvalue_this_receiver,
                        const_this_receiver,
                        explicit_member_receiver,
                    )
                } else {
                    CallResolutionContext::direct_call(*call_arity)
                };
                if let Some(target_symbol_id) = resolve_reference_path_with_deadline(
                    &reference.spelling,
                    language_id,
                    call_context,
                    symbol,
                    raw_symbols,
                    name_index,
                    semantic_path_index,
                    file_overrides,
                    include_contexts_by_file,
                    javascript_import_contexts_by_file,
                    go_import_contexts_by_file,
                    java_import_contexts_by_file,
                    deadline,
                )? && target_symbol_id != symbol.symbol_id
                {
                    dependencies.insert(target_symbol_id);
                }
            }
        } else if let Some(target_symbol_id) = resolve_reference_path_with_deadline(
            &reference.spelling,
            language_id,
            CallResolutionContext::non_call(),
            symbol,
            raw_symbols,
            name_index,
            semantic_path_index,
            file_overrides,
            include_contexts_by_file,
            javascript_import_contexts_by_file,
            go_import_contexts_by_file,
            java_import_contexts_by_file,
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
fn resolve_reference_path_with_deadline<'a>(
    reference_name: &str,
    language_id: Option<LanguageId>,
    call_context: CallResolutionContext,
    source_symbol: &'a IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    include_contexts_by_file: &mut HashMap<&'a str, Option<CIncludeContext>>,
    javascript_import_contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if let Some(deadline) = deadline {
        deadline.check("resolving reference candidates")?;
    }
    let call_arity = call_context.arity;
    if language_id == Some(LanguageId::Go) {
        if let Some((imported_name, binding)) = resolve_go_import_binding_for_reference(
            &source_symbol.file_path,
            reference_name,
            file_overrides,
            go_import_contexts_by_file,
            deadline,
        )? {
            let candidates = name_index
                .get(&imported_name)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.node_kind == "function_declaration"
                        && candidate.semantic_path == imported_name
                        && binding.package_paths.contains(&candidate.file_path)
                })
                .collect::<Vec<_>>();
            return Ok(
                (candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone())
            );
        }

        let candidates = semantic_path_index
            .get(reference_name)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| raw_symbols[*index].file_path == source_symbol.file_path)
            .collect::<Vec<_>>();
        return Ok((candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone()));
    }
    if language_id == Some(LanguageId::Rust) {
        let candidates = semantic_path_index
            .get(reference_name)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| raw_symbols[*index].file_path == source_symbol.file_path)
            .collect::<Vec<_>>();
        return Ok((candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone()));
    }
    if language_id == Some(LanguageId::Java) {
        let Some(call_arity) = call_context.arity else {
            return Ok(None);
        };
        if let Some((method_name, binding)) = resolve_java_import_binding_for_reference(
            &source_symbol.file_path,
            reference_name,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )? {
            let target_path = format!("{}::{method_name}", binding.semantic_path);
            let candidates = semantic_path_index
                .get(&target_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.file_path == binding.source_path
                        && candidate.node_kind == "method_declaration"
                        && candidate
                            .signature
                            .as_deref()
                            .is_some_and(java_method_signature_is_static)
                        && candidate.parameters.len() == call_arity
                        && !candidate
                            .parameters
                            .iter()
                            .any(|parameter| parameter.contains("..."))
                })
                .collect::<Vec<_>>();
            return Ok(
                (candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone())
            );
        }

        let method_name = if let Some(method_name) = reference_name.strip_prefix("this.") {
            if method_name.is_empty() || method_name.contains('.') {
                return Ok(None);
            }
            method_name
        } else {
            reference_name
        };
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        let target_path = format!("{scope_path}::{method_name}");
        let candidates = semantic_path_index
            .get(&target_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.file_path == source_symbol.file_path
                    && candidate.node_kind == "method_declaration"
                    && candidate.parameters.len() == call_arity
                    && !candidate
                        .parameters
                        .iter()
                        .any(|parameter| parameter.contains("..."))
            })
            .collect::<Vec<_>>();
        return Ok((candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone()));
    }
    let (lookup_name, module_hint) = if language_id == Some(LanguageId::Python) {
        python_reference_lookup(reference_name)
    } else {
        (reference_name, None)
    };
    let javascript_import_binding = if matches!(
        language_id,
        Some(LanguageId::JavaScript | LanguageId::TypeScript | LanguageId::Tsx)
    ) {
        resolve_javascript_named_import_binding_for_reference(
            &source_symbol.file_path,
            lookup_name,
            file_overrides,
            javascript_import_contexts_by_file,
            deadline,
        )?
    } else {
        None
    };
    let candidate_lookup_name = javascript_import_binding
        .as_ref()
        .map(|binding| binding.imported_name.as_str())
        .unwrap_or(lookup_name);
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
        let Some(candidates) = name_index.get(candidate_lookup_name) else {
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
    let import_bound_candidates = if let Some(binding) = javascript_import_binding {
        candidate_slice
            .iter()
            .copied()
            .filter(|index| {
                binding
                    .module_paths
                    .contains(&raw_symbols[*index].file_path)
            })
            .collect()
    } else {
        candidate_slice
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
        let filtered = import_bound_candidates
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
            import_bound_candidates
        } else {
            filtered
        }
    } else {
        import_bound_candidates
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
    let source_file_path = source_symbol.file_path.as_str();
    if !include_contexts_by_file.contains_key(source_file_path) {
        let context = c_include_context_for_file_with_overrides_and_deadline(
            &source_symbol.file_path,
            file_overrides,
            deadline,
        )
        .ok();
        if let Some(deadline) = deadline {
            deadline.check("building C include context")?;
        }
        include_contexts_by_file.insert(source_file_path, context);
    }
    let include_context = include_contexts_by_file
        .get(source_file_path)
        .and_then(Option::as_ref);

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
            include_context,
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

fn java_method_signature_is_static(signature: &str) -> bool {
    signature.split_whitespace().any(|token| token == "static")
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::{Duration, Instant};

    use super::resolve_dependencies_for_symbol_with_deadline;
    use crate::symbol_index_model::IndexedSymbol;
    use crate::workspace_scan::WorkspaceScanDeadline;

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
            is_overload: false,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
            reference_facts: Vec::new(),
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
            &mut std::collections::HashMap::new(),
            &mut std::collections::HashMap::new(),
            &mut std::collections::BTreeMap::new(),
            &mut std::collections::BTreeMap::new(),
            &mut std::collections::BTreeMap::new(),
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
