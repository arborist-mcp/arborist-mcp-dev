use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::Result;

use super::super::c::{CIncludeContext, c_include_context_for_file_with_overrides_and_deadline};
use super::super::csharp::{
    CSharpBaseTypeBinding, CSharpGlobalImportContext, CSharpImportContext,
    CSharpNamespaceImportBinding, CSharpStaticTypeImportBinding, CSharpTypeAliasBinding,
    csharp_global_type_alias_name_is_ambiguous, csharp_type_alias_name_is_ambiguous_for_reference,
    csharp_type_alias_name_is_declared_for_reference,
    resolve_csharp_base_type_binding_for_reference,
    resolve_csharp_global_namespace_imports_for_reference,
    resolve_csharp_global_static_type_imports_for_reference,
    resolve_csharp_global_type_alias_binding_for_reference,
    resolve_csharp_namespace_imports_for_reference,
    resolve_csharp_static_type_imports_for_reference,
    resolve_csharp_type_alias_binding_for_reference,
};
use super::super::go::{
    GoImportContext, go_package_name_for_source_file, resolve_go_import_binding_for_reference,
};
use super::super::java::{
    JavaImportBinding, JavaImportContext, resolve_java_import_binding_for_reference,
    resolve_java_static_method_import_binding_for_reference,
    resolve_java_type_import_binding_for_name,
};
use super::super::javascript::{
    JavaScriptImportContext, resolve_javascript_named_import_binding_for_reference,
};
use super::super::rust::{RustOutOfLineModuleContext, resolve_rust_out_of_line_module_reference};
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
use crate::language::{
    JavaDirectSuperclassReference, detect_language,
    java_direct_interface_references_for_declaration, java_direct_superclass_reference,
    normalize_path, parse_document, read_source,
};
use crate::model::LanguageId;
use crate::patching::resolve_local_python_imported_symbol;
use crate::symbol_index_model::{
    GoReferenceDetails, IndexedSymbol, ReferenceLanguageDetails, RustImportRoot,
};
use crate::symbol_reference_compat::effective_reference_facts;
use crate::workspace_scan::WorkspaceScanDeadline;

struct GoSamePackageReferenceTarget<'a> {
    reference_name: &'a str,
    node_kind: &'a str,
    candidate_indexes: &'a [usize],
}

enum GoNamedTypeDeclaration {
    Absent,
    Unique,
    Ambiguous,
}

enum GoSimpleTypeAliasTerminalTarget {
    NotAlias,
    Resolved(String),
    UnresolvedAlias,
}

#[derive(Clone, Copy)]
struct CSharpCandidateRequirements {
    node_kind: &'static str,
    require_static: bool,
    require_instance: bool,
    require_same_file: bool,
}

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
    rust_out_of_line_module_context: &RustOutOfLineModuleContext,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
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
        rust_out_of_line_module_context,
        java_import_contexts_by_file,
        csharp_import_contexts_by_file,
        csharp_global_import_context,
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
    rust_out_of_line_module_context: &RustOutOfLineModuleContext,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
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
                ReferenceLanguageDetails::Go(_) => (false, false, false),
                ReferenceLanguageDetails::Rust(_) => (false, false, false),
            };
        let rust_import_root = match &reference.language_details {
            ReferenceLanguageDetails::Rust(details) => details.import_root.as_ref(),
            _ => None,
        };
        let go_reference_details = match &reference.language_details {
            ReferenceLanguageDetails::Go(details) => Some(details),
            _ => None,
        };
        if matches!(
            language_id,
            Some(LanguageId::Cpp | LanguageId::Java | LanguageId::CSharp)
        ) && let Some(call_arities) = reference.call_arities.as_ref()
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
                    rust_import_root,
                    go_reference_details,
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
                    rust_out_of_line_module_context,
                    java_import_contexts_by_file,
                    csharp_import_contexts_by_file,
                    csharp_global_import_context,
                    deadline,
                )? && target_symbol_id != symbol.symbol_id
                {
                    dependencies.insert(target_symbol_id);
                }
            }
        } else if let Some(target_symbol_id) = resolve_reference_path_with_deadline(
            &reference.spelling,
            rust_import_root,
            go_reference_details,
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
            rust_out_of_line_module_context,
            java_import_contexts_by_file,
            csharp_import_contexts_by_file,
            csharp_global_import_context,
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
    rust_import_root: Option<&RustImportRoot>,
    go_reference_details: Option<&GoReferenceDetails>,
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
    rust_out_of_line_module_context: &RustOutOfLineModuleContext,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if let Some(deadline) = deadline {
        deadline.check("resolving reference candidates")?;
    }
    let call_arity = call_context.arity;
    if language_id == Some(LanguageId::Go) {
        if let Some(details) = go_reference_details {
            return match (details.type_conversion, details.type_assertion) {
                (true, false) => resolve_go_type_conversion_reference(
                    source_symbol,
                    reference_name,
                    raw_symbols,
                    name_index,
                    semantic_path_index,
                    file_overrides,
                    go_import_contexts_by_file,
                    deadline,
                ),
                (false, true) => resolve_go_type_assertion_reference(
                    source_symbol,
                    reference_name,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    go_import_contexts_by_file,
                    deadline,
                ),
                _ => Ok(None),
            };
        }
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
        if candidates.len() == 1 {
            return Ok(Some(raw_symbols[candidates[0]].symbol_id.clone()));
        }
        if reference_name.contains("::") {
            if let Some(method_symbol_id) = resolve_go_same_package_method_reference(
                source_symbol,
                reference_name,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                go_import_contexts_by_file,
                deadline,
            )? {
                return Ok(Some(method_symbol_id));
            }
            return resolve_go_same_package_type_alias_method_reference(
                source_symbol,
                reference_name,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                go_import_contexts_by_file,
                deadline,
            );
        }
        return resolve_go_same_package_function_reference(
            source_symbol,
            reference_name,
            raw_symbols,
            name_index,
            file_overrides,
            go_import_contexts_by_file,
            deadline,
        );
    }
    if language_id == Some(LanguageId::Rust) {
        let candidates = if matches!(rust_import_root, None | Some(RustImportRoot::SelfModule)) {
            semantic_path_index
                .get(reference_name)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| raw_symbols[*index].file_path == source_symbol.file_path)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if candidates.len() == 1 {
            return Ok(Some(raw_symbols[candidates[0]].symbol_id.clone()));
        }

        let Some((target_file_path, target_semantic_path)) =
            resolve_rust_out_of_line_module_reference(
                rust_out_of_line_module_context,
                &source_symbol.file_path,
                reference_name,
                rust_import_root,
            )
        else {
            return Ok(None);
        };
        let candidates = semantic_path_index
            .get(&target_semantic_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.file_path == target_file_path
                    && candidate.node_kind == "function_item"
                    && candidate.semantic_path == target_semantic_path
            })
            .collect::<Vec<_>>();
        return Ok((candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone()));
    }
    if language_id == Some(LanguageId::CSharp) {
        let Some(call_arity) = call_context.arity else {
            return Ok(None);
        };
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        let source_namespace_path =
            csharp_source_namespace_path(source_symbol, raw_symbols).flatten();
        if reference_name == "this" {
            if source_symbol.node_kind != "constructor_declaration" {
                return Ok(None);
            }
            let target_path = format!("{scope_path}::{}", source_symbol.base_name);
            return Ok(resolve_csharp_candidate(
                raw_symbols,
                semantic_path_index,
                &target_path,
                Some(source_symbol),
                call_arity,
                CSharpCandidateRequirements {
                    node_kind: "constructor_declaration",
                    require_static: false,
                    require_instance: false,
                    require_same_file: true,
                },
            ));
        }
        if reference_name == "base" {
            if source_symbol.node_kind != "constructor_declaration" {
                return Ok(None);
            }
            let Some(base_type_binding) = csharp_source_base_type_binding(
                source_symbol,
                raw_symbols,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            let Some(target_path) =
                csharp_base_constructor_target_path(source_symbol, raw_symbols, &base_type_binding)
            else {
                return Ok(None);
            };
            return Ok(resolve_csharp_candidate(
                raw_symbols,
                semantic_path_index,
                &target_path,
                Some(source_symbol),
                call_arity,
                CSharpCandidateRequirements {
                    node_kind: "constructor_declaration",
                    require_static: false,
                    require_instance: false,
                    require_same_file: false,
                },
            ));
        }
        if let Some(method_name) = reference_name.strip_prefix("base.") {
            if method_name.is_empty() || method_name.contains('.') {
                return Ok(None);
            }
            let Some(base_type_binding) = csharp_source_base_type_binding(
                source_symbol,
                raw_symbols,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            let Some(target_path) = csharp_base_method_target_path(
                source_symbol,
                raw_symbols,
                semantic_path_index,
                &base_type_binding,
                method_name,
                call_arity,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            return Ok(resolve_csharp_candidate(
                raw_symbols,
                semantic_path_index,
                &target_path,
                Some(source_symbol),
                call_arity,
                CSharpCandidateRequirements {
                    node_kind: "method_declaration",
                    require_static: false,
                    require_instance: true,
                    require_same_file: false,
                },
            ));
        }
        if let Some(target_path) = csharp_global_qualified_static_target_path(reference_name) {
            return Ok(resolve_csharp_candidate(
                raw_symbols,
                semantic_path_index,
                &target_path,
                Some(source_symbol),
                call_arity,
                CSharpCandidateRequirements {
                    node_kind: "method_declaration",
                    require_static: true,
                    require_instance: false,
                    require_same_file: false,
                },
            ));
        }
        if let Some((type_path, _)) = reference_name.rsplit_once('.')
            && type_path.contains('.')
            && let Some(first_type_segment) = type_path.split('.').next()
            && csharp_type_alias_name_is_declared_for_reference(
                &source_symbol.file_path,
                first_type_segment,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        {
            return Ok(None);
        }
        if let Some(target_path) =
            csharp_nested_type_static_target_path(reference_name, source_symbol, raw_symbols)
        {
            return Ok(resolve_csharp_candidate(
                raw_symbols,
                semantic_path_index,
                &target_path,
                Some(source_symbol),
                call_arity,
                CSharpCandidateRequirements {
                    node_kind: "method_declaration",
                    require_static: true,
                    require_instance: false,
                    require_same_file: false,
                },
            ));
        }
        if let Some((method_name, binding)) = resolve_csharp_type_alias_binding_for_reference(
            &source_symbol.file_path,
            reference_name,
            source_namespace_path,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            let Some((alias_name, _)) = reference_name.split_once('.') else {
                return Ok(None);
            };
            if !csharp_alias_name_is_unshadowed(alias_name, source_symbol, raw_symbols) {
                return Ok(None);
            }
            return Ok(resolve_csharp_imported_static_method(
                raw_symbols,
                semantic_path_index,
                &binding,
                &method_name,
                call_arity,
            ));
        }
        if csharp_type_alias_name_is_ambiguous_for_reference(
            &source_symbol.file_path,
            reference_name,
            source_namespace_path,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            return Ok(None);
        }
        if let Some(csharp_global_import_context) = csharp_global_import_context
            && csharp_global_type_alias_name_is_ambiguous(
                reference_name,
                csharp_global_import_context,
            )
        {
            return Ok(None);
        }
        if let Some(csharp_global_import_context) = csharp_global_import_context
            && let Some((method_name, binding)) =
                resolve_csharp_global_type_alias_binding_for_reference(
                    reference_name,
                    csharp_global_import_context,
                )
        {
            let Some((alias_name, _)) = reference_name.split_once('.') else {
                return Ok(None);
            };
            if !csharp_alias_name_is_unshadowed(alias_name, source_symbol, raw_symbols) {
                return Ok(None);
            }
            return Ok(resolve_csharp_imported_static_method(
                raw_symbols,
                semantic_path_index,
                &binding,
                &method_name,
                call_arity,
            ));
        }
        if let Some(target_path) =
            csharp_simple_type_static_target_path(reference_name, source_symbol, raw_symbols)
        {
            return Ok(resolve_csharp_candidate(
                raw_symbols,
                semantic_path_index,
                &target_path,
                Some(source_symbol),
                call_arity,
                CSharpCandidateRequirements {
                    node_kind: "method_declaration",
                    require_static: true,
                    require_instance: false,
                    require_same_file: false,
                },
            ));
        }
        if let Some((type_name, method_name)) = reference_name.split_once('.')
            && !type_name.is_empty()
            && type_name != "this"
            && !type_name.starts_with("global::")
            && !method_name.is_empty()
            && !method_name.contains('.')
        {
            if !csharp_namespace_import_type_is_unshadowed(type_name, source_symbol, raw_symbols) {
                return Ok(None);
            }
            let mut namespace_imports = resolve_csharp_namespace_imports_for_reference(
                &source_symbol.file_path,
                type_name,
                source_namespace_path,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
            if let Some(csharp_global_import_context) = csharp_global_import_context {
                namespace_imports.extend(resolve_csharp_global_namespace_imports_for_reference(
                    type_name,
                    csharp_global_import_context,
                ));
            }
            return Ok(resolve_csharp_namespace_imported_static_method(
                raw_symbols,
                semantic_path_index,
                &namespace_imports,
                type_name,
                method_name,
                call_arity,
            ));
        }
        let (method_name, has_explicit_this_receiver) =
            if let Some(method_name) = reference_name.strip_prefix("this.") {
                if method_name.is_empty() || method_name.contains('.') {
                    return Ok(None);
                }
                (method_name, true)
            } else {
                if reference_name.contains('.') {
                    return Ok(None);
                }
                (reference_name, false)
            };
        let target_path = format!("{scope_path}::{method_name}");
        let has_same_type_method = semantic_path_index
            .get(&target_path)
            .into_iter()
            .flatten()
            .copied()
            .any(|index| {
                let candidate = &raw_symbols[index];
                candidate.file_path == source_symbol.file_path
                    && candidate.node_kind == "method_declaration"
            });
        if has_same_type_method {
            return Ok(resolve_csharp_candidate(
                raw_symbols,
                semantic_path_index,
                &target_path,
                Some(source_symbol),
                call_arity,
                CSharpCandidateRequirements {
                    node_kind: "method_declaration",
                    require_static: false,
                    require_instance: false,
                    require_same_file: true,
                },
            ));
        }
        if !csharp_method_is_static(source_symbol)
            && let Some(base_type_binding) = csharp_source_base_type_binding(
                source_symbol,
                raw_symbols,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            && let Some(target_path) = csharp_base_method_target_path(
                source_symbol,
                raw_symbols,
                semantic_path_index,
                &base_type_binding,
                method_name,
                call_arity,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        {
            return Ok(resolve_csharp_candidate(
                raw_symbols,
                semantic_path_index,
                &target_path,
                Some(source_symbol),
                call_arity,
                CSharpCandidateRequirements {
                    node_kind: "method_declaration",
                    require_static: false,
                    require_instance: true,
                    require_same_file: false,
                },
            ));
        }
        if has_explicit_this_receiver {
            return Ok(None);
        }
        let mut static_type_imports = resolve_csharp_static_type_imports_for_reference(
            &source_symbol.file_path,
            reference_name,
            source_namespace_path,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?;
        if let Some(csharp_global_import_context) = csharp_global_import_context {
            static_type_imports.extend(resolve_csharp_global_static_type_imports_for_reference(
                reference_name,
                csharp_global_import_context,
            ));
        }
        return Ok(resolve_csharp_static_type_imported_method(
            raw_symbols,
            semantic_path_index,
            &static_type_imports,
            method_name,
            call_arity,
        ));
    }
    if language_id == Some(LanguageId::Java) {
        let Some(call_arity) = call_context.arity else {
            return Ok(None);
        };
        if reference_name == "this" {
            if source_symbol.node_kind != "constructor_declaration" {
                return Ok(None);
            }
            let Some(scope_path) = source_symbol.scope_path.as_deref() else {
                return Ok(None);
            };
            let target_path = format!("{scope_path}::{}", source_symbol.base_name);
            let candidates = semantic_path_index
                .get(&target_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.file_path == source_symbol.file_path
                        && candidate.node_kind == "constructor_declaration"
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
        if let Some(method_name) = reference_name.strip_prefix("super.") {
            if method_name.is_empty() || method_name.contains('.') {
                return Ok(None);
            }
            return resolve_java_simple_super_method_reference(
                source_symbol,
                method_name,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                call_arity,
                deadline,
            );
        }
        if reference_name == "super" {
            return resolve_java_same_file_super_constructor_reference(
                source_symbol,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                call_arity,
                deadline,
            );
        }
        if let Some(symbol_id) = resolve_java_nested_static_method_reference(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            return Ok(Some(symbol_id));
        }
        if let Some((method_name, binding)) = resolve_java_import_binding_for_reference(
            &source_symbol.file_path,
            reference_name,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )? {
            return Ok(resolve_java_imported_static_method(
                raw_symbols,
                semantic_path_index,
                &binding,
                &method_name,
                call_arity,
            ));
        }
        if let Some(symbol_id) = resolve_java_same_package_static_method_reference(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            call_arity,
        ) {
            return Ok(Some(symbol_id));
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
        let same_type_candidates = semantic_path_index
            .get(&target_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.file_path == source_symbol.file_path
                    && candidate.node_kind == "method_declaration"
            })
            .collect::<Vec<_>>();
        let candidates = same_type_candidates
            .iter()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.parameters.len() == call_arity
                    && !candidate
                        .parameters
                        .iter()
                        .any(|parameter| parameter.contains("..."))
            })
            .collect::<Vec<_>>();
        if !same_type_candidates.is_empty() {
            return Ok(
                (candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone())
            );
        }
        if method_name != reference_name {
            return resolve_java_direct_interface_default_method_reference(
                source_symbol,
                method_name,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                call_arity,
                deadline,
            );
        }
        if let Some(symbol_id) = resolve_java_simple_super_method_reference(
            source_symbol,
            method_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            return Ok(Some(symbol_id));
        }
        if let Some(symbol_id) = resolve_java_direct_interface_default_method_reference(
            source_symbol,
            method_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            return Ok(Some(symbol_id));
        }
        let Some(binding) = resolve_java_static_method_import_binding_for_reference(
            &source_symbol.file_path,
            reference_name,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return Ok(resolve_java_imported_static_method(
            raw_symbols,
            semantic_path_index,
            &binding,
            reference_name,
            call_arity,
        ));
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

fn resolve_csharp_candidate(
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    target_path: &str,
    source_symbol: Option<&IndexedSymbol>,
    call_arity: usize,
    requirements: CSharpCandidateRequirements,
) -> Option<String> {
    let candidates = semantic_path_index
        .get(target_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            (!requirements.require_same_file
                || source_symbol.is_some_and(|source| candidate.file_path == source.file_path))
                && candidate.node_kind == requirements.node_kind
                && candidate.parameters.len() == call_arity
                && !candidate
                    .parameters
                    .iter()
                    .any(|parameter| parameter.split_whitespace().any(|part| part == "params"))
                && (!requirements.require_static || csharp_method_is_static(candidate))
                && (!requirements.require_instance || !csharp_method_is_static(candidate))
        })
        .collect::<Vec<_>>();
    (candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone())
}

fn csharp_source_type_declaration<'a>(
    source_symbol: &IndexedSymbol,
    raw_symbols: &'a [IndexedSymbol],
) -> Option<&'a IndexedSymbol> {
    let scope_path = source_symbol.scope_path.as_deref()?;
    let candidates = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.file_path == source_symbol.file_path
                && candidate.semantic_path == scope_path
                && csharp_is_base_constructible_type(candidate)
        })
        .collect::<Vec<_>>();
    (candidates.len() == 1).then_some(candidates[0])
}

fn csharp_source_base_type_binding(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    let Some(source_type) = csharp_source_type_declaration(source_symbol, raw_symbols) else {
        return Ok(None);
    };
    resolve_csharp_base_type_binding_for_reference(
        &source_symbol.file_path,
        source_type.byte_range,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )
}

fn csharp_base_type_binding_for_type(
    source_type: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    let source_namespace_path = csharp_source_namespace_path(source_type, raw_symbols).flatten();
    resolve_csharp_base_type_binding_for_reference(
        &source_type.file_path,
        source_type.byte_range,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )
}

fn csharp_base_type_path(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    binding: &CSharpBaseTypeBinding,
) -> Option<String> {
    if binding.alias_name.as_deref().is_some_and(|alias_name| {
        !csharp_alias_name_is_unshadowed(alias_name, source_symbol, raw_symbols)
    }) {
        return None;
    }
    if binding.is_global_qualified {
        return csharp_unique_base_constructible_type_path(
            raw_symbols,
            &binding.semantic_type_path,
        );
    }
    if binding.semantic_type_path.contains("::") {
        return csharp_unshadowed_qualified_base_type_path(source_symbol, raw_symbols, binding);
    }

    let base_type_path = csharp_source_namespace_path(source_symbol, raw_symbols)?
        .map(|namespace_path| format!("{namespace_path}::{}", binding.semantic_type_path))
        .unwrap_or_else(|| binding.semantic_type_path.clone());
    let local_type_candidates = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.semantic_path == base_type_path && csharp_is_type_declaration(candidate)
        })
        .count();
    if local_type_candidates != 0 {
        return csharp_unique_base_constructible_type_path(raw_symbols, &base_type_path);
    }

    let mut imported_type_paths = BTreeSet::new();
    for type_path in binding
        .namespace_import_paths
        .iter()
        .map(|namespace_path| format!("{namespace_path}::{}", binding.semantic_type_path))
        .collect::<BTreeSet<_>>()
    {
        let candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.semantic_path == type_path && csharp_is_type_declaration(candidate)
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => {}
            [candidate] if csharp_is_base_constructible_type(candidate) => {
                imported_type_paths.insert(type_path);
            }
            _ => return None,
        }
    }
    (imported_type_paths.len() == 1).then(|| imported_type_paths.into_iter().next().unwrap())
}

fn csharp_unshadowed_qualified_base_type_path(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    binding: &CSharpBaseTypeBinding,
) -> Option<String> {
    let base_type_path = binding.semantic_type_path.as_str();
    if let Some(mut namespace_path) = csharp_source_namespace_path(source_symbol, raw_symbols)? {
        loop {
            let relative_type_path = format!("{namespace_path}::{base_type_path}");
            if raw_symbols.iter().any(|candidate| {
                candidate.semantic_path == relative_type_path
                    && csharp_is_type_declaration(candidate)
            }) {
                return None;
            }
            let Some((parent_namespace_path, _)) = namespace_path.rsplit_once("::") else {
                break;
            };
            namespace_path = parent_namespace_path;
        }
    }
    csharp_unique_base_constructible_type_path(raw_symbols, base_type_path)
}

fn csharp_unique_base_constructible_type_path(
    raw_symbols: &[IndexedSymbol],
    type_path: &str,
) -> Option<String> {
    let candidates = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.semantic_path == type_path && csharp_is_type_declaration(candidate)
        })
        .collect::<Vec<_>>();
    (candidates.len() == 1 && csharp_is_base_constructible_type(candidates[0]))
        .then(|| type_path.to_string())
}

fn csharp_base_constructor_target_path(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    binding: &CSharpBaseTypeBinding,
) -> Option<String> {
    let base_type_path = csharp_base_type_path(source_symbol, raw_symbols, binding)?;
    let base_type_name = base_type_path.rsplit("::").next()?;
    Some(format!("{base_type_path}::{base_type_name}"))
}

#[allow(clippy::too_many_arguments)]
fn csharp_base_method_target_path(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    binding: &CSharpBaseTypeBinding,
    method_name: &str,
    call_arity: usize,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(mut base_type_path) = csharp_base_type_path(source_symbol, raw_symbols, binding)
    else {
        return Ok(None);
    };
    let mut visited_type_paths = BTreeSet::new();

    loop {
        if !visited_type_paths.insert(base_type_path.clone()) {
            return Ok(None);
        }
        let Some(type_indexes) = semantic_path_index.get(&base_type_path) else {
            return Ok(None);
        };
        let type_indexes = type_indexes
            .iter()
            .copied()
            .filter(|index| csharp_is_base_constructible_type(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if type_indexes.len() != 1 {
            return Ok(None);
        }

        let method_path = format!("{base_type_path}::{method_name}");
        let methods = semantic_path_index
            .get(&method_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| raw_symbols[*index].node_kind == "method_declaration")
            .collect::<Vec<_>>();
        if !methods.is_empty() {
            let matching_methods = methods
                .iter()
                .copied()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.parameters.len() == call_arity
                        && !candidate.parameters.iter().any(|parameter| {
                            parameter.split_whitespace().any(|part| part == "params")
                        })
                        && !csharp_method_is_static(candidate)
                })
                .collect::<Vec<_>>();
            return Ok((matching_methods.len() == 1).then_some(method_path));
        }

        let source_type = &raw_symbols[type_indexes[0]];
        let Some(next_binding) = csharp_base_type_binding_for_type(
            source_type,
            raw_symbols,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let Some(next_type_path) = csharp_base_type_path(source_type, raw_symbols, &next_binding)
        else {
            return Ok(None);
        };
        base_type_path = next_type_path;
    }
}

fn csharp_is_base_constructible_type(symbol: &IndexedSymbol) -> bool {
    matches!(
        symbol.node_kind.as_str(),
        "class_declaration" | "record_declaration"
    )
}

#[allow(clippy::too_many_arguments)]
fn resolve_go_type_conversion_reference(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((receiver_type, method_name)) = reference_name.split_once("::") else {
        return Ok(None);
    };
    if receiver_type.is_empty()
        || method_name.is_empty()
        || receiver_type.contains(':')
        || method_name.contains(':')
    {
        return Ok(None);
    }

    match go_named_type_declaration_status(
        source_symbol,
        receiver_type,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        go_import_contexts_by_file,
        deadline,
    )? {
        GoNamedTypeDeclaration::Unique => resolve_go_same_package_method_reference(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            go_import_contexts_by_file,
            deadline,
        ),
        GoNamedTypeDeclaration::Absent => {
            match go_same_package_simple_type_alias_terminal_target(
                source_symbol,
                receiver_type,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                go_import_contexts_by_file,
                deadline,
            )? {
                GoSimpleTypeAliasTerminalTarget::Resolved(target_type) => {
                    resolve_go_same_package_method_reference(
                        source_symbol,
                        &format!("{target_type}::{method_name}"),
                        raw_symbols,
                        semantic_path_index,
                        file_overrides,
                        go_import_contexts_by_file,
                        deadline,
                    )
                }
                GoSimpleTypeAliasTerminalTarget::NotAlias => {
                    resolve_go_same_package_function_reference(
                        source_symbol,
                        receiver_type,
                        raw_symbols,
                        name_index,
                        file_overrides,
                        go_import_contexts_by_file,
                        deadline,
                    )
                }
                GoSimpleTypeAliasTerminalTarget::UnresolvedAlias => Ok(None),
            }
        }
        GoNamedTypeDeclaration::Ambiguous => Ok(None),
    }
}

fn resolve_go_type_assertion_reference(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((receiver_type, method_name)) = reference_name.split_once("::") else {
        return Ok(None);
    };
    if receiver_type.is_empty()
        || method_name.is_empty()
        || receiver_type.contains(':')
        || method_name.contains(':')
    {
        return Ok(None);
    }
    let target_type = match go_named_type_declaration_status(
        source_symbol,
        receiver_type,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        go_import_contexts_by_file,
        deadline,
    )? {
        GoNamedTypeDeclaration::Unique => receiver_type.to_string(),
        GoNamedTypeDeclaration::Absent => {
            let GoSimpleTypeAliasTerminalTarget::Resolved(target_type) =
                go_same_package_simple_type_alias_terminal_target(
                    source_symbol,
                    receiver_type,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    go_import_contexts_by_file,
                    deadline,
                )?
            else {
                return Ok(None);
            };
            target_type
        }
        GoNamedTypeDeclaration::Ambiguous => return Ok(None),
    };
    resolve_go_same_package_method_reference(
        source_symbol,
        &format!("{target_type}::{method_name}"),
        raw_symbols,
        semantic_path_index,
        file_overrides,
        go_import_contexts_by_file,
        deadline,
    )
}

fn go_named_type_declaration_status(
    source_symbol: &IndexedSymbol,
    receiver_type: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<GoNamedTypeDeclaration> {
    let Some(caller_package_name) = go_package_name_for_source_file(
        &source_symbol.file_path,
        file_overrides,
        go_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(GoNamedTypeDeclaration::Ambiguous);
    };
    let Some(caller_directory) = Path::new(&source_symbol.file_path)
        .parent()
        .map(normalize_path)
    else {
        return Ok(GoNamedTypeDeclaration::Ambiguous);
    };

    let mut type_declaration_count = 0;
    for index in semantic_path_index.get(receiver_type).into_iter().flatten() {
        if let Some(deadline) = deadline {
            deadline.check("resolving Go type conversion receiver")?;
        }
        let candidate = &raw_symbols[*index];
        if candidate.node_kind != "type_spec"
            || candidate.semantic_path != receiver_type
            || !is_production_go_source_file(&candidate.file_path)
            || Path::new(&candidate.file_path)
                .parent()
                .map(normalize_path)
                .as_deref()
                != Some(caller_directory.as_str())
        {
            continue;
        }
        let Some(candidate_package_name) = go_package_name_for_source_file(
            &candidate.file_path,
            file_overrides,
            go_import_contexts_by_file,
            deadline,
        )?
        else {
            continue;
        };
        if candidate_package_name == caller_package_name {
            type_declaration_count += 1;
            if type_declaration_count > 1 {
                return Ok(GoNamedTypeDeclaration::Ambiguous);
            }
        }
    }

    Ok(if type_declaration_count == 1 {
        GoNamedTypeDeclaration::Unique
    } else {
        GoNamedTypeDeclaration::Absent
    })
}

fn resolve_go_same_package_type_alias_method_reference(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((alias_name, method_name)) = reference_name.split_once("::") else {
        return Ok(None);
    };
    if alias_name.is_empty()
        || method_name.is_empty()
        || alias_name.contains(':')
        || method_name.contains(':')
    {
        return Ok(None);
    }
    let GoSimpleTypeAliasTerminalTarget::Resolved(target_type) =
        go_same_package_simple_type_alias_terminal_target(
            source_symbol,
            alias_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            go_import_contexts_by_file,
            deadline,
        )?
    else {
        return Ok(None);
    };
    resolve_go_same_package_method_reference(
        source_symbol,
        &format!("{target_type}::{method_name}"),
        raw_symbols,
        semantic_path_index,
        file_overrides,
        go_import_contexts_by_file,
        deadline,
    )
}

fn go_same_package_simple_type_alias_terminal_target(
    source_symbol: &IndexedSymbol,
    alias_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<GoSimpleTypeAliasTerminalTarget> {
    let mut current_name = alias_name.to_string();
    let mut visited_aliases = BTreeSet::new();
    let mut followed_alias = false;
    loop {
        if let Some(deadline) = deadline {
            deadline.check("resolving Go type alias chain")?;
        }
        if !visited_aliases.insert(current_name.clone()) {
            return Ok(GoSimpleTypeAliasTerminalTarget::UnresolvedAlias);
        }
        match go_named_type_declaration_status(
            source_symbol,
            &current_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            go_import_contexts_by_file,
            deadline,
        )? {
            GoNamedTypeDeclaration::Unique => {
                return Ok(GoSimpleTypeAliasTerminalTarget::Resolved(current_name));
            }
            GoNamedTypeDeclaration::Ambiguous => {
                return Ok(GoSimpleTypeAliasTerminalTarget::UnresolvedAlias);
            }
            GoNamedTypeDeclaration::Absent => {}
        }
        match go_same_package_simple_type_alias_target(
            source_symbol,
            &current_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            go_import_contexts_by_file,
            deadline,
        )? {
            GoSimpleTypeAliasTerminalTarget::NotAlias => {
                return Ok(if followed_alias {
                    GoSimpleTypeAliasTerminalTarget::UnresolvedAlias
                } else {
                    GoSimpleTypeAliasTerminalTarget::NotAlias
                });
            }
            GoSimpleTypeAliasTerminalTarget::Resolved(next_name) => {
                followed_alias = true;
                current_name = next_name;
            }
            GoSimpleTypeAliasTerminalTarget::UnresolvedAlias => {
                return Ok(GoSimpleTypeAliasTerminalTarget::UnresolvedAlias);
            }
        }
    }
}

fn go_same_package_simple_type_alias_target(
    source_symbol: &IndexedSymbol,
    alias_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<GoSimpleTypeAliasTerminalTarget> {
    let Some(caller_package_name) = go_package_name_for_source_file(
        &source_symbol.file_path,
        file_overrides,
        go_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(GoSimpleTypeAliasTerminalTarget::UnresolvedAlias);
    };
    let Some(caller_directory) = Path::new(&source_symbol.file_path)
        .parent()
        .map(normalize_path)
    else {
        return Ok(GoSimpleTypeAliasTerminalTarget::UnresolvedAlias);
    };

    let mut candidates = Vec::new();
    for index in semantic_path_index.get(alias_name).into_iter().flatten() {
        if let Some(deadline) = deadline {
            deadline.check("resolving Go type alias target")?;
        }
        let candidate = &raw_symbols[*index];
        if candidate.node_kind != "type_alias"
            || candidate.semantic_path != alias_name
            || !is_production_go_source_file(&candidate.file_path)
            || Path::new(&candidate.file_path)
                .parent()
                .map(normalize_path)
                .as_deref()
                != Some(caller_directory.as_str())
        {
            continue;
        }
        let Some(candidate_package_name) = go_package_name_for_source_file(
            &candidate.file_path,
            file_overrides,
            go_import_contexts_by_file,
            deadline,
        )?
        else {
            continue;
        };
        if candidate_package_name == caller_package_name {
            candidates.push(*index);
        }
    }

    let [candidate_index] = candidates.as_slice() else {
        return Ok(if candidates.is_empty() {
            GoSimpleTypeAliasTerminalTarget::NotAlias
        } else {
            GoSimpleTypeAliasTerminalTarget::UnresolvedAlias
        });
    };
    Ok(
        go_simple_type_alias_target(&raw_symbols[*candidate_index]).map_or(
            GoSimpleTypeAliasTerminalTarget::UnresolvedAlias,
            GoSimpleTypeAliasTerminalTarget::Resolved,
        ),
    )
}

fn go_simple_type_alias_target(alias_symbol: &IndexedSymbol) -> Option<String> {
    let declaration = alias_symbol
        .signature
        .as_deref()?
        .strip_prefix("type ")?
        .trim();
    let (alias_name, target_name) = declaration.split_once('=')?;
    if alias_name.trim() != alias_symbol.semantic_path {
        return None;
    }
    let target_name = target_name.trim();
    go_simple_identifier(target_name).then(|| target_name.to_string())
}
fn go_simple_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

fn resolve_go_same_package_function_reference(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if reference_name.is_empty() || reference_name.contains(['.', ':']) {
        return Ok(None);
    }
    let candidate_indexes = name_index.get(reference_name).cloned().unwrap_or_default();
    resolve_go_same_package_reference(
        source_symbol,
        GoSamePackageReferenceTarget {
            reference_name,
            node_kind: "function_declaration",
            candidate_indexes: &candidate_indexes,
        },
        raw_symbols,
        file_overrides,
        go_import_contexts_by_file,
        deadline,
    )
}

fn resolve_go_same_package_method_reference(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((receiver_type, method_name)) = reference_name.split_once("::") else {
        return Ok(None);
    };
    if receiver_type.is_empty()
        || method_name.is_empty()
        || receiver_type.contains(':')
        || method_name.contains(':')
    {
        return Ok(None);
    }
    let candidate_indexes = semantic_path_index
        .get(reference_name)
        .cloned()
        .unwrap_or_default();
    resolve_go_same_package_reference(
        source_symbol,
        GoSamePackageReferenceTarget {
            reference_name,
            node_kind: "method_declaration",
            candidate_indexes: &candidate_indexes,
        },
        raw_symbols,
        file_overrides,
        go_import_contexts_by_file,
        deadline,
    )
}

fn resolve_go_same_package_reference(
    source_symbol: &IndexedSymbol,
    target: GoSamePackageReferenceTarget<'_>,
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(caller_package_name) = go_package_name_for_source_file(
        &source_symbol.file_path,
        file_overrides,
        go_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(caller_directory) = Path::new(&source_symbol.file_path)
        .parent()
        .map(normalize_path)
    else {
        return Ok(None);
    };

    let mut candidates = Vec::new();
    for index in target.candidate_indexes {
        let candidate = &raw_symbols[*index];
        if candidate.node_kind != target.node_kind
            || candidate.semantic_path != target.reference_name
            || !is_production_go_source_file(&candidate.file_path)
            || Path::new(&candidate.file_path)
                .parent()
                .map(normalize_path)
                .as_deref()
                != Some(caller_directory.as_str())
        {
            continue;
        }
        let Some(candidate_package_name) = go_package_name_for_source_file(
            &candidate.file_path,
            file_overrides,
            go_import_contexts_by_file,
            deadline,
        )?
        else {
            continue;
        };
        if candidate_package_name == caller_package_name {
            candidates.push(*index);
        }
    }

    Ok((candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone()))
}

fn is_production_go_source_file(file_path: &str) -> bool {
    !Path::new(file_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.ends_with("_test"))
}

fn csharp_global_qualified_static_target_path(reference_name: &str) -> Option<String> {
    let qualified_name = reference_name.strip_prefix("global::")?;
    let (type_path, method_name) = qualified_name.rsplit_once('.')?;
    if type_path.is_empty()
        || method_name.is_empty()
        || type_path.split('.').any(|segment| segment.is_empty())
    {
        return None;
    }
    Some(format!("{}::{method_name}", type_path.replace('.', "::")))
}

fn csharp_nested_type_static_target_path(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
) -> Option<String> {
    let (type_path, method_name) = reference_name.rsplit_once('.')?;
    if method_name.is_empty()
        || type_path.is_empty()
        || type_path.starts_with("global::")
        || type_path
            .split('.')
            .any(|segment| !is_safe_csharp_identifier(segment))
    {
        return None;
    }
    let type_segments = type_path.split('.').collect::<Vec<_>>();
    if type_segments.len() < 2 {
        return None;
    }
    let first_type_name = type_segments[0];
    let relative_type_path = type_path.replace('.', "::");

    let mut namespace_path = csharp_source_namespace_path(source_symbol, raw_symbols)?;
    loop {
        let root_type_path = namespace_path
            .map(|namespace_path| format!("{namespace_path}::{first_type_name}"))
            .unwrap_or_else(|| first_type_name.to_string());
        let root_type_candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.semantic_path == root_type_path && csharp_is_type_declaration(candidate)
            })
            .count();
        if root_type_candidates > 0 {
            if root_type_candidates != 1 {
                return None;
            }
            let target_type_path = namespace_path
                .map(|namespace_path| format!("{namespace_path}::{relative_type_path}"))
                .unwrap_or_else(|| relative_type_path.clone());
            let target_type_candidates = raw_symbols
                .iter()
                .filter(|candidate| {
                    candidate.semantic_path == target_type_path
                        && csharp_is_type_declaration(candidate)
                })
                .count();
            return (target_type_candidates == 1)
                .then(|| format!("{target_type_path}::{method_name}"));
        }
        namespace_path = match namespace_path {
            Some(current_path) => current_path.rsplit_once("::").map(|(parent, _)| parent),
            None => return None,
        };
    }
}

fn is_safe_csharp_identifier(identifier: &str) -> bool {
    let mut characters = identifier.chars();
    matches!(characters.next(), Some(character) if character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn csharp_simple_type_static_target_path(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
) -> Option<String> {
    let (type_name, method_name) = reference_name.split_once('.')?;
    if type_name.is_empty()
        || method_name.is_empty()
        || method_name.contains('.')
        || type_name == "this"
        || type_name.starts_with("global::")
    {
        return None;
    }

    let mut namespace_path = csharp_source_namespace_path(source_symbol, raw_symbols)?;
    loop {
        let target_type_path = namespace_path
            .map(|namespace_path| format!("{namespace_path}::{type_name}"))
            .unwrap_or_else(|| type_name.to_string());
        let target_type_candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.semantic_path == target_type_path && csharp_is_type_declaration(candidate)
            })
            .count();
        if target_type_candidates > 0 {
            return (target_type_candidates == 1)
                .then(|| format!("{target_type_path}::{method_name}"));
        }
        namespace_path = match namespace_path {
            Some(current_path) => current_path.rsplit_once("::").map(|(parent, _)| parent),
            None => return None,
        };
    }
}

fn csharp_is_type_declaration(symbol: &IndexedSymbol) -> bool {
    matches!(
        symbol.node_kind.as_str(),
        "class_declaration"
            | "struct_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
    )
}

fn csharp_method_is_static(symbol: &IndexedSymbol) -> bool {
    symbol
        .signature
        .as_deref()
        .is_some_and(|signature| signature.split_whitespace().any(|part| part == "static"))
}

fn csharp_namespace_import_type_is_unshadowed(
    type_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
) -> bool {
    let Some(namespace_path) = csharp_source_namespace_path(source_symbol, raw_symbols) else {
        return false;
    };
    let target_path = namespace_path
        .map(|namespace_path| format!("{namespace_path}::{type_name}"))
        .unwrap_or_else(|| type_name.to_string());
    !raw_symbols.iter().any(|candidate| {
        candidate.semantic_path == target_path && csharp_is_type_declaration(candidate)
    })
}

fn csharp_source_namespace_path<'a>(
    source_symbol: &'a IndexedSymbol,
    raw_symbols: &'a [IndexedSymbol],
) -> Option<Option<&'a str>> {
    let Some(mut type_path) = source_symbol.scope_path.as_deref() else {
        return csharp_is_type_declaration(source_symbol).then_some(None);
    };
    if !csharp_is_type_declaration(source_symbol)
        && raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.file_path == source_symbol.file_path
                    && candidate.semantic_path == type_path
                    && csharp_is_type_declaration(candidate)
            })
            .count()
            != 1
    {
        return None;
    }

    loop {
        let type_candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.file_path == source_symbol.file_path
                    && candidate.semantic_path == type_path
                    && csharp_is_type_declaration(candidate)
            })
            .count();
        match type_candidates {
            0 => return Some(Some(type_path)),
            1 => {}
            _ => return None,
        }
        let Some((parent_path, _)) = type_path.rsplit_once("::") else {
            return Some(None);
        };
        type_path = parent_path;
    }
}

fn csharp_alias_name_is_unshadowed(
    alias_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
) -> bool {
    !raw_symbols.iter().any(|candidate| {
        candidate.file_path == source_symbol.file_path
            && candidate.base_name == alias_name
            && csharp_is_type_declaration(candidate)
    })
}

fn resolve_csharp_namespace_imported_static_method(
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    bindings: &[CSharpNamespaceImportBinding],
    type_name: &str,
    method_name: &str,
    call_arity: usize,
) -> Option<String> {
    let mut candidates = Vec::new();
    for binding in bindings {
        let target_type_path = format!("{}::{type_name}", binding.semantic_namespace_path);
        let type_candidates = semantic_path_index
            .get(&target_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .count();
        if type_candidates > 1 {
            return None;
        }
        let target_path = format!("{target_type_path}::{method_name}");
        candidates.extend(
            semantic_path_index
                .get(&target_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.node_kind == "method_declaration"
                        && candidate.parameters.len() == call_arity
                        && !candidate.parameters.iter().any(|parameter| {
                            parameter.split_whitespace().any(|part| part == "params")
                        })
                        && csharp_method_is_static(candidate)
                }),
        );
    }
    (candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone())
}

fn resolve_csharp_static_type_imported_method(
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    bindings: &[CSharpStaticTypeImportBinding],
    method_name: &str,
    call_arity: usize,
) -> Option<String> {
    let mut candidates = Vec::new();
    for binding in bindings {
        let type_candidates = semantic_path_index
            .get(&binding.semantic_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .count();
        if type_candidates != 1 {
            return None;
        }
        let target_path = format!("{}::{method_name}", binding.semantic_type_path);
        candidates.extend(
            semantic_path_index
                .get(&target_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.node_kind == "method_declaration"
                        && candidate.parameters.len() == call_arity
                        && !candidate.parameters.iter().any(|parameter| {
                            parameter.split_whitespace().any(|part| part == "params")
                        })
                        && csharp_method_is_static(candidate)
                }),
        );
    }
    (candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone())
}

fn resolve_csharp_imported_static_method(
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    binding: &CSharpTypeAliasBinding,
    method_name: &str,
    call_arity: usize,
) -> Option<String> {
    let type_candidates = semantic_path_index
        .get(&binding.semantic_type_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
        .count();
    if type_candidates != 1 {
        return None;
    }
    let target_path = format!("{}::{method_name}", binding.semantic_type_path);
    resolve_csharp_candidate(
        raw_symbols,
        semantic_path_index,
        &target_path,
        None,
        call_arity,
        CSharpCandidateRequirements {
            node_kind: "method_declaration",
            require_static: true,
            require_instance: false,
            require_same_file: false,
        },
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java superclass resolution inputs explicit"
)]
fn resolve_java_simple_super_method_reference(
    source_symbol: &IndexedSymbol,
    method_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(superclass_path) = java_simple_superclass_path(
        source_symbol,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    resolve_java_inherited_method_from_type_path(
        &superclass_path,
        method_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        call_arity,
        deadline,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java superclass resolution inputs explicit"
)]
fn resolve_java_inherited_method_from_type_path(
    initial_type_path: &str,
    method_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let mut visited_type_paths = BTreeSet::new();
    let mut current_type_path = initial_type_path.to_string();
    loop {
        if let Some(deadline) = deadline {
            deadline.check("resolving Java inherited method")?;
        }
        if !visited_type_paths.insert(current_type_path.clone()) {
            return Ok(None);
        }
        let target_path = format!("{current_type_path}::{method_name}");
        let declared_candidates = semantic_path_index
            .get(&target_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.node_kind == "method_declaration"
            })
            .collect::<Vec<_>>();
        if !declared_candidates.is_empty() {
            let candidates = declared_candidates
                .into_iter()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.parameters.len() == call_arity
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

        let class_candidates = semantic_path_index
            .get(&current_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| raw_symbols[*index].node_kind == "class_declaration")
            .collect::<Vec<_>>();
        let [class_index] = class_candidates.as_slice() else {
            return Ok(None);
        };
        let Some(superclass_path) = java_simple_superclass_path_for_class(
            &raw_symbols[*class_index],
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        current_type_path = superclass_path;
    }
}

enum JavaDefaultInterfaceMethodResolution {
    Resolved(String),
    NoMethod,
    Blocked,
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java direct interface resolution inputs explicit"
)]
fn resolve_java_direct_interface_default_method_reference(
    source_symbol: &IndexedSymbol,
    method_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(interface_paths) = java_resolved_direct_interface_paths(
        source_symbol,
        method_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let mut resolved_symbol_id = None;
    for interface_path in interface_paths {
        match resolve_java_default_interface_method_from_type_path(
            &interface_path,
            method_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            JavaDefaultInterfaceMethodResolution::Resolved(symbol_id) => {
                if resolved_symbol_id.replace(symbol_id).is_some() {
                    return Ok(None);
                }
            }
            JavaDefaultInterfaceMethodResolution::NoMethod => {}
            JavaDefaultInterfaceMethodResolution::Blocked => return Ok(None),
        }
    }
    Ok(resolved_symbol_id)
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java interface inheritance resolution inputs explicit"
)]
fn resolve_java_default_interface_method_from_type_path(
    initial_interface_path: &str,
    method_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaDefaultInterfaceMethodResolution> {
    let mut visited_interface_paths = BTreeSet::new();
    let mut current_interface_path = initial_interface_path.to_string();
    loop {
        if let Some(deadline) = deadline {
            deadline.check("resolving Java default interface method")?;
        }
        if !visited_interface_paths.insert(current_interface_path.clone()) {
            return Ok(JavaDefaultInterfaceMethodResolution::Blocked);
        }
        let target_path = format!("{current_interface_path}::{method_name}");
        let declared_candidates = semantic_path_index
            .get(&target_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| raw_symbols[*index].node_kind == "method_declaration")
            .collect::<Vec<_>>();
        if !declared_candidates.is_empty() {
            let candidates = declared_candidates
                .into_iter()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate
                        .signature
                        .as_deref()
                        .is_some_and(java_method_signature_is_default)
                        && candidate.parameters.len() == call_arity
                        && !candidate
                            .parameters
                            .iter()
                            .any(|parameter| parameter.contains("..."))
                })
                .collect::<Vec<_>>();
            return Ok(match candidates.as_slice() {
                [candidate_index] => JavaDefaultInterfaceMethodResolution::Resolved(
                    raw_symbols[*candidate_index].symbol_id.clone(),
                ),
                _ => JavaDefaultInterfaceMethodResolution::Blocked,
            });
        }
        let interface_candidates = semantic_path_index
            .get(&current_interface_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| raw_symbols[*index].node_kind == "interface_declaration")
            .collect::<Vec<_>>();
        let [interface_index] = interface_candidates.as_slice() else {
            return Ok(JavaDefaultInterfaceMethodResolution::Blocked);
        };
        let source_interface = &raw_symbols[*interface_index];
        let Some(parent_interface_path) = java_unique_direct_parent_interface_path(
            source_interface,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(
                if java_interface_has_no_direct_parents(source_interface, file_overrides, deadline)?
                    == Some(true)
                {
                    JavaDefaultInterfaceMethodResolution::NoMethod
                } else {
                    JavaDefaultInterfaceMethodResolution::Blocked
                },
            );
        };
        current_interface_path = parent_interface_path;
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java interface inheritance resolution inputs explicit"
)]
fn java_unique_direct_parent_interface_path(
    source_interface: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if source_interface.node_kind != "interface_declaration" {
        return Ok(None);
    }
    let path = Path::new(&source_interface.file_path);
    let normalized_path = normalize_path(path);
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalized_path))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    let document = parse_document(path, &source)?;
    let mut stack = vec![document.tree.root_node()];
    let mut interface_references = None;
    while let Some(node) = stack.pop() {
        if let Some(deadline) = deadline {
            deadline.check("locating Java parent interface")?;
        }
        if node.kind() == "interface_declaration"
            && (node.start_byte(), node.end_byte()) == source_interface.byte_range
        {
            interface_references = java_direct_interface_references_for_declaration(node, &source)?;
            break;
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    let Some(interface_references) = interface_references else {
        return Ok(None);
    };
    let [interface_reference] = interface_references.as_slice() else {
        return Ok(None);
    };
    resolve_java_direct_interface_target_path(
        &source_interface.file_path,
        source_interface.scope_path.as_deref(),
        interface_reference,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

fn java_interface_has_no_direct_parents(
    source_interface: &IndexedSymbol,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<bool>> {
    if source_interface.node_kind != "interface_declaration" {
        return Ok(None);
    }
    let path = Path::new(&source_interface.file_path);
    let normalized_path = normalize_path(path);
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalized_path))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    let document = parse_document(path, &source)?;
    let mut stack = vec![document.tree.root_node()];
    while let Some(node) = stack.pop() {
        if let Some(deadline) = deadline {
            deadline.check("locating Java parent interface")?;
        }
        if node.kind() == "interface_declaration"
            && (node.start_byte(), node.end_byte()) == source_interface.byte_range
        {
            let mut cursor = node.walk();
            return Ok(Some(
                !node
                    .named_children(&mut cursor)
                    .any(|child| child.kind() == "extends_interfaces"),
            ));
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    Ok(None)
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java direct interface resolution inputs explicit"
)]
fn java_resolved_direct_interface_paths(
    source_symbol: &IndexedSymbol,
    method_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<Vec<String>>> {
    let Some(scope_path) = source_symbol.scope_path.as_deref() else {
        return Ok(None);
    };
    let path = Path::new(&source_symbol.file_path);
    let normalized_path = normalize_path(path);
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalized_path))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    let document = parse_document(path, &source)?;
    let mut stack = vec![document.tree.root_node()];
    let mut interface_references = None;
    while let Some(node) = stack.pop() {
        if let Some(deadline) = deadline {
            deadline.check("locating Java direct interface")?;
        }
        if (node.kind() == "method_declaration" || node.kind() == "constructor_declaration")
            && (node.start_byte(), node.end_byte()) == source_symbol.byte_range
        {
            let mut ancestor = node.parent();
            while let Some(candidate) = ancestor {
                if candidate.kind() == "class_declaration" {
                    if let Some(superclass) = candidate.child_by_field_name("superclass") {
                        let Some(superclass_reference) =
                            java_direct_superclass_reference(superclass, &source)?
                        else {
                            return Ok(None);
                        };
                        let enclosing_scope_path =
                            scope_path.rsplit_once("::").map(|(parent, _)| parent);
                        let Some(superclass_path) = resolve_java_direct_superclass_target_path(
                            &source_symbol.file_path,
                            enclosing_scope_path,
                            &superclass_reference,
                            raw_symbols,
                            semantic_path_index,
                            file_overrides,
                            java_import_contexts_by_file,
                            deadline,
                        )?
                        else {
                            return Ok(None);
                        };
                        if java_class_hierarchy_defines_method_from_type_path(
                            &superclass_path,
                            method_name,
                            raw_symbols,
                            semantic_path_index,
                            file_overrides,
                            java_import_contexts_by_file,
                            deadline,
                        )? != Some(false)
                        {
                            return Ok(None);
                        }
                    }
                    interface_references =
                        java_direct_interface_references_for_declaration(candidate, &source)?;
                    break;
                }
                ancestor = candidate.parent();
            }
            break;
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    let Some(interface_references) = interface_references else {
        return Ok(None);
    };
    let enclosing_scope_path = scope_path.rsplit_once("::").map(|(parent, _)| parent);
    let mut interface_paths = Vec::new();
    for interface_reference in interface_references {
        let Some(interface_path) = resolve_java_direct_interface_target_path(
            &source_symbol.file_path,
            enclosing_scope_path,
            &interface_reference,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        interface_paths.push(interface_path);
    }
    Ok((!interface_paths.is_empty()).then_some(interface_paths))
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java superclass hierarchy resolution inputs explicit"
)]
fn java_class_hierarchy_defines_method_from_type_path(
    initial_type_path: &str,
    method_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<bool>> {
    let mut visited_type_paths = BTreeSet::new();
    let mut current_type_path = initial_type_path.to_string();
    loop {
        if let Some(deadline) = deadline {
            deadline.check("checking Java superclass methods")?;
        }
        if !visited_type_paths.insert(current_type_path.clone()) {
            return Ok(None);
        }
        let target_path = format!("{current_type_path}::{method_name}");
        if semantic_path_index
            .get(&target_path)
            .into_iter()
            .flatten()
            .copied()
            .any(|index| raw_symbols[index].node_kind == "method_declaration")
        {
            return Ok(Some(true));
        }
        let class_candidates = semantic_path_index
            .get(&current_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| raw_symbols[*index].node_kind == "class_declaration")
            .collect::<Vec<_>>();
        let [class_index] = class_candidates.as_slice() else {
            return Ok(None);
        };
        let Some(superclass_path) = java_simple_superclass_path_for_class(
            &raw_symbols[*class_index],
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(Some(false));
        };
        current_type_path = superclass_path;
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java nested static resolution inputs explicit"
)]
fn resolve_java_nested_static_method_reference(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((type_reference, method_name)) = reference_name.rsplit_once('.') else {
        return Ok(None);
    };
    if type_reference.is_empty()
        || method_name.is_empty()
        || !type_reference.contains('.')
        || matches!(type_reference, "this" | "super")
    {
        return Ok(None);
    }
    let Some(scope_path) = source_symbol.scope_path.as_deref() else {
        return Ok(None);
    };
    let enclosing_scope_path = scope_path.rsplit_once("::").map(|(parent, _)| parent);
    let Some(type_path) = resolve_java_direct_superclass_target_path(
        &source_symbol.file_path,
        enclosing_scope_path,
        &JavaDirectSuperclassReference::Qualified(type_reference.to_string()),
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let target_path = format!("{type_path}::{method_name}");
    let candidates = semantic_path_index
        .get(&target_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "method_declaration"
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
    Ok((candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone()))
}

fn resolve_java_same_package_static_method_reference(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    call_arity: usize,
) -> Option<String> {
    let (type_name, method_name) = reference_name.split_once('.')?;
    if type_name.is_empty()
        || method_name.is_empty()
        || method_name.contains('.')
        || matches!(type_name, "this" | "super")
    {
        return None;
    }
    let scope_path = source_symbol.scope_path.as_deref()?;
    let type_path = scope_path.rsplit_once("::").map_or_else(
        || type_name.to_string(),
        |(enclosing_scope_path, _)| format!("{enclosing_scope_path}::{type_name}"),
    );
    let type_candidates = semantic_path_index
        .get(&type_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            matches!(
                raw_symbols[*index].node_kind.as_str(),
                "class_declaration" | "interface_declaration"
            )
        })
        .collect::<Vec<_>>();
    if type_candidates.len() != 1 {
        return None;
    }

    let target_path = format!("{type_path}::{method_name}");
    let candidates = semantic_path_index
        .get(&target_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "method_declaration"
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
    (candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone())
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java superclass resolution inputs explicit"
)]
fn resolve_java_direct_superclass_target_path(
    source_file_path: &str,
    enclosing_scope_path: Option<&str>,
    superclass_reference: &JavaDirectSuperclassReference,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    resolve_java_direct_type_target_path(
        source_file_path,
        enclosing_scope_path,
        superclass_reference,
        "class_declaration",
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java direct interface resolution inputs explicit"
)]
fn resolve_java_direct_interface_target_path(
    source_file_path: &str,
    enclosing_scope_path: Option<&str>,
    interface_reference: &JavaDirectSuperclassReference,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    resolve_java_direct_type_target_path(
        source_file_path,
        enclosing_scope_path,
        interface_reference,
        "interface_declaration",
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java direct type resolution inputs explicit"
)]
fn resolve_java_direct_type_target_path(
    source_file_path: &str,
    enclosing_scope_path: Option<&str>,
    type_reference: &JavaDirectSuperclassReference,
    declaration_kind: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    match type_reference {
        JavaDirectSuperclassReference::Simple(superclass_name) => {
            let mut scope_path = enclosing_scope_path;
            while let Some(current_scope_path) = scope_path {
                let local_path = format!("{current_scope_path}::{superclass_name}");
                let local_candidates = semantic_path_index
                    .get(&local_path)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|index| raw_symbols[*index].node_kind == declaration_kind)
                    .collect::<Vec<_>>();
                match local_candidates.as_slice() {
                    [_] => return Ok(Some(local_path)),
                    [] => {}
                    _ => return Ok(None),
                }
                scope_path = current_scope_path
                    .rsplit_once("::")
                    .map(|(parent, _)| parent);
            }

            let Some(binding) = resolve_java_type_import_binding_for_name(
                source_file_path,
                superclass_name,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            let candidates = semantic_path_index
                .get(&binding.semantic_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.file_path == binding.source_path
                        && candidate.node_kind == declaration_kind
                })
                .collect::<Vec<_>>();
            Ok((candidates.len() == 1).then_some(binding.semantic_path))
        }
        JavaDirectSuperclassReference::Qualified(qualified_name) => {
            let qualified_path = qualified_name.replace('.', "::");
            let mut scope_path = enclosing_scope_path;
            while let Some(current_scope_path) = scope_path {
                let semantic_path = format!("{current_scope_path}::{qualified_path}");
                let candidates = semantic_path_index
                    .get(&semantic_path)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|index| raw_symbols[*index].node_kind == declaration_kind)
                    .collect::<Vec<_>>();
                match candidates.as_slice() {
                    [_] => return Ok(Some(semantic_path)),
                    [] => {}
                    _ => return Ok(None),
                }
                scope_path = current_scope_path
                    .rsplit_once("::")
                    .map(|(parent, _)| parent);
            }
            let candidates = semantic_path_index
                .get(&qualified_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| raw_symbols[*index].node_kind == declaration_kind)
                .collect::<Vec<_>>();
            match candidates.as_slice() {
                [_] => return Ok(Some(qualified_path)),
                [] => {}
                _ => return Ok(None),
            }

            let Some((outer_type_name, nested_type_path)) = qualified_name.split_once('.') else {
                return Ok(None);
            };
            let Some(binding) = resolve_java_type_import_binding_for_name(
                source_file_path,
                outer_type_name,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            let semantic_path = format!(
                "{}::{}",
                binding.semantic_path,
                nested_type_path.replace('.', "::")
            );
            let candidates = semantic_path_index
                .get(&semantic_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.file_path == binding.source_path
                        && candidate.node_kind == declaration_kind
                })
                .collect::<Vec<_>>();
            Ok((candidates.len() == 1).then_some(semantic_path))
        }
    }
}

fn java_simple_superclass_path(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(scope_path) = source_symbol.scope_path.as_deref() else {
        return Ok(None);
    };
    let path = Path::new(&source_symbol.file_path);
    let normalized_path = normalize_path(path);
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalized_path))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    let document = parse_document(path, &source)?;
    let mut stack = vec![document.tree.root_node()];
    let mut superclass_reference = None;
    while let Some(node) = stack.pop() {
        if let Some(deadline) = deadline {
            deadline.check("locating Java superclass")?;
        }
        if (node.kind() == "method_declaration" || node.kind() == "constructor_declaration")
            && (node.start_byte(), node.end_byte()) == source_symbol.byte_range
        {
            let mut ancestor = node.parent();
            while let Some(candidate) = ancestor {
                if candidate.kind() == "class_declaration" {
                    let Some(superclass) = candidate.child_by_field_name("superclass") else {
                        return Ok(None);
                    };
                    superclass_reference = java_direct_superclass_reference(superclass, &source)?;
                    break;
                }
                ancestor = candidate.parent();
            }
            break;
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    let Some(superclass_reference) = superclass_reference else {
        return Ok(None);
    };
    let enclosing_scope_path = scope_path.rsplit_once("::").map(|(parent, _)| parent);
    resolve_java_direct_superclass_target_path(
        &source_symbol.file_path,
        enclosing_scope_path,
        &superclass_reference,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}
fn java_simple_superclass_path_for_class(
    source_class: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if source_class.node_kind != "class_declaration" {
        return Ok(None);
    }
    let path = Path::new(&source_class.file_path);
    let normalized_path = normalize_path(path);
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalized_path))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    let document = parse_document(path, &source)?;
    let mut stack = vec![document.tree.root_node()];
    let mut superclass_reference = None;
    while let Some(node) = stack.pop() {
        if let Some(deadline) = deadline {
            deadline.check("locating Java superclass")?;
        }
        if node.kind() == "class_declaration"
            && (node.start_byte(), node.end_byte()) == source_class.byte_range
        {
            let Some(superclass) = node.child_by_field_name("superclass") else {
                return Ok(None);
            };
            superclass_reference = java_direct_superclass_reference(superclass, &source)?;
            break;
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    let Some(superclass_reference) = superclass_reference else {
        return Ok(None);
    };
    resolve_java_direct_superclass_target_path(
        &source_class.file_path,
        source_class.scope_path.as_deref(),
        &superclass_reference,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

fn resolve_java_same_file_super_constructor_reference(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if source_symbol.node_kind != "constructor_declaration" {
        return Ok(None);
    }
    let Some(superclass_path) = java_simple_superclass_path(
        source_symbol,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(superclass_name) = superclass_path.rsplit("::").next() else {
        return Ok(None);
    };
    let target_path = format!("{superclass_path}::{superclass_name}");
    let candidates = semantic_path_index
        .get(&target_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "constructor_declaration"
                && candidate.parameters.len() == call_arity
                && !candidate
                    .parameters
                    .iter()
                    .any(|parameter| parameter.contains("..."))
        })
        .collect::<Vec<_>>();
    Ok((candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone()))
}
fn resolve_java_imported_static_method(
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    binding: &JavaImportBinding,
    method_name: &str,
    call_arity: usize,
) -> Option<String> {
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
    (candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone())
}

fn java_method_signature_is_static(signature: &str) -> bool {
    signature.split_whitespace().any(|token| token == "static")
}

fn java_method_signature_is_default(signature: &str) -> bool {
    signature.split_whitespace().any(|token| token == "default")
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::{Duration, Instant};

    use super::resolve_dependencies_for_symbol_with_deadline;
    use crate::symbol_dependency::rust::RustOutOfLineModuleContext;
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
            &RustOutOfLineModuleContext::default(),
            &mut std::collections::BTreeMap::new(),
            &mut std::collections::BTreeMap::new(),
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
