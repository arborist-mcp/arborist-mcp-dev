use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::Path;

use anyhow::Result;

use super::super::c::{
    CIncludeContext, CIncludeTargetsCache, c_include_context_for_file_with_overrides_and_deadline,
};
use super::super::csharp::{
    CSharpBaseTypeBinding, CSharpGlobalImportContext, CSharpImportContext, CSharpInterfaceParents,
    CSharpNamespaceImportBinding, CSharpReceiverTypeBindings, CSharpStaticTypeImportBinding,
    CSharpTypeAliasBinding, csharp_array_component_spelling_at_depth,
    csharp_global_base_type_alias_is_ambiguous, csharp_global_type_alias_name_is_ambiguous,
    csharp_interface_parent_bindings_for_interface, csharp_member_type_bindings_for_type,
    csharp_receiver_type_bindings_for_function, csharp_type_alias_name_is_ambiguous_for_reference,
    csharp_type_alias_name_is_declared_for_reference, csharp_type_parameter_names_for_type,
    resolve_csharp_base_type_binding_for_reference,
    resolve_csharp_declared_type_binding_for_reference, resolve_csharp_global_base_type_alias,
    resolve_csharp_global_namespace_imports_for_reference,
    resolve_csharp_global_nested_type_alias_binding_for_reference,
    resolve_csharp_global_static_type_imports_for_reference,
    resolve_csharp_global_type_alias_binding_for_reference,
    resolve_csharp_namespace_imports_for_reference,
    resolve_csharp_nested_type_alias_binding_for_reference,
    resolve_csharp_static_type_imports_for_reference, resolve_csharp_type_alias_binding_for_name,
    resolve_csharp_type_alias_binding_for_reference, substitute_csharp_type_parameters,
};
use super::super::go::{
    GoImportContext, go_package_name_for_source_file, resolve_go_import_binding_for_reference,
};
use super::super::java::{
    JavaImportBinding, JavaImportContext, JavaReceiverTypeBindings, java_array_type_component_name,
    java_dotted_type_name, java_receiver_type_bindings_for_function,
    resolve_java_import_binding_for_reference,
    resolve_java_static_method_import_binding_for_reference,
    resolve_java_type_import_binding_for_name,
};
use super::super::javascript::{
    JavaScriptImportBinding, JavaScriptImportContext,
    resolve_javascript_module_default_export_name,
    resolve_javascript_named_import_binding_for_reference,
    resolve_javascript_namespace_member_binding, resolve_javascript_namespace_object_call_binding,
};
use super::super::kotlin::{
    KotlinImportContext, KotlinReceiverTypeBindings, kotlin_array_type_component_name,
    kotlin_dotted_type_name, kotlin_receiver_type_bindings_for_function,
    resolve_kotlin_import_binding_for_reference,
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
    JavaDirectSuperclassReference, LanguageRegistry, detect_language,
    java_direct_interface_references_for_declaration, java_direct_superclass_reference, node_text,
    normalize_path, parse_document, read_source,
};
use crate::model::LanguageId;
use crate::patching::resolve_local_python_imported_symbol;
use crate::symbol_index_model::JavaScriptReferenceDetails;
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
    include_targets_cache: &mut CIncludeTargetsCache,
    javascript_import_contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    rust_out_of_line_module_context: &RustOutOfLineModuleContext,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
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
        include_targets_cache,
        javascript_import_contexts_by_file,
        go_import_contexts_by_file,
        rust_out_of_line_module_context,
        java_import_contexts_by_file,
        kotlin_import_contexts_by_file,
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
    include_targets_cache: &mut CIncludeTargetsCache,
    javascript_import_contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    rust_out_of_line_module_context: &RustOutOfLineModuleContext,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
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
                ReferenceLanguageDetails::JavaScript(_) => (false, false, false),
            };
        let rust_import_root = match &reference.language_details {
            ReferenceLanguageDetails::Rust(details) => details.import_root.as_ref(),
            _ => None,
        };
        let go_reference_details = match &reference.language_details {
            ReferenceLanguageDetails::Go(details) => Some(details),
            _ => None,
        };
        let javascript_reference_details = match &reference.language_details {
            ReferenceLanguageDetails::JavaScript(details) => Some(details),
            _ => None,
        };
        // Bare JavaScript/TypeScript identifier references are recorded only
        // for call expressions, so a non-empty arity set signals a direct
        // call such as `ns(...)`. The general call-arity path stays C++ and
        // JVM-only; the namespace-object call branch below uses this signal
        // instead of CallResolutionContext.
        let javascript_direct_call = matches!(
            language_id,
            Some(LanguageId::JavaScript | LanguageId::TypeScript | LanguageId::Tsx)
        ) && reference
            .call_arities
            .as_ref()
            .is_some_and(|arities| !arities.is_empty());
        if matches!(
            language_id,
            Some(LanguageId::Cpp | LanguageId::Java | LanguageId::CSharp | LanguageId::Kotlin)
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
                    javascript_reference_details,
                    language_id,
                    call_context,
                    javascript_direct_call,
                    symbol,
                    raw_symbols,
                    name_index,
                    semantic_path_index,
                    file_overrides,
                    include_contexts_by_file,
                    include_targets_cache,
                    javascript_import_contexts_by_file,
                    go_import_contexts_by_file,
                    rust_out_of_line_module_context,
                    java_import_contexts_by_file,
                    kotlin_import_contexts_by_file,
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
            javascript_reference_details,
            language_id,
            CallResolutionContext::non_call(),
            javascript_direct_call,
            symbol,
            raw_symbols,
            name_index,
            semantic_path_index,
            file_overrides,
            include_contexts_by_file,
            include_targets_cache,
            javascript_import_contexts_by_file,
            go_import_contexts_by_file,
            rust_out_of_line_module_context,
            java_import_contexts_by_file,
            kotlin_import_contexts_by_file,
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
    javascript_reference_details: Option<&JavaScriptReferenceDetails>,
    language_id: Option<LanguageId>,
    call_context: CallResolutionContext,
    javascript_direct_call: bool,
    source_symbol: &'a IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    include_contexts_by_file: &mut HashMap<&'a str, Option<CIncludeContext>>,
    include_targets_cache: &mut CIncludeTargetsCache,
    javascript_import_contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    go_import_contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    rust_out_of_line_module_context: &RustOutOfLineModuleContext,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
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
        // A `base.`-rooted member chain such as `base.member.helper(...)` or
        // `base.inner().helper(...)` walks each intermediate hop on the unique
        // class/record base type before dispatching the final member; unknown
        // or unresolvable hops fail closed instead of falling through to
        // static type calls. Plain `base.method()` calls keep the
        // direct-base-chain contract below.
        if let Some(chain) = reference_name.strip_prefix("base.")
            && !chain.is_empty()
            && chain.contains('.')
        {
            return resolve_csharp_base_member_chain_call(
                source_symbol,
                chain,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                call_arity,
                deadline,
            );
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
                false,
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
        // A `this.`-rooted member chain such as `this.member.helper(...)`
        // walks each intermediate hop on the enclosing type before dispatching
        // the final member; unknown or unresolvable hops fail closed instead
        // of falling through to static type calls. Plain `this.method()` calls
        // keep the same-type contract below.
        if let Some(chain) = reference_name.strip_prefix("this.")
            && !chain.is_empty()
            && chain.contains('.')
        {
            return resolve_csharp_this_member_chain_call(
                source_symbol,
                chain,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                call_arity,
                deadline,
            );
        }
        // A dotted call whose leading receiver names a locally bound value is
        // an instance call on that value's declared type; it shadows any
        // same-named type. Bound-but-unresolvable receivers fail closed
        // instead of falling through to the static type-call paths below.
        match resolve_csharp_instance_receiver_call(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            CSharpInstanceReceiverResolution::Resolved(symbol_id) => {
                return Ok(Some(symbol_id));
            }
            CSharpInstanceReceiverResolution::Blocked => return Ok(None),
            CSharpInstanceReceiverResolution::NoBinding => {}
        }
        // A dotted call rooted at a bare inherited member such as
        // `MATRIX[0,0].entry.Run(1)` or `holder.entry.Run(1)` where the
        // leading field/property (with an optional element-access suffix) is
        // declared on a class/record ancestor of the enclosing type resolves
        // the root through the same inherited-then-static-imported rules as
        // bare `var` initializer and `foreach` roots, walks any remaining
        // hops, and dispatches the final member as an instance call; it runs
        // before the static-imported and static type-call paths so an
        // inherited member shadows a same-named static import or type call.
        if let Some(symbol_id) = resolve_csharp_bare_inherited_member_chain_call(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            return Ok(Some(symbol_id));
        }

        // A bare factory-call root with an element-access suffix such as
        // `makeItems()[0].helper(...)` resolves the leading call through the
        // same factory rules as a `var` initializer and dispatches the
        // trailing member chain on the factory return array's element
        // component type; unknown or arity-mismatched factories, primitive or
        // multi-dimensional return arrays, and multi-dimensional element
        // access fail closed.
        if let Some(symbol_id) = resolve_csharp_bare_factory_array_member_chain(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            return Ok(Some(symbol_id));
        }
        // A dotted call rooted at a static type-qualified member such as
        // `Util.STATIC_HELPER.entry.Run(1)`,
        // `global::Demo.Util.STATIC_HELPER.entry.Run(1)`, or
        // `Util.MakeHelper().entry.Run(1)` walks the static field/factory
        // root and any instance hops before dispatching the final member as
        // an instance call. It runs before the constructed-receiver path so a
        // static factory call on a type is not mistaken for a constructor
        // marker; a reference that is not a resolvable static-member chain
        // falls through to the constructed-receiver and static type-call
        // paths below.
        if let Some(symbol_id) = resolve_csharp_static_field_member_chain_call(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            return Ok(Some(symbol_id));
        }
        // A dotted call rooted at a static-imported member such as
        // `STATIC_HELPER.entry.Run(1)` with `using static Demo.Util;` walks
        // the static member root and any instance hops before dispatching the
        // final member as an instance call; a reference that is not a
        // resolvable static-imported member chain falls through to the
        // constructed-receiver and static type-call paths below.
        if let Some(symbol_id) = resolve_csharp_static_imported_member_chain_call(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            return Ok(Some(symbol_id));
        }
        // A dotted call rooted at a bare factory method call such as
        // `MakeHelper().entry.Run(1)` or `MakeHelper().Run(1)` dispatches the
        // leading arity-matched factory method on the enclosing type, the
        // unique base chain, or a static-imported type before walking any
        // instance hops and dispatching the final member as an instance call.
        // It runs before the constructed-receiver path so a bare factory call
        // is not mistaken for a constructor marker; a reference that is not a
        // resolvable bare-factory chain falls through to the
        // constructed-receiver and static type-call paths below.
        if let Some(symbol_id) = resolve_csharp_direct_bare_factory_member_chain_call(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            return Ok(Some(symbol_id));
        }
        // A receiver spelling such as `Helper().Run` names a fresh constructor
        // call on a constructed type; it dispatches as an instance call and
        // never falls through to the static type-call paths. Malformed or
        // unresolvable constructed receivers fail closed.
        match resolve_csharp_constructor_receiver_call(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            CSharpConstructorReceiverResolution::Resolved(symbol_id) => {
                return Ok(Some(symbol_id));
            }
            CSharpConstructorReceiverResolution::Blocked => return Ok(None),
            CSharpConstructorReceiverResolution::NotConstructorReceiver => {}
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
        if let Some((nested_type_path, method_name, binding)) =
            resolve_csharp_nested_type_alias_binding_for_reference(
                &source_symbol.file_path,
                reference_name,
                source_namespace_path,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        {
            let Some((alias_name, _)) = reference_name.split_once('.') else {
                return Ok(None);
            };
            if !csharp_alias_name_is_unshadowed(alias_name, source_symbol, raw_symbols) {
                return Ok(None);
            }
            return resolve_csharp_imported_nested_static_method(
                source_symbol,
                raw_symbols,
                semantic_path_index,
                &binding,
                &nested_type_path,
                &method_name,
                call_arity,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        if let Some(csharp_global_import_context) = csharp_global_import_context
            && let Some((nested_type_path, method_name, binding)) =
                resolve_csharp_global_nested_type_alias_binding_for_reference(
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
            return resolve_csharp_imported_nested_static_method(
                source_symbol,
                raw_symbols,
                semantic_path_index,
                &binding,
                &nested_type_path,
                &method_name,
                call_arity,
                Some(csharp_global_import_context),
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
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
        if let Some((nested_type_path, first_type_name, method_name)) =
            csharp_nested_type_static_reference_parts(reference_name)
            && csharp_nested_type_root_is_unshadowed(first_type_name, source_symbol, raw_symbols)
        {
            let mut namespace_imports = resolve_csharp_namespace_imports_for_reference(
                &source_symbol.file_path,
                first_type_name,
                source_namespace_path,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
            if let Some(csharp_global_import_context) = csharp_global_import_context {
                namespace_imports.extend(resolve_csharp_global_namespace_imports_for_reference(
                    first_type_name,
                    csharp_global_import_context,
                ));
            }
            if let Some(symbol_id) = resolve_csharp_namespace_imported_nested_static_method(
                raw_symbols,
                semantic_path_index,
                &namespace_imports,
                nested_type_path,
                method_name,
                call_arity,
            ) {
                return Ok(Some(symbol_id));
            }
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
            return resolve_csharp_imported_static_method(
                source_symbol,
                raw_symbols,
                semantic_path_index,
                &binding,
                &method_name,
                call_arity,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
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
            return resolve_csharp_imported_static_method(
                source_symbol,
                raw_symbols,
                semantic_path_index,
                &binding,
                &method_name,
                call_arity,
                Some(csharp_global_import_context),
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        if let Some(target_path) =
            csharp_simple_type_static_target_path(reference_name, source_symbol, raw_symbols)
            && let Some((type_path, method_name)) = target_path.rsplit_once("::")
        {
            // A type-qualified static method such as `Caller.Make()` may be
            // declared directly on the qualified type or inherited through
            // its unique class/record ancestor chain; the nearest declaring
            // ancestor pins the target, so an inherited static factory over a
            // constructed generic base resolves to the declaring base method.
            return resolve_csharp_type_qualified_static_method(
                source_symbol,
                type_path,
                method_name,
                call_arity,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
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
        if let Some(target_path) = csharp_namespace_absolute_dotted_static_target_path(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? && let Some((type_path, method_name)) = target_path.rsplit_once("::")
        {
            // A type-qualified static method on a namespace-absolute dotted
            // type such as `Other.Derived.Make()` or
            // `Other.Derived<HelperA>.Make()` (with the generic argument
            // lists stripped by the caller) resolves through the
            // receiver-type binding rules, and the nearest declaring
            // class/record ancestor pins the target like the simple-type
            // branch above.
            return resolve_csharp_type_qualified_static_method(
                source_symbol,
                type_path,
                method_name,
                call_arity,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
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
        if let Some(base_type_binding) = csharp_source_base_type_binding(
            source_symbol,
            raw_symbols,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            // A non-static caller can dispatch an instance base method by
            // simple name; both static and non-static callers can dispatch a
            // static base method by simple name, so the instance attempt runs
            // first and the static attempt follows for every caller. An
            // explicit `this.` receiver never dispatches a static method and
            // fails closed after the instance attempt.
            if !csharp_method_is_static(source_symbol)
                && let Some(target_path) = csharp_base_method_target_path(
                    source_symbol,
                    raw_symbols,
                    semantic_path_index,
                    &base_type_binding,
                    method_name,
                    call_arity,
                    false,
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
            if !has_explicit_this_receiver
                && let Some(target_path) = csharp_base_method_target_path(
                    source_symbol,
                    raw_symbols,
                    semantic_path_index,
                    &base_type_binding,
                    method_name,
                    call_arity,
                    true,
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
                        require_static: true,
                        require_instance: false,
                        require_same_file: false,
                    },
                ));
            }
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
    if language_id == Some(LanguageId::Kotlin) {
        return resolve_kotlin_reference_with_deadline(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            call_context,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
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
        if let Some(chain) = reference_name.strip_prefix("super.") {
            if chain.is_empty() {
                return Ok(None);
            }
            // A `super.`-rooted receiver chain such as `super.member.helper(...)`
            // or `super.inner().helper(...)` dispatches on the unique local-source
            // direct superclass type path through the same member-chain rules as
            // bound receivers; unknown or unresolvable hops fail closed. Plain
            // `super.method()` calls keep the direct-base-chain contract below.
            if chain.contains('.') {
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
                return resolve_java_member_chain_from_type_path(
                    &superclass_path,
                    chain,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    java_import_contexts_by_file,
                    call_arity,
                    deadline,
                );
            }
            return resolve_java_simple_super_method_reference(
                source_symbol,
                chain,
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
        // A dotted call whose leading receiver names a locally bound value is
        // an instance call on that value's declared type; it shadows any
        // same-named type. Bound-but-unresolvable receivers fail closed instead
        // of falling through to the static type-call paths below.
        match resolve_java_instance_receiver_call(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            JavaInstanceReceiverResolution::Resolved(symbol_id) => return Ok(Some(symbol_id)),
            JavaInstanceReceiverResolution::Blocked => return Ok(None),
            JavaInstanceReceiverResolution::NoBinding => {}
        }
        // A constructor-call receiver such as `new Foo().helper(...)` is
        // recorded as `Foo().helper` and dispatches on the constructed type.
        if let Some(symbol_id) = resolve_java_constructor_receiver_call(
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
        // A dotted reference whose leading segment is a bare factory-call hop
        // such as `makeFoo()` or `MakeHelper()` in `makeFoo().helper(...)` or
        // `MakeHelper().entry.helper(...)` resolves the leading call through
        // the same factory rules as a `var` initializer (a unique same-type
        // method or explicit static-method import with matching non-varargs
        // arity and a usable declared return type) and dispatches the
        // trailing member chain on the factory's declared type; unknown,
        // ambiguous, or arity-mismatched factories and unresolvable hops fail
        // closed.
        if let Some((root_spelling, member_chain)) = reference_name.split_once('.')
            && !root_spelling.is_empty()
            && !member_chain.is_empty()
            && let Some((function_name, function_arity)) =
                java_method_call_hop_spelling(root_spelling)
            && !function_name.contains('.')
            && let Some(root_type_path) = resolve_java_initializer_type_path(
                source_symbol,
                &function_name,
                function_arity,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
            && let Some(symbol_id) = resolve_java_member_chain_from_type_path(
                &root_type_path,
                member_chain,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                call_arity,
                deadline,
            )?
        {
            return Ok(Some(symbol_id));
        }
        // A bare factory-call root with an element-access suffix such as
        // `makeItems()[0].helper(...)` resolves the leading call through the
        // same factory rules as a `var` initializer and dispatches the
        // trailing member chain on the factory return array's element
        // component type; unknown or arity-mismatched factories, primitive or
        // multi-dimensional return arrays, and multi-dimensional element
        // access fail closed.
        if let Some(symbol_id) = resolve_java_bare_factory_array_member_chain(
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

        // A `this.`-rooted receiver chain such as `this.member.helper(...)` or
        // `this.inner().helper(...)` dispatches on the enclosing type path
        // through the same member-chain rules as bound receivers; unknown or
        // unresolvable hops fail closed instead of falling through to static
        // type calls. Plain `this.method()` calls keep the same-type contract
        // below.
        if let Some(chain) = reference_name.strip_prefix("this.") {
            if chain.is_empty() {
                return Ok(None);
            }
            if chain.contains('.') {
                let Some(scope_path) = source_symbol.scope_path.as_deref() else {
                    return Ok(None);
                };
                return resolve_java_member_chain_from_type_path(
                    scope_path,
                    chain,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    java_import_contexts_by_file,
                    call_arity,
                    deadline,
                );
            }
        }
        let method_name = if let Some(method_name) = reference_name.strip_prefix("this.") {
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
        // A dotted reference whose leading segment names a statically imported
        // field is a member chain such as `STATIC_HELPER.entry.helper(...)` on
        // the imported field's declared type; unknown, ambiguous, or
        // unresolvable fields and hops fail closed instead of falling through
        // to a same-named static type call. Bare static-imported method calls
        // keep the static-method contract below.
        if let Some((receiver_name, member_chain)) = reference_name.split_once('.')
            && !receiver_name.is_empty()
            && !member_chain.is_empty()
            && let Some(binding) = resolve_java_static_method_import_binding_for_reference(
                &source_symbol.file_path,
                receiver_name,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
            && let Some(field_type_path) = resolve_java_imported_static_field_type_path(
                &binding,
                receiver_name,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
            && let Some(symbol_id) = resolve_java_member_chain_from_type_path(
                &field_type_path,
                member_chain,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                call_arity,
                deadline,
            )?
        {
            return Ok(Some(symbol_id));
        }
        // A dotted reference whose leading segments name a type and whose next
        // hop is a static field or static factory call is a member chain such
        // as `Util.STATIC_HELPER.helper(...)`,
        // `Util.STATIC_HELPER.entry.helper(...)`, or
        // `Util.MakeHelper().helper(...)` rooted at that static member's
        // declared type. The type prefix may be a same-package, explicitly
        // imported, fully qualified, or nested type; competing prefix
        // interpretations, non-static roots, and unknown or unresolvable hops
        // fail closed instead of falling through to a same-named static type
        // call.
        if let Some(symbol_id) = resolve_java_direct_type_qualified_static_root_member_chain(
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
    let javascript_default_import_candidates = if let Some(binding) =
        javascript_import_binding.as_ref()
        && !binding.unresolved
        && binding.imported_name == "default"
    {
        javascript_default_import_candidate_indexes(
            file_overrides,
            raw_symbols,
            name_index,
            binding,
            deadline,
        )?
    } else {
        Vec::new()
    };
    let javascript_namespace_receiver =
        javascript_reference_details.and_then(|details| details.namespace_receiver.as_deref());
    let javascript_namespace_candidates = if let Some(receiver) = javascript_namespace_receiver {
        let binding = resolve_javascript_named_import_binding_for_reference(
            &source_symbol.file_path,
            receiver,
            file_overrides,
            javascript_import_contexts_by_file,
            deadline,
        )?;
        if let Some(binding) = binding
            && !binding.unresolved
            && binding.imported_name == "<namespace>"
        {
            javascript_module_member_candidate_indexes(
                raw_symbols,
                name_index,
                reference_name,
                &binding.module_paths,
                file_overrides,
                javascript_import_contexts_by_file,
                deadline,
            )?
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    // A bare call to a namespace import (`ns(...)`) resolves only when the
    // bound module exports a single CommonJS callable through
    // `module.exports = ...`; ESM namespace objects are never callable, so
    // missing or non-callable exports fail closed instead of falling back.
    let javascript_namespace_object_call_candidates = if javascript_direct_call
        && let Some(binding) = javascript_import_binding.as_ref()
        && !binding.unresolved
        && binding.imported_name == "<namespace>"
    {
        javascript_namespace_object_call_candidate_indexes(
            raw_symbols,
            name_index,
            &binding.module_paths,
            file_overrides,
            deadline,
        )?
    } else {
        Vec::new()
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
        cpp_qualified_reference_path_groups(
            lookup_name,
            source_symbol,
            raw_symbols,
            file_overrides,
            include_targets_cache,
        )
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
                include_targets_cache,
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
            include_targets_cache,
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
    } else if javascript_namespace_receiver.is_some() {
        // Namespace member calls resolve only within the bound module; unknown
        // members fail closed instead of falling back to same-named symbols.
        (javascript_namespace_candidates, false)
    } else if !javascript_namespace_object_call_candidates.is_empty() {
        (javascript_namespace_object_call_candidates, false)
    } else if !javascript_default_import_candidates.is_empty() {
        (javascript_default_import_candidates, false)
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
    // References never cross a language family without an approved bridge
    // (design doc §17.3, §18): a candidate in a different family is an
    // unsupported cross-language match and must fail closed.
    let candidate_slice = candidate_slice
        .into_iter()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            let Some(source_language) = language_id else {
                return true;
            };
            detect_language(Path::new(&candidate.file_path)).is_ok_and(|candidate_language| {
                LanguageRegistry::same_language_family(source_language, candidate_language)
            })
        })
        .collect::<Vec<_>>();
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
            include_targets_cache,
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

enum CSharpInstanceReceiverResolution {
    Resolved(String),
    NoBinding,
    Blocked,
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# instance receiver resolution inputs explicit"
)]
fn resolve_csharp_instance_receiver_call(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpInstanceReceiverResolution> {
    let Some((raw_receiver_name, member_chain)) = reference_name.split_once('.') else {
        return Ok(CSharpInstanceReceiverResolution::NoBinding);
    };
    if raw_receiver_name.is_empty() || member_chain.is_empty() {
        return Ok(CSharpInstanceReceiverResolution::NoBinding);
    }
    // A bound receiver may carry an element-access suffix such as `items[0]`
    // in `items[0].helper(...)`; the element access dispatches on the array's
    // element component type, while indexing a non-array receiver is
    // malformed and fails closed.
    let (receiver_name, array_access) = match raw_receiver_name.find('[') {
        Some(open) if raw_receiver_name.ends_with(']') => {
            let base = &raw_receiver_name[..open];
            if base.is_empty() {
                return Ok(CSharpInstanceReceiverResolution::Blocked);
            }
            (base, true)
        }
        _ => (raw_receiver_name, false),
    };
    let Some(bindings) = csharp_receiver_type_bindings_for_function(
        &source_symbol.file_path,
        source_symbol.byte_range,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(CSharpInstanceReceiverResolution::NoBinding);
    };
    if !bindings.contains(receiver_name) {
        return Ok(CSharpInstanceReceiverResolution::NoBinding);
    }
    // A bound receiver is always an instance expression: receivers without a
    // resolvable declared type (`var` locals, lambda parameters, type
    // parameters) fail closed instead of falling through to a same-named
    // static type call. A `var` local initialized from a factory call
    // (`var helper = MakeHelper()`) infers its receiver type from the
    // factory's declared return type when the factory resolves uniquely;
    // unknown, ambiguous, arity-mismatched, `void`, and primitive factories
    // fail closed too.
    let raw_binding = bindings.raw_for(receiver_name).unwrap_or_default();
    let array_component = bindings.array_component_for(receiver_name);
    let initial_binding = if array_access {
        // An element-access receiver such as `items[0].helper(...)` on a
        // single-level array-typed receiver dispatches on the array's element
        // component type, and a jagged element-access receiver such as
        // `matrix[0][0].helper(...)` on `Helper[][]` or
        // `matrix[0][0][0].helper(...)` on `Helper[][][]` strips one
        // component layer per element access and dispatches on the remaining
        // component type; indexing a non-array or primitive-array receiver,
        // or an element access deeper than the receiver's array layers, fails
        // closed. A `var` local initialized from a factory call whose
        // declared return type is a single-level array (`var items =
        // makeItems()` or `var items = Util.makeItems()`) dispatches the
        // element access through the factory's element component type too,
        // with bare and qualified callees resolving through the same factory
        // rules as other `var` initializers.
        if let Some(component_type) = array_component {
            resolve_csharp_receiver_type_binding(
                source_symbol,
                &component_type,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        } else if let Some(declared_type) = bindings.type_for(receiver_name)
            && let Some(depth) = csharp_array_access_depth(raw_receiver_name)
            && let Some(component_type) =
                csharp_array_component_spelling_at_depth(&declared_type, depth)
        {
            resolve_csharp_receiver_type_binding(
                source_symbol,
                &component_type,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        } else if let Some((factory_name, factory_arity)) = csharp_var_factory_spelling(raw_binding)
            && let Some(element_depth) = csharp_array_access_depth(raw_receiver_name)
            && let Some(binding) = csharp_factory_array_component_binding(
                source_symbol,
                &factory_name,
                factory_arity,
                element_depth,
                &bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        {
            Some(binding)
        } else if let Some((factory_name, factory_arity)) =
            csharp_foreach_factory_element_spelling(raw_binding)
            && let Some(element_depth) = csharp_array_access_depth(raw_receiver_name)
            && let Some(binding) = csharp_factory_array_component_binding(
                source_symbol,
                &factory_name,
                factory_arity,
                element_depth + 1,
                &bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        {
            Some(binding)
        } else if let Some(chain) = csharp_foreach_chain_element_spelling(raw_binding)
            && let Some(element_depth) = csharp_array_access_depth(raw_receiver_name)
            && let Some(binding) = csharp_qualified_element_access_component_type_path(
                source_symbol,
                chain,
                element_depth + 1,
                &bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        {
            Some(binding)
        } else if let Some(chain) = csharp_var_initializer_chain_spelling(raw_binding)
            && let Some(element_depth) = csharp_array_access_depth(raw_receiver_name)
        {
            // A `var` local bound from a member-chain initializer (such as
            // `var boxes = this.holder.boxes` or `var boxes = holder.boxes`)
            // resolves the chain terminal's declared array type before
            // stripping one component layer per element access; a dotted
            // chain walks the terminal array member through the qualified
            // element-access path, and a bare chain names a bound
            // field/property/local whose declared array type pins the element
            // component type directly. Unresolvable chains, marker-bound
            // chain terminals, and non-array terminals fail closed.
            if chain.contains('.') {
                csharp_qualified_element_access_component_type_path(
                    source_symbol,
                    chain,
                    element_depth,
                    &bindings,
                    raw_symbols,
                    semantic_path_index,
                    source_namespace_path,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
            } else {
                if let Some(declared_type) = bindings.raw_for(chain) {
                    if declared_type.is_empty() || declared_type.starts_with('@') {
                        return Ok(CSharpInstanceReceiverResolution::Blocked);
                    }
                    let Some(component_type) =
                        csharp_array_component_spelling_at_depth(declared_type, element_depth)
                    else {
                        return Ok(CSharpInstanceReceiverResolution::Blocked);
                    };
                    resolve_csharp_receiver_type_binding(
                        source_symbol,
                        &component_type,
                        raw_symbols,
                        semantic_path_index,
                        source_namespace_path,
                        csharp_global_import_context,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                    )?
                } else if let Some(binding) =
                    resolve_csharp_unbound_bare_member_array_component_binding(
                        source_symbol,
                        chain,
                        element_depth,
                        raw_symbols,
                        semantic_path_index,
                        source_namespace_path,
                        csharp_global_import_context,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                    )?
                {
                    Some(binding)
                } else {
                    return Ok(CSharpInstanceReceiverResolution::Blocked);
                }
            }
        } else {
            return Ok(CSharpInstanceReceiverResolution::Blocked);
        }
    } else if array_component.is_some() {
        // A direct member call on an array-typed receiver such as
        // `items.helper(...)` fails closed; only element-access receivers
        // dispatch on the array's element component type.
        return Ok(CSharpInstanceReceiverResolution::Blocked);
    } else if let Some((factory_name, factory_arity)) = csharp_var_factory_spelling(raw_binding) {
        resolve_csharp_factory_receiver_binding(
            source_symbol,
            &factory_name,
            factory_arity,
            &bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else if let Some((factory_name, factory_arity)) =
        csharp_foreach_factory_element_spelling(raw_binding)
    {
        csharp_factory_array_component_binding(
            source_symbol,
            &factory_name,
            factory_arity,
            1,
            &bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else if let Some(chain) = csharp_foreach_chain_element_spelling(raw_binding) {
        csharp_qualified_element_access_component_type_path(
            source_symbol,
            chain,
            1,
            &bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else if let Some(chain) = csharp_var_initializer_chain_spelling(raw_binding) {
        resolve_csharp_initializer_chain_binding(
            source_symbol,
            chain,
            &bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else if let Some((base_reference, base_arity, base_depth)) =
        bindings.element_access_base_for(receiver_name)
    {
        // A `var` local bound from an element access such as
        // `var first = items[0]` or `var first = matrix[0][0]` resolves to
        // the base array's element component type, stripping one component
        // layer per element access; a qualified base such as
        // `var fourth = this.fieldItems[0]` resolves the field chain's
        // terminal array field, and a factory-call base such as
        // `var first = makeItems()[0]` resolves through the same factory
        // rules as other `var` initializers. A bare base that is itself a
        // marker-bound collection local such as `var items = makeItems()`,
        // `var items = group.GetItems()`, or `var items = group?.items`
        // resolves the collection's element component type through the same
        // factory and member-chain rules. An unbound or non-array base, and
        // a depth beyond the base array's layer count, fail closed.
        if let Some(factory_call) = base_reference.strip_suffix("()") {
            csharp_factory_array_component_binding(
                source_symbol,
                factory_call,
                base_arity,
                base_depth,
                &bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        } else if base_reference.contains('.') {
            csharp_qualified_element_access_component_type_path(
                source_symbol,
                &base_reference,
                base_depth,
                &bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        } else {
            // A bare base resolves through its declared array type first
            // (parameters, typed locals, and enclosing-class fields); a base
            // that is itself a marker-bound collection local (`var items =
            // makeItems()`, `var items = group.GetItems()`, or `var items =
            // group?.items`) resolves the collection's element component type
            // through the same factory and chain rules before stripping one
            // component layer per element-access depth; a bare field chain
            // (`var items = holder`) resolves the chain terminal's declared
            // array type directly. A bare base that is not bound at all
            // (`var first = STATIC_MATRIX[0,0]` with `using static
            // Demo.Util;`) resolves as an unbound inherited or static-imported
            // member array root before stripping the element-access depth.
            // Untyped, unknown, and non-array bases fail closed.
            let marker_binding = if let Some(declared_type) = bindings.type_for(&base_reference) {
                let Some(component_type) =
                    csharp_array_component_spelling_at_depth(&declared_type, base_depth)
                else {
                    return Ok(CSharpInstanceReceiverResolution::Blocked);
                };
                resolve_csharp_receiver_type_binding(
                    source_symbol,
                    &component_type,
                    raw_symbols,
                    semantic_path_index,
                    source_namespace_path,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
            } else if bindings.element_access_base_for(&base_reference).is_some() {
                // A bare base that is itself an element-access `var` local
                // (such as `row` in `var row = Factory.MakeNestedMatrix()[0]`
                // followed by `var first = row[0]`) resolves through the same
                // component-binding recursion, stripping the additional
                // element-access depth against the terminal base.
                csharp_array_element_component_binding(
                    source_symbol,
                    &base_reference,
                    base_depth,
                    &bindings,
                    raw_symbols,
                    semantic_path_index,
                    source_namespace_path,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
            } else {
                let raw_binding = bindings.raw_for(&base_reference).unwrap_or_default();
                if let Some((factory_name, factory_arity)) =
                    csharp_var_factory_spelling(raw_binding)
                {
                    csharp_factory_array_component_binding(
                        source_symbol,
                        &factory_name,
                        factory_arity,
                        base_depth,
                        &bindings,
                        raw_symbols,
                        semantic_path_index,
                        source_namespace_path,
                        csharp_global_import_context,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                    )?
                } else if let Some((factory_name, factory_arity)) =
                    csharp_foreach_factory_element_spelling(raw_binding)
                {
                    // A bare base bound from a `foreach` over a factory-returned
                    // array (such as `row` in `foreach (var row in
                    // Factory.MakeNestedMatrix())` followed by `var first =
                    // row[0]`) resolves the factory return array's element
                    // component type one layer deeper than the recorded base
                    // depth, since the loop variable is already the element at
                    // depth one.
                    csharp_factory_array_component_binding(
                        source_symbol,
                        &factory_name,
                        factory_arity,
                        base_depth + 1,
                        &bindings,
                        raw_symbols,
                        semantic_path_index,
                        source_namespace_path,
                        csharp_global_import_context,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                    )?
                } else if let Some(chain) = csharp_var_initializer_chain_spelling(raw_binding) {
                    if chain.contains('.') {
                        csharp_qualified_element_access_component_type_path(
                            source_symbol,
                            chain,
                            base_depth,
                            &bindings,
                            raw_symbols,
                            semantic_path_index,
                            source_namespace_path,
                            csharp_global_import_context,
                            file_overrides,
                            csharp_import_contexts_by_file,
                            deadline,
                        )?
                    } else {
                        // A bare chain names a bound field, property, local,
                        // or parameter whose declared array type pins the
                        // element component type; a marker-bound or untyped
                        // chain member fails closed.
                        if let Some(declared_type) = bindings.raw_for(chain) {
                            if declared_type.is_empty() || declared_type.starts_with('@') {
                                return Ok(CSharpInstanceReceiverResolution::Blocked);
                            }
                            let Some(component_type) =
                                csharp_array_component_spelling_at_depth(declared_type, base_depth)
                            else {
                                return Ok(CSharpInstanceReceiverResolution::Blocked);
                            };
                            resolve_csharp_receiver_type_binding(
                                source_symbol,
                                &component_type,
                                raw_symbols,
                                semantic_path_index,
                                source_namespace_path,
                                csharp_global_import_context,
                                file_overrides,
                                csharp_import_contexts_by_file,
                                deadline,
                            )?
                        } else if let Some(binding) =
                            resolve_csharp_unbound_bare_member_array_component_binding(
                                source_symbol,
                                chain,
                                base_depth,
                                raw_symbols,
                                semantic_path_index,
                                source_namespace_path,
                                csharp_global_import_context,
                                file_overrides,
                                csharp_import_contexts_by_file,
                                deadline,
                            )?
                        {
                            Some(binding)
                        } else {
                            return Ok(CSharpInstanceReceiverResolution::Blocked);
                        }
                    }
                } else {
                    resolve_csharp_unbound_bare_member_array_component_binding(
                        source_symbol,
                        &base_reference,
                        base_depth,
                        raw_symbols,
                        semantic_path_index,
                        source_namespace_path,
                        csharp_global_import_context,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                    )?
                }
            };
            let Some(binding) = marker_binding else {
                return Ok(CSharpInstanceReceiverResolution::Blocked);
            };
            Some(binding)
        }
    } else if raw_binding.is_empty() {
        None
    } else {
        resolve_csharp_receiver_type_binding(
            source_symbol,
            raw_binding,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    };
    let Some(binding) = initial_binding else {
        return Ok(CSharpInstanceReceiverResolution::Blocked);
    };
    // A member chain such as `group.member.helper(...)` walks each
    // intermediate hop as a uniquely declared field, property, or event on
    // the current type or its unique class/record ancestor chain (nearest
    // declaring ancestor pins the hop); unknown, ambiguous, or unresolvable
    // hops fail closed instead of falling through to a same-named static
    // type call.
    let mut hops = member_chain.split('.').collect::<Vec<_>>();
    if hops.iter().any(|hop| hop.is_empty()) {
        return Ok(CSharpInstanceReceiverResolution::Blocked);
    }
    let Some(final_member) = hops.pop() else {
        return Ok(CSharpInstanceReceiverResolution::NoBinding);
    };
    let Some((binding, dispatch_source_symbol)) = resolve_csharp_member_chain_binding(
        source_symbol,
        binding,
        &hops,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(CSharpInstanceReceiverResolution::Blocked);
    };
    match resolve_csharp_instance_method_on_binding(
        dispatch_source_symbol,
        &binding,
        final_member,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        call_arity,
        deadline,
    )? {
        Some(symbol_id) => Ok(CSharpInstanceReceiverResolution::Resolved(symbol_id)),
        None => Ok(CSharpInstanceReceiverResolution::Blocked),
    }
}

/// Parses a factory marker binding such as `@factory:MakeHelper(0)`,
/// `@factory:this.MakeHelper(0)`, `@factory:Factories.MakeHelper(1)`, or
/// `@factory:holder.GetInner().MakeHelper(0)` into the factory call spelling
/// and its call arity. The trailing `(arity)` is the factory call's own
/// argument list; the spelling before it may contain balanced method-call
/// parens from a receiver chain. Non-marker bindings return `None`.
fn csharp_var_factory_spelling(binding: &str) -> Option<(String, usize)> {
    let call = binding.strip_prefix("@factory:")?;
    let open = call.rfind('(')?;
    let (factory_name, arguments) = call.split_at(open);
    if factory_name.is_empty() {
        return None;
    }
    let arguments = arguments.strip_prefix('(')?.strip_suffix(')')?;
    let arity = if arguments.is_empty() {
        0
    } else {
        arguments.parse::<usize>().ok()?
    };
    Some((factory_name.to_string(), arity))
}

/// Parses a `var` foreach factory-element marker binding such as
/// `@factory-element:makeItems(0)` or
/// `@factory-element:this.makeItems(0)` into the factory call spelling and
/// its call arity. The marker records that a `var` foreach variable inferred
/// its element type from a factory-returned array collection, so the resolver
/// dispatches on the factory return array's element component type, stripping
/// one additional element-component layer for an element access on the loop
/// variable. Malformed markers return `None` and fail closed.
fn csharp_foreach_factory_element_spelling(binding: &str) -> Option<(String, usize)> {
    let call = binding.strip_prefix("@factory-element:")?;
    let open = call.rfind('(')?;
    let (factory_name, arguments) = call.split_at(open);
    if factory_name.is_empty() {
        return None;
    }
    let arguments = arguments.strip_prefix('(')?.strip_suffix(')')?;
    let arity = if arguments.is_empty() {
        0
    } else {
        arguments.parse::<usize>().ok()?
    };
    Some((factory_name.to_string(), arity))
}

/// Parses a `var` foreach chain-element marker binding such as
/// `@chain-element:group.items` or `@chain-element:this.holder.items` into
/// the member-chain spelling. The marker records that a `var` foreach
/// variable inferred its element type from a member-access array collection,
/// so the resolver walks the chain to the terminal array member and dispatches
/// on its element component type, stripping one additional layer for an
/// element access on the loop variable. Malformed markers return `None` and
/// fail closed.
fn csharp_foreach_chain_element_spelling(binding: &str) -> Option<&str> {
    binding.strip_prefix("@chain-element:")
}

/// Resolves the receiver type binding for a bound receiver name used as a
/// factory receiver root: a directly-typed receiver resolves its declared
/// type; a `var` receiver bound from a factory call (`var o = MakeNested()`)
/// resolves the factory's declared return type; and a `var` receiver bound
/// from a field/property-access chain (`var o = Factory.Holder`) resolves the
/// chain terminal's declared type. Unknown, untyped, and unresolvable
/// marker-bound receivers return `None` and fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# bound factory receiver binding inputs explicit"
)]
fn resolve_csharp_bound_factory_receiver_binding(
    source_symbol: &IndexedSymbol,
    receiver_name: &str,
    bindings: &CSharpReceiverTypeBindings,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    if let Some(type_name) = bindings.type_for(receiver_name)
        && !type_name.is_empty()
    {
        return resolve_csharp_receiver_type_binding(
            source_symbol,
            &type_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    let raw_binding = bindings.raw_for(receiver_name).unwrap_or_default();
    if let Some((factory_name, factory_arity)) = csharp_var_factory_spelling(raw_binding) {
        return resolve_csharp_factory_receiver_binding(
            source_symbol,
            &factory_name,
            factory_arity,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    if let Some(chain) = csharp_var_initializer_chain_spelling(raw_binding) {
        return resolve_csharp_initializer_chain_binding(
            source_symbol,
            chain,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    // A `var` local bound from an element access such as
    // `var first = Factory.MakeNestedArray()[0]` or `var first = items[0]`
    // resolves to the base array's element component type; a factory-call
    // base (`Factory.MakeNestedArray()`) resolves through the same factory
    // rules, and a dotted member-chain base walks the terminal array
    // field, while a bare array base is handled by the declared-type and
    // marker branches above.
    if let Some((base_reference, base_arity, base_depth)) =
        bindings.element_access_base_for(receiver_name)
    {
        if let Some(factory_call) = base_reference.strip_suffix("()") {
            return csharp_factory_array_component_binding(
                source_symbol,
                factory_call,
                base_arity,
                base_depth,
                bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        if base_reference.contains('.') {
            return csharp_qualified_element_access_component_type_path(
                source_symbol,
                &base_reference,
                base_depth,
                bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        return Ok(None);
    }
    // A dotted factory root whose receiver is not a bound local may still be
    // a type-qualified static call on a declared type (including a type
    // alias or a constructed generic spelling), so `Alias.Make()` with
    // `using Alias = Demo.Derived<HelperA>;` dispatches with the receiver
    // type's concrete generic arguments for return-type substitution.
    // Unresolvable receiver names keep `None` and fail closed.
    resolve_csharp_receiver_type_binding(
        source_symbol,
        receiver_name,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )
}

/// Resolves the receiver type binding for the prefix of a factory-chain
/// marker such as `Factory.MakeNested()` in
/// `var first = Factory.MakeNested().GetOuterItem()` or `o.GetInner()` in
/// `var first = o.GetInner().MakeHelper()`. A dotted chain resolves through
/// the same initializer-chain rules as a `var` initializer (a static
/// type-qualified member root, a bound receiver, or a `this.`/`base.`/
/// constructed root), and a bare leading call such as `MakeNested()` resolves
/// the call as a factory method on the enclosing type, the unique base chain,
/// or a static-imported type and pins the receiver to its declared return
/// type. Unresolvable chains return `None` so callers keep the declared
/// return type and fail closed downstream.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# factory chain receiver resolution inputs explicit"
)]
fn resolve_csharp_factory_chain_receiver_binding(
    source_symbol: &IndexedSymbol,
    chain: &str,
    bindings: &CSharpReceiverTypeBindings,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    // A `new`-prefixed constructed-receiver root such as
    // `new Box<HelperA>()` in `var single = new Box<HelperA>().GetSingle()`
    // or `new Group().GetMaybe()` in
    // `var helper = new Group().GetMaybe()?.inner()` resolves the constructed
    // type binding (keeping its concrete type-argument spellings) and walks
    // any remaining member-chain hops through the same member-chain rules, so
    // the trailing factory's declared return type substitutes the generic
    // parameters. Malformed or unresolvable constructed roots fail closed.
    if let Some(rest) = chain.strip_prefix("new ") {
        let Some((type_name, trailing)) =
            csharp_constructed_receiver_chain_parts(rest).or_else(|| {
                // A bare constructed root such as `new Box<HelperA>()` (the
                // constructor call is the final segment with no trailing member
                // chain) keeps the whole type spelling as the receiver and an
                // empty trailing chain, so a trailing factory on the constructed
                // receiver such as `new Box<HelperA>().GetSingle()` still
                // substitutes the concrete generic arguments into the factory's
                // declared return type. Malformed spellings fail closed.
                rest.strip_suffix("()")
                    .filter(|type_name| !type_name.is_empty())
                    .map(|type_name| (type_name.to_string(), String::new()))
            })
        else {
            return Ok(None);
        };
        let Some(binding) = resolve_csharp_receiver_type_binding(
            source_symbol,
            &type_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        if trailing.is_empty() {
            return Ok(Some(binding));
        }
        let hops = trailing.split('.').collect::<Vec<_>>();
        if hops.iter().any(|hop| hop.is_empty()) {
            return Ok(None);
        }
        let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
            source_symbol,
            binding,
            &hops,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return Ok(canonicalize_csharp_type_binding(
            scope_source_symbol,
            &binding,
            raw_symbols,
        ));
    }
    // An element-access chain such as `items[0]` in
    // `var first = items[0].GetOuterItem()` or `Factory.MakeNestedArray()[0]`
    // in `var first = Factory.MakeNestedArray()[0].GetOuterItem()` dispatches
    // on the base array's element component type (a bound array local, a
    // bound `var` local initialized from a factory-returned array, or a
    // factory-call spelling), stripping one component layer per
    // element-access depth; indexing a non-array or primitive-array base, an
    // element access deeper than the base's array layers, and unresolvable
    // factory bases fail closed. Dotted member-chain bases such as
    // `group.items[0]` keep the initializer-chain walk below.
    if chain.ends_with(']')
        && let Some(open) = chain.find('[')
        && open > 0
        && let Some(depth) = csharp_array_access_depth(chain)
        && (!chain[..open].contains('.') || chain[..open].contains('('))
    {
        return csharp_array_element_component_binding(
            source_symbol,
            &chain[..open],
            depth,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    // A dotted receiver that resolves as a plain or constructed type path
    // (such as `Outer<HelperA>.Inner<HelperB>` in
    // `Outer<HelperA>.Inner<HelperB>.MakeStaticItems()`) pins the receiver
    // to the type binding, keeping its concrete generic arguments so the
    // trailing factory's declared return type substitutes them.
    if chain.contains('.')
        && let Some(binding) = resolve_csharp_receiver_type_binding(
            source_symbol,
            chain,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && csharp_dispatchable_type_path(
            source_symbol,
            raw_symbols,
            &binding,
            csharp_is_type_declaration,
        )
        .is_some()
    {
        return Ok(Some(binding));
    }
    if chain.contains('.') {
        return resolve_csharp_initializer_chain_binding(
            source_symbol,
            chain,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    // A bare leading call such as `MakeNested()` resolves as a factory and
    // pins the receiver to its declared return type.
    if let Some((leading_name, leading_arity)) = csharp_method_call_hop_spelling(chain)
        && let Some(method_type_arguments) = csharp_method_type_arguments(chain)
        && let Some(leading_method) = resolve_csharp_var_factory_method(
            source_symbol,
            &leading_name,
            leading_arity,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(leading_return) = leading_method.return_type.as_deref()
        && !leading_return.is_empty()
        && let Ok(leading_return) = substitute_csharp_method_type_parameters(
            leading_method,
            &method_type_arguments,
            leading_return,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )
    {
        return resolve_csharp_receiver_type_binding(
            leading_method,
            &leading_return,
            raw_symbols,
            semantic_path_index,
            csharp_source_namespace_path(leading_method, raw_symbols).flatten(),
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    Ok(None)
}

/// Resolves the declared-type binding of a leading static member on a
/// constructed static receiver such as `StaticNested` in
/// `Outer<HelperA>.Inner<HelperB>.StaticNested.Items[0]` or
/// `StaticNestedArray[0]` in
/// `Outer<HelperA>.Inner<HelperB>.StaticNestedArray[0].Items[0]`. The
/// receiver binding pins the declaring type, and the member must be declared
/// static with a usable declared type; the receiver's concrete generic
/// arguments substitute into the declared type (so `Inner<U>` resolves to
/// `Inner<HelperB>` and an outer-parameter member `T[] OuterItems` to
/// `HelperA[]`), one array component layer is stripped per element-access
/// depth, and the resulting component type resolves in the declaring type's
/// own file and enclosing scope. A member the resolved type does not declare
/// is looked up through the unique class/record ancestor chain, composing the
/// constructed base binding's concrete arguments from the derived receiver's
/// arguments, so `GenericDerived<HelperB>.StaticNestedArray` resolves a
/// member declared on `GenericBase<T>` when
/// `GenericDerived<T> : GenericBase<T>`. Unknown, instance, non-array, or
/// unresolvable members return `None` and fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# constructed static receiver member binding inputs explicit"
)]
fn resolve_csharp_constructed_static_receiver_member_binding(
    source_symbol: &IndexedSymbol,
    receiver_binding: &CSharpBaseTypeBinding,
    member_name: &str,
    element_depth: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    let Some(type_path) = csharp_dispatchable_type_path(
        source_symbol,
        raw_symbols,
        receiver_binding,
        csharp_is_type_declaration,
    ) else {
        return Ok(None);
    };
    let type_indexes = semantic_path_index
        .get(&type_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
        .collect::<Vec<_>>();
    if type_indexes.len() != 1 {
        return Ok(None);
    }
    // A static member may be inherited through the unique class/record
    // ancestor chain, so `GenericDerived<HelperB>.StaticNestedArray[0]`
    // resolves a static member declared on `GenericBase<T>` when
    // `GenericDerived<T> : GenericBase<T>`; each base step composes the
    // constructed base binding by substituting the current receiver's
    // concrete arguments into the base spelling's raw type-argument
    // spellings, so the member's declared type substitutes the base's type
    // parameters (and any enclosing type parameters) with the derived
    // receiver's concrete arguments. Unknown members, instance-member
    // declarations, unresolvable or cyclic base chains, and non-array or
    // primitive-array members fail closed.
    let mut current_type_symbol = &raw_symbols[type_indexes[0]];
    let mut current_receiver_binding = receiver_binding.clone();
    let mut visited_type_paths = BTreeSet::new();
    loop {
        let Some(member_bindings) = csharp_member_type_bindings_for_type(
            &current_type_symbol.file_path,
            current_type_symbol.byte_range,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        if member_bindings.contains(member_name) {
            if !member_bindings.is_static_member(member_name) {
                return Ok(None);
            }
            let Some(declared_type) = member_bindings.type_for(member_name) else {
                return Ok(None);
            };
            let mut declared_type = declared_type.to_string();
            if let Some(parameters) = csharp_type_parameter_names_for_type(
                &current_type_symbol.file_path,
                current_type_symbol.byte_range,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )? {
                declared_type = substitute_csharp_type_parameters(
                    &declared_type,
                    &parameters,
                    &current_receiver_binding.generic_arguments,
                );
            }
            declared_type = substitute_csharp_enclosing_type_parameters(
                current_type_symbol,
                &current_receiver_binding.enclosing_generic_arguments,
                &declared_type,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
            let Some(component_type) =
                csharp_array_component_spelling_at_depth(&declared_type, element_depth)
            else {
                return Ok(None);
            };
            let Some(binding) = resolve_csharp_member_hop_type_binding(
                current_type_symbol,
                &component_type,
                &current_receiver_binding,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            // The component binding resolves in the declaring type's own file
            // and enclosing scope; canonicalize it so callers in other
            // namespaces dispatch on the canonical declared type.
            return Ok(canonicalize_csharp_type_binding(
                current_type_symbol,
                &binding,
                raw_symbols,
            ));
        }
        if current_type_symbol.node_kind == "interface_declaration" {
            return Ok(None);
        }
        let Some(base_binding) = csharp_base_type_binding_for_type(
            current_type_symbol,
            raw_symbols,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let Some(base_type_path) =
            csharp_base_type_path(current_type_symbol, raw_symbols, &base_binding)
        else {
            return Ok(None);
        };
        if !visited_type_paths.insert(base_type_path.clone()) {
            return Ok(None);
        }
        let base_indexes = semantic_path_index
            .get(&base_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if base_indexes.len() != 1 {
            return Ok(None);
        }
        let parameters = csharp_type_parameter_names_for_type(
            &current_type_symbol.file_path,
            current_type_symbol.byte_range,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        .unwrap_or_default();
        let generic_arguments = base_binding
            .raw_generic_argument_spellings
            .iter()
            .map(|spelling| {
                substitute_csharp_type_parameters(
                    spelling,
                    &parameters,
                    &current_receiver_binding.generic_arguments,
                )
            })
            .collect::<Vec<_>>();
        let enclosing_generic_arguments = csharp_base_step_enclosing_arguments(
            &base_binding,
            &base_type_path,
            &current_receiver_binding.semantic_type_path,
            &current_receiver_binding.enclosing_generic_arguments,
            &parameters,
            &current_receiver_binding.generic_arguments,
            raw_symbols,
            semantic_path_index,
        );
        current_receiver_binding = CSharpBaseTypeBinding {
            semantic_type_path: base_type_path,
            is_global_qualified: true,
            alias_name: None,
            namespace_import_paths: Vec::new(),
            generic_arguments,
            raw_generic_argument_spellings: base_binding.raw_generic_argument_spellings.clone(),
            enclosing_generic_arguments,
            raw_enclosing_generic_argument_spellings: base_binding
                .raw_enclosing_generic_argument_spellings
                .clone(),
        };
        current_type_symbol = &raw_symbols[base_indexes[0]];
    }
}

/// Parses a leading static member hop spelling such as `StaticNested` (a
/// plain member) or `StaticNestedArray[0]` (an element-access member) into
/// the member name and element-access depth. Method-call spellings return
/// `None` so they fall through to the factory-call branches, and malformed
/// or non-identifier spellings fail closed.
fn csharp_static_member_element_access_spelling(hop: &str) -> Option<(String, usize)> {
    if hop.contains(['(', ')']) {
        return None;
    }
    let member = csharp_array_access_member_name(hop).unwrap_or(hop);
    if !is_safe_csharp_identifier(member) {
        return None;
    }
    let depth = csharp_array_access_depth(hop).unwrap_or(0);
    Some((member.to_string(), depth))
}

/// Resolves the receiver type binding for a `var` local initialized from a
/// factory call such as `var helper = MakeHelper()` or
/// `var helper = holder.MakeHelper()`. The factory call resolves as an
/// instance call on the enclosing type, on a bound receiver's declared type,
/// as a base-type method, a static-imported method, or a type-qualified
/// static method; the factory's declared return type then resolves to a
/// unique type binding in the factory's own scope. Unknown, ambiguous,
/// arity-mismatched, `void`, and primitive factories return `None` and fail
/// closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# factory receiver binding inputs explicit"
)]
fn resolve_csharp_factory_receiver_binding(
    source_symbol: &IndexedSymbol,
    factory_name: &str,
    factory_arity: usize,
    bindings: &CSharpReceiverTypeBindings,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    let Some(method) = resolve_csharp_var_factory_method(
        source_symbol,
        factory_name,
        factory_arity,
        bindings,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(return_type) = method.return_type.as_deref() else {
        return Ok(None);
    };
    if return_type.is_empty() {
        return Ok(None);
    }
    // A fully-parenthesized constructed-receiver factory root such as
    // `(new Box<HelperA>()).GetSingle()` unwraps to the same `new`-prefixed
    // shape so the receiver substitution below applies like its
    // unparenthesized spelling.
    let factory_spelling = csharp_parenthesized_constructed_factory_spelling(factory_name)
        .unwrap_or_else(|| factory_name.to_string());
    // A factory dispatched on a bound receiver substitutes the receiver's
    // concrete generic arguments into the method's return type, so
    // `var first = d?.GetItem()` on a `Derived<Helper> : Box<T>` receiver
    // resolves the declared `T` return to `Helper`. The receiver may be
    // directly typed, or a `var` local bound from a factory call
    // (`var o = MakeNested()`) or a field/property-access chain
    // (`var o = Factory.Holder`) that resolves to the same binding. Other
    // factory shapes have no generic receiver mapping and keep the declared
    // return type, failing closed downstream when it names a type parameter.
    let mut receiver_binding_for_return_type = None;
    let substituted_return_type = if let Some((receiver_name, method_name)) =
        factory_spelling.split_once('.')
        && !receiver_name.is_empty()
        && !method_name.is_empty()
        && !method_name.contains(['(', ')', '.'])
        && let Some(receiver_binding) = resolve_csharp_bound_factory_receiver_binding(
            source_symbol,
            receiver_name,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(receiver_type_path) = csharp_dispatchable_type_path(
            source_symbol,
            raw_symbols,
            &receiver_binding,
            csharp_is_type_declaration,
        ) {
        receiver_binding_for_return_type = Some(receiver_binding.clone());
        substitute_csharp_method_return_type(
            method,
            &receiver_binding,
            &receiver_type_path,
            return_type,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else if let Some((chain, trailing_method)) = factory_spelling.rsplit_once('.')
        && !chain.is_empty()
        && !trailing_method.is_empty()
        && !trailing_method.contains(['(', ')', '.'])
        && (chain.contains('.')
            || chain.ends_with(')')
            || chain.ends_with(']')
            || chain.ends_with('}'))
        && let Some(receiver_binding) = resolve_csharp_factory_chain_receiver_binding(
            source_symbol,
            chain,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(receiver_type_path) = csharp_dispatchable_type_path(
            source_symbol,
            raw_symbols,
            &receiver_binding,
            csharp_is_type_declaration,
        )
    {
        receiver_binding_for_return_type = Some(receiver_binding.clone());
        substitute_csharp_method_return_type(
            method,
            &receiver_binding,
            &receiver_type_path,
            return_type,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else {
        csharp_substitute_bare_inherited_factory_return_type(
            source_symbol,
            method,
            return_type,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    };
    // An explicit generic method type-argument list at the call site such as
    // `Make<HelperA>()` or `new Maker().Make<HelperA>()` substitutes the
    // method's own type parameters (`T` in `T Make<T>()`) into the declared
    // return type, so `var x = new Maker().Make<HelperA>()` binds `x` to
    // `HelperA`. Non-generic calls, arity mismatches, and methods without
    // their own type parameters keep the substituted return type and fail
    // closed downstream when it names a type parameter.
    let Some(method_type_arguments) = csharp_factory_method_type_arguments(&factory_spelling)
    else {
        return Ok(None);
    };
    let substituted_return_type = substitute_csharp_method_type_parameters(
        method,
        &method_type_arguments,
        &substituted_return_type,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    // Resolve the factory's declared return type in the factory's own scope
    // and canonicalize it to the global-qualified semantic path, so a caller
    // in another namespace dispatches the final member on the canonical
    // declared type independently of its own imports. When the factory was
    // dispatched on a receiver binding (a constructed root or a bound
    // receiver), a simple return type that names a nested type such as
    // `Inner<HelperB>` on `Outer<HelperA>.Inner<HelperB>` resolves relative
    // to the factory's own scope chain and keeps the receiver's concrete
    // enclosing generic arguments; the existing rules remain the fallback
    // for dotted, imported, and namespace-qualified spellings.
    let Some(binding) = (if let Some(receiver_binding) = receiver_binding_for_return_type.as_ref() {
        resolve_csharp_member_hop_type_binding(
            method,
            &substituted_return_type,
            receiver_binding,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else {
        resolve_csharp_receiver_type_binding(
            method,
            &substituted_return_type,
            raw_symbols,
            semantic_path_index,
            csharp_source_namespace_path(method, raw_symbols).flatten(),
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    }) else {
        return Ok(None);
    };
    Ok(canonicalize_csharp_type_binding(
        method,
        &binding,
        raw_symbols,
    ))
}

/// Unwraps a leading parenthesized constructed-receiver factory root such as
/// `(new Group()).GetItems` to the unparenthesized `new Group().GetItems`
/// spelling. Other leading-parenthesis spellings, unbalanced parentheses,
/// and parentheses without a trailing member chain return `None` and fail
/// closed.
fn csharp_parenthesized_constructed_factory_spelling(factory_name: &str) -> Option<String> {
    if !factory_name.starts_with('(') {
        return None;
    }
    let mut depth = 0usize;
    for (index, byte) in factory_name.bytes().enumerate() {
        match byte as char {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    let inner = &factory_name[1..index];
                    let remainder = factory_name[index + 1..].strip_prefix('.')?;
                    if !inner.starts_with("new ") || remainder.is_empty() {
                        return None;
                    }
                    return Some(format!("{inner}.{remainder}"));
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits a `new`-rooted constructed receiver chain such as
/// `Box<HelperA>()`, `Group(1).holder`, or `Box<HelperA> { Capacity = 2 }.items`
/// (the text after a `new ` prefix) into the normalized constructed type name
/// (keeping concrete generic type-argument spellings) and the trailing
/// member-chain hops, stripping the constructor argument list or
/// object-initializer body. Malformed or unbalanced spellings return `None`
/// and fail closed.
fn csharp_constructed_receiver_chain_parts(constructed_spelling: &str) -> Option<(String, String)> {
    let open_index = constructed_spelling.find(['(', '{'])?;
    let type_name = constructed_spelling[..open_index].trim();
    if type_name.is_empty() || type_name.contains(['[', ']', '?']) {
        return None;
    }
    // Whitespace is valid inside balanced generic argument lists (for example
    // `Pair<HelperA, HelperB>`), so only reject spaces outside angle brackets;
    // malformed spellings with stray spaces or unbalanced argument lists fail
    // closed here and downstream.
    let mut generic_depth = 0usize;
    for character in type_name.chars() {
        match character {
            '<' => generic_depth += 1,
            '>' => {
                generic_depth = generic_depth.checked_sub(1)?;
            }
            ' ' if generic_depth == 0 => return None,
            _ => {}
        }
    }
    if generic_depth != 0 {
        return None;
    }
    let open_character = constructed_spelling.as_bytes()[open_index] as char;
    let close_character = if open_character == '(' { ')' } else { '}' };
    let mut depth = 0usize;
    let mut rest = None;
    for (index, byte) in constructed_spelling.bytes().enumerate().skip(open_index) {
        let character = byte as char;
        if character == open_character {
            depth += 1;
        } else if character == close_character {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                rest = Some(&constructed_spelling[index + 1..]);
                break;
            }
        }
    }
    let rest = rest?;
    let trailing = rest.strip_prefix('.').unwrap_or_default();
    let normalized = csharp_constructed_type_spelling_with_generics(type_name)?;
    Some((normalized, trailing.to_string()))
}

/// Normalizes a constructed type spelling such as `Box<HelperA>`,
/// `Outer<HelperA>.Inner<HelperA>`, or `Demo.Box<HelperA>` to the dotted
/// semantic path with the trimmed concrete type-argument spellings
/// re-attached to their segments, so generic parameters substitute during
/// member-chain resolution. Malformed spellings and segment count mismatches
/// return `None` and fail closed.
fn csharp_constructed_type_spelling_with_generics(type_name: &str) -> Option<String> {
    let semantic_type_path = crate::language::csharp_generic_type_semantic_path(type_name)?;
    let arguments_per_segment =
        crate::language::csharp_generic_type_arguments_per_segment(type_name)?;
    let semantic_segments = semantic_type_path.split("::").collect::<Vec<_>>();
    if semantic_segments.len() != arguments_per_segment.len() {
        return None;
    }
    let mut normalized_segments = Vec::with_capacity(semantic_segments.len());
    for (segment, arguments) in semantic_segments.iter().zip(arguments_per_segment.iter()) {
        if arguments.is_empty() {
            normalized_segments.push((*segment).to_string());
        } else {
            normalized_segments.push(format!("{}<{}>", segment, arguments.join(", ")));
        }
    }
    Some(normalized_segments.join("."))
}

/// Normalizes a constructed-receiver factory spelling such as
/// `Group().GetItems`, `Group(1).GetItems`, `Outer.Inner(1).holder.GetItems`,
/// or `Group { Capacity = 2 }.GetItems` (the text after a `new ` prefix) into
/// the dotted `Type().<chain>` shape the constructed-receiver resolver
/// expects, stripping the constructor argument list or object-initializer
/// body and keeping the concrete generic type-argument spellings attached to
/// their segments so generic parameters substitute during member-chain
/// resolution. A spelling without a trailing member chain or with a malformed
/// type or unbalanced argument list returns `None` and fails closed.
fn csharp_constructed_factory_call_spelling(factory_spelling: &str) -> Option<String> {
    let open_index = factory_spelling.find(['(', '{'])?;
    let type_name = factory_spelling[..open_index].trim();
    if type_name.is_empty() || type_name.contains(['(', ')', '[', ']', '?']) {
        return None;
    }
    let open_character = factory_spelling.as_bytes()[open_index] as char;
    let close_character = if open_character == '(' { ')' } else { '}' };
    let mut depth = 0usize;
    let mut rest = None;
    for (index, byte) in factory_spelling.bytes().enumerate().skip(open_index) {
        let character = byte as char;
        if character == open_character {
            depth += 1;
        } else if character == close_character {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                rest = Some(&factory_spelling[index + 1..]);
                break;
            }
        }
    }
    let member_chain = rest?.trim_start().strip_prefix('.')?;
    if member_chain.is_empty() {
        return None;
    }
    let normalized_type = csharp_constructed_type_spelling_with_generics(type_name)?;
    Some(format!("{}().{}", normalized_type, member_chain))
}

/// Resolves the factory call of a `var` initializer to a unique method
/// symbol. Bare names resolve as enclosing-type instance calls first, then
/// base-type and static-imported methods; `this.`-rooted names never fall
/// through to static imports; `base.`-rooted names resolve as instance calls
/// on the unique base chain; a dotted name whose leading segment is a bound
/// receiver resolves as an instance method call on the receiver's declared
/// type after walking any field/property and method-call hops on the
/// receiver; remaining dotted names resolve as type-qualified static calls.
/// Unresolved or ambiguous factories return `None`.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# factory method resolution inputs explicit"
)]
fn resolve_csharp_var_factory_method<'a>(
    source_symbol: &'a IndexedSymbol,
    factory_name: &str,
    factory_arity: usize,
    bindings: &CSharpReceiverTypeBindings,
    raw_symbols: &'a [IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<&'a IndexedSymbol>> {
    // A var-marker factory chain keeps the call-site generic type-argument
    // list on the trailing method call (such as `m.Make<HelperA>` or
    // `Maker.MakeStatic<HelperA>`), so dispatch normalizes the spelling to
    // the bare trailing method name (`m.Make` or `Maker.MakeStatic`) while
    // the caller still reads the type-argument spellings off the original
    // factory spelling for return-type substitution. Non-generic spellings
    // keep the spelling unchanged; malformed or unbalanced angle lists fail
    // closed.
    let Some(factory_name) = csharp_factory_method_dispatch_name(factory_name) else {
        return Ok(None);
    };
    // A parenthesized constructed-receiver factory root such as
    // `(new Group()).GetItems` unwraps to the same shape as the
    // unparenthesized form before dispatch.
    if let Some(unwrapped) = csharp_parenthesized_constructed_factory_spelling(&factory_name) {
        return resolve_csharp_var_factory_method(
            source_symbol,
            &unwrapped,
            factory_arity,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    if let Some(method_name) = factory_name.strip_prefix("this.") {
        if method_name.is_empty() {
            return Ok(None);
        }
        if !method_name.contains('.') {
            return resolve_csharp_factory_instance_method(
                source_symbol,
                method_name,
                factory_arity,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        // A `this.`-rooted factory chain such as
        // `this.thisGroup().GetItems()` or `this.holder.GetInner().MakeHelper()`
        // resolves the leading member on the unique enclosing type, walks any
        // intermediate field, property, event, or arity-matched method-call
        // hops through the same member-chain rules (nearest declaring ancestor
        // pins each hop), and dispatches the trailing factory method on the
        // resulting type; static callers, unknown or primitive hops, and
        // missing, static, or arity-mismatched factories fail closed.
        if csharp_method_is_static(source_symbol) {
            return Ok(None);
        }
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        let type_candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.file_path == source_symbol.file_path
                    && candidate.semantic_path == scope_path
                    && csharp_is_type_declaration(candidate)
            })
            .collect::<Vec<_>>();
        if type_candidates.len() != 1 {
            return Ok(None);
        }
        let type_symbol = type_candidates[0];
        let (hops, method_name) = match method_name.rsplit_once('.') {
            Some((hops, method_name)) => {
                let hops = if hops.is_empty() {
                    Vec::new()
                } else {
                    hops.split('.').collect::<Vec<_>>()
                };
                (hops, method_name)
            }
            None => (Vec::new(), method_name),
        };
        if method_name.is_empty() || method_name.contains(['(', ')', '.']) {
            return Ok(None);
        }
        if hops.iter().any(|hop| hop.is_empty()) {
            return Ok(None);
        }
        let binding = CSharpBaseTypeBinding {
            semantic_type_path: scope_path.to_string(),
            is_global_qualified: true,
            alias_name: None,
            namespace_import_paths: Vec::new(),
            generic_arguments: Vec::new(),
            raw_generic_argument_spellings: Vec::new(),
            enclosing_generic_arguments: Vec::new(),
            raw_enclosing_generic_argument_spellings: Vec::new(),
        };
        let (binding, dispatch_source_symbol) = if hops.is_empty() {
            (binding, type_symbol)
        } else {
            let Some((binding, dispatch)) = resolve_csharp_member_chain_binding(
                type_symbol,
                binding,
                &hops,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            (binding, dispatch)
        };
        let symbol_id = resolve_csharp_instance_method_on_binding(
            dispatch_source_symbol,
            &binding,
            method_name,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            factory_arity,
            deadline,
        )?;
        return Ok(symbol_id.and_then(|symbol_id| {
            raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == symbol_id)
        }));
    }
    // An explicit `base.`-rooted factory resolves as an instance method call
    // on the unique class/record base chain, such as
    // `var helper = base.MakeHelper()` or `var helper = base.inner.MakeHelper()`;
    // the first form dispatches directly on the base chain while a form with
    // intermediate hops walks the same field/property and arity-matched
    // method-call member-chain rules on the unique base type before
    // dispatching the factory on the resulting type. Missing bases, missing
    // or primitive hops, and static, arity-mismatched, or missing base
    // factories fail closed.
    if let Some(rest) = factory_name.strip_prefix("base.") {
        if rest.is_empty() || csharp_method_is_static(source_symbol) {
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
        let (hops, method_name) = match rest.rsplit_once('.') {
            Some((hops, method_name)) => (hops, method_name),
            None => ("", rest),
        };
        if method_name.is_empty() || method_name.contains(['(', ')', '.']) {
            return Ok(None);
        }
        let hops = if hops.is_empty() {
            Vec::new()
        } else {
            hops.split('.').collect::<Vec<_>>()
        };
        if hops.iter().any(|hop| hop.is_empty()) {
            return Ok(None);
        }
        if hops.is_empty() {
            let Some(target_path) = csharp_base_method_target_path(
                source_symbol,
                raw_symbols,
                semantic_path_index,
                &base_type_binding,
                method_name,
                factory_arity,
                false,
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
                factory_arity,
                CSharpCandidateRequirements {
                    node_kind: "method_declaration",
                    require_static: false,
                    require_instance: true,
                    require_same_file: false,
                },
            )
            .and_then(|symbol_id| {
                raw_symbols
                    .iter()
                    .find(|candidate| candidate.symbol_id == symbol_id)
            }));
        }
        let Some(base_type_path) =
            csharp_base_type_path(source_symbol, raw_symbols, &base_type_binding)
        else {
            return Ok(None);
        };
        let base_indexes = semantic_path_index
            .get(&base_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_base_constructible_type(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if base_indexes.len() != 1 {
            return Ok(None);
        }
        let base_symbol = &raw_symbols[base_indexes[0]];
        let Some((binding, dispatch_source_symbol)) = resolve_csharp_member_chain_binding(
            base_symbol,
            CSharpBaseTypeBinding {
                semantic_type_path: base_type_path,
                is_global_qualified: true,
                alias_name: None,
                namespace_import_paths: Vec::new(),
                generic_arguments: Vec::new(),
                raw_generic_argument_spellings: Vec::new(),
                enclosing_generic_arguments: Vec::new(),
                raw_enclosing_generic_argument_spellings: Vec::new(),
            },
            &hops,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let symbol_id = resolve_csharp_instance_method_on_binding(
            dispatch_source_symbol,
            &binding,
            method_name,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            factory_arity,
            deadline,
        )?;
        return Ok(symbol_id.and_then(|symbol_id| {
            raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == symbol_id)
        }));
    }
    // A constructed-receiver factory such as `new Group().MakeHelper()` or
    // `var helper = new Group().holder.MakeHelper()` resolves the constructed
    // type in the caller's namespace/import scope and dispatches the factory
    // as an instance method on that type, walking any intermediate field,
    // property, event, or arity-matched method-call hops through the same
    // member-chain rules as a constructed-receiver member chain; unknown or
    // primitive constructed types, unknown hops, and missing, static, or
    // arity-mismatched factories fail closed.
    if let Some(rest) = factory_name.strip_prefix("new ") {
        if rest.is_empty() {
            return Ok(None);
        }
        // The element-access initializer base keeps the raw constructed
        // spelling such as `new Group(1).GetItems`, so normalize the
        // constructor argument list or object-initializer body away before
        // the constructed-receiver dispatch expects a `Type().member` shape.
        let Some(constructed_spelling) = csharp_constructed_factory_call_spelling(rest) else {
            return Ok(None);
        };
        return match resolve_csharp_constructor_receiver_call(
            source_symbol,
            &constructed_spelling,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            factory_arity,
            deadline,
        )? {
            CSharpConstructorReceiverResolution::Resolved(symbol_id) => Ok(raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == symbol_id)),
            CSharpConstructorReceiverResolution::NotConstructorReceiver
            | CSharpConstructorReceiverResolution::Blocked => Ok(None),
        };
    }
    // A factory chain whose leading element-access receiver is a
    // type-qualified static factory call, such as
    // `Factory.MakeNestedArray()[0].GetOuterItem` in
    // `var first = Factory.MakeNestedArray()[0].GetOuterItem()` or
    // `Factory.MakeNestedMatrix()[0][0].GetInnerItem()`: the leading call
    // resolves through the same factory rules as a `var` initializer, the
    // element-access suffix (one or more bracket groups) dispatches on the
    // call's declared return array's element component type, and the
    // trailing factory resolves as an instance method on that component
    // binding, walking any intermediate field, property, element, or
    // arity-matched method-call hops through the same member-chain rules.
    // Unresolvable or arity-mismatched leading factories, primitive or
    // multi-dimensional return arrays, and missing, static, or
    // arity-mismatched trailing factories fail closed.
    if let Some((leading_call, remainder)) = csharp_factory_chain_leading_call(&factory_name)
        && remainder.starts_with('[')
        && let Some(element_suffix_len) = csharp_element_access_suffix_len(remainder)
        && let Some(depth) = csharp_element_access_suffix_depth(&remainder[..element_suffix_len])
        && let Some((_, leading_arity)) = csharp_method_call_hop_spelling(leading_call)
        && let Some(leading_reference) = leading_call
            .find('(')
            .map(|open| &leading_call[..open])
            .filter(|name| !name.is_empty())
            .or(Some(leading_call))
        && let Some(element_binding) = csharp_factory_array_component_binding(
            source_symbol,
            leading_reference,
            leading_arity,
            depth,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        let (hops, method_name) = match remainder[element_suffix_len..].strip_prefix('.') {
            Some(trailing) => match trailing.rsplit_once('.') {
                Some((hops, method_name)) => {
                    let hops = if hops.is_empty() {
                        Vec::new()
                    } else {
                        hops.split('.').collect::<Vec<_>>()
                    };
                    (hops, method_name)
                }
                None => (Vec::new(), trailing),
            },
            None => (Vec::new(), ""),
        };
        if method_name.is_empty() || method_name.contains(['(', ')', '.']) {
            return Ok(None);
        }
        if hops.iter().any(|hop| hop.is_empty()) {
            return Ok(None);
        }
        let (binding, dispatch_source_symbol) = if hops.is_empty() {
            let Some(type_path) = csharp_dispatchable_type_path(
                source_symbol,
                raw_symbols,
                &element_binding,
                csharp_is_type_declaration,
            ) else {
                return Ok(None);
            };
            let type_indexes = semantic_path_index
                .get(&type_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                .collect::<Vec<_>>();
            if type_indexes.len() != 1 {
                return Ok(None);
            }
            (element_binding, &raw_symbols[type_indexes[0]])
        } else {
            let Some((binding, dispatch)) = resolve_csharp_member_chain_binding(
                source_symbol,
                element_binding,
                &hops,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            (binding, dispatch)
        };
        let symbol_id = resolve_csharp_instance_method_on_binding(
            dispatch_source_symbol,
            &binding,
            method_name,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            factory_arity,
            deadline,
        )?;
        return Ok(symbol_id.and_then(|symbol_id| {
            raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == symbol_id)
        }));
    }

    // A factory chain whose leading segment is an element access on a bound
    // array-typed receiver, such as `items[0].GetOuterItem` in
    // `var first = items[0].GetOuterItem()` or
    // `var first = items[0].holder.MakeHelper()`: the element access
    // dispatches on the array's element component type (stripping one
    // component layer per element-access depth), including a bound `var`
    // local initialized from a factory-returned array (`var items =
    // Factory.MakeNestedArray()`), and the trailing factory resolves as an
    // instance method call on that component binding, walking any
    // intermediate field, property, element, or arity-matched method-call
    // hops through the same member-chain rules. Indexing a non-array or
    // primitive-array receiver, an element access deeper than the
    // receiver's array layers, and unknown hops or missing, static, or
    // arity-mismatched factories fail closed.
    if let Some((receiver_name, remainder)) = factory_name.split_once('.')
        && !receiver_name.is_empty()
        && !remainder.is_empty()
        && receiver_name.ends_with(']')
        && let Some(open) = receiver_name.find('[')
        && open > 0
    {
        let base = &receiver_name[..open];
        let Some(depth) = csharp_array_access_depth(receiver_name) else {
            return Ok(None);
        };
        if base.is_empty() {
            return Ok(None);
        }
        let (hops, method_name) = match remainder.rsplit_once('.') {
            Some((hops, method_name)) => {
                let hops = if hops.is_empty() {
                    Vec::new()
                } else {
                    hops.split('.').collect::<Vec<_>>()
                };
                (hops, method_name)
            }
            None => (Vec::new(), remainder),
        };
        if method_name.is_empty() || method_name.contains(['(', ')', '.']) {
            return Ok(None);
        }
        if hops.iter().any(|hop| hop.is_empty()) {
            return Ok(None);
        }
        let element_binding = csharp_array_element_component_binding(
            source_symbol,
            base,
            depth,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?;
        let Some(binding) = element_binding else {
            return Ok(None);
        };
        let (binding, dispatch_source_symbol) = if hops.is_empty() {
            let Some(type_path) = csharp_dispatchable_type_path(
                source_symbol,
                raw_symbols,
                &binding,
                csharp_is_type_declaration,
            ) else {
                return Ok(None);
            };
            let type_indexes = semantic_path_index
                .get(&type_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                .collect::<Vec<_>>();
            if type_indexes.len() != 1 {
                return Ok(None);
            }
            (binding, &raw_symbols[type_indexes[0]])
        } else {
            let Some((binding, dispatch)) = resolve_csharp_member_chain_binding(
                source_symbol,
                binding,
                &hops,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            (binding, dispatch)
        };
        let symbol_id = resolve_csharp_instance_method_on_binding(
            dispatch_source_symbol,
            &binding,
            method_name,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            factory_arity,
            deadline,
        )?;
        return Ok(symbol_id.and_then(|symbol_id| {
            raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == symbol_id)
        }));
    }
    // A dotted factory whose leading segment is a bound receiver resolves as
    // an instance method call on the receiver's declared type, such as
    // `var helper = holder.MakeHelper()`, `var helper = holder.GetInner().MakeHelper()`,
    // or `var helper = holder.helper.MakeHelper()`; intermediate field,
    // property, and event hops walk the declared type or its unique
    // class/record ancestor chain (nearest declaring ancestor pins the hop)
    // through the same member-chain rules. Unknown, untyped, `void`, or
    // primitive receivers, unknown or primitive hops, and missing, static, or
    // arity-mismatched factory methods fail closed.
    if let Some((receiver_name, remainder)) = factory_name.split_once('.')
        && !receiver_name.is_empty()
        && !remainder.is_empty()
        && bindings.contains(receiver_name)
    {
        let (hops, method_name) = match remainder.rsplit_once('.') {
            Some((hops, method_name)) => {
                let hops = if hops.is_empty() {
                    Vec::new()
                } else {
                    hops.split('.').collect::<Vec<_>>()
                };
                (hops, method_name)
            }
            None => (Vec::new(), remainder),
        };
        if method_name.is_empty() || method_name.contains(['(', ')', '.']) {
            return Ok(None);
        }
        if hops.iter().any(|hop| hop.is_empty()) {
            return Ok(None);
        }
        // A receiver bound to a concrete declared type dispatches directly;
        // a receiver bound to a factory or member-chain marker
        // (`var group = makeGroup()` or `var group = holder`), or to an
        // element-access var (`var first = items[0]` or
        // `var first = Factory.MakeNestedArray()[0]`), resolves its receiver
        // type through the same factory, chain, and array rules before the
        // hops walk; untyped receivers, unknown markers, and unresolvable
        // marker bindings fail closed.
        let Some(binding) = resolve_csharp_bound_factory_receiver_binding(
            source_symbol,
            receiver_name,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let (binding, dispatch_source_symbol) = if hops.is_empty() {
            let Some(type_path) = csharp_dispatchable_type_path(
                source_symbol,
                raw_symbols,
                &binding,
                csharp_is_type_declaration,
            ) else {
                return Ok(None);
            };
            let type_indexes = semantic_path_index
                .get(&type_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                .collect::<Vec<_>>();
            if type_indexes.len() != 1 {
                return Ok(None);
            }
            (binding, &raw_symbols[type_indexes[0]])
        } else {
            let Some((binding, dispatch)) = resolve_csharp_member_chain_binding(
                source_symbol,
                binding,
                &hops,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            (binding, dispatch)
        };
        let symbol_id = resolve_csharp_instance_method_on_binding(
            dispatch_source_symbol,
            &binding,
            method_name,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            factory_arity,
            deadline,
        )?;
        return Ok(symbol_id.and_then(|symbol_id| {
            raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == symbol_id)
        }));
    }
    // A factory chain whose leading segment is a method call, such as
    // `makeGroup().GetItems` or `Util.MakeGroup().GetItems`, resolves the
    // leading call (scanned to its balanced argument list) as a factory on
    // the enclosing type, the unique base chain, or a static-imported type,
    // then walks any intermediate field/property/element/method-call hops
    // through the same member-chain rules before dispatching the trailing
    // factory method on the resulting type. Unknown or ambiguous leading
    // factories, unknown or primitive hops, and missing, static, or
    // arity-mismatched trailing factories fail closed.
    if let Some((leading_call, remainder)) = csharp_factory_chain_leading_call(&factory_name)
        && let Some(mut leading_call) =
            csharp_outer_parenthesized_inner(leading_call).or(Some(leading_call))
        && {
            while let Some(inner) = csharp_outer_parenthesized_inner(leading_call) {
                leading_call = inner;
            }
            true
        }
        && let Some((leading_name, leading_arity)) = csharp_method_call_hop_spelling(leading_call)
        && let Some(method_type_arguments) = csharp_method_type_arguments(leading_call)
        && let Some(leading_method) = resolve_csharp_var_factory_method(
            source_symbol,
            &leading_name,
            leading_arity,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(leading_return) = leading_method.return_type.as_deref()
        && !leading_return.is_empty()
        && let Ok(leading_return) = substitute_csharp_method_type_parameters(
            leading_method,
            &method_type_arguments,
            leading_return,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )
        && let Some(leading_binding) = resolve_csharp_receiver_type_binding(
            leading_method,
            &leading_return,
            raw_symbols,
            semantic_path_index,
            csharp_source_namespace_path(leading_method, raw_symbols).flatten(),
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        let (hops, method_name) = match remainder.rsplit_once('.') {
            Some((hops, method_name)) => {
                let hops = if hops.is_empty() {
                    Vec::new()
                } else {
                    hops.split('.').collect::<Vec<_>>()
                };
                (hops, method_name)
            }
            None => (Vec::new(), remainder),
        };
        if method_name.is_empty() || method_name.contains(['(', ')', '.']) {
            return Ok(None);
        }
        if hops.iter().any(|hop| hop.is_empty()) {
            return Ok(None);
        }
        let (binding, dispatch_source_symbol) = if hops.is_empty() {
            let Some(type_path) = csharp_dispatchable_type_path(
                source_symbol,
                raw_symbols,
                &leading_binding,
                csharp_is_type_declaration,
            ) else {
                return Ok(None);
            };
            let type_indexes = semantic_path_index
                .get(&type_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                .collect::<Vec<_>>();
            if type_indexes.len() != 1 {
                return Ok(None);
            }
            (leading_binding, &raw_symbols[type_indexes[0]])
        } else {
            let Some((binding, dispatch)) = resolve_csharp_member_chain_binding(
                leading_method,
                leading_binding,
                &hops,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            (binding, dispatch)
        };
        let symbol_id = resolve_csharp_instance_method_on_binding(
            dispatch_source_symbol,
            &binding,
            method_name,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            factory_arity,
            deadline,
        )?;
        return Ok(symbol_id.and_then(|symbol_id| {
            raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == symbol_id)
        }));
    }
    // A dotted factory whose leading segment is a bare inherited or
    // static-imported field/property root resolves the root through the
    // unique base chain (an inherited member shadows a same-named static
    // import) and dispatches the trailing factory as an instance method on
    // the resulting type, such as `holder.GetItems()` or
    // `holder.holder.GetItems()` from a type that inherits `holder` from a
    // base class; unknown or primitive roots, unknown hops, and missing,
    // static, or arity-mismatched factories fail closed.
    if let Some((receiver_name, remainder)) = factory_name.split_once('.')
        && !receiver_name.is_empty()
        && !remainder.is_empty()
        && !receiver_name.contains(['(', ')', '[', ']'])
    {
        let initial_binding = if let Some(binding) =
            resolve_csharp_inherited_field_initializer_binding(
                source_symbol,
                receiver_name,
                &[],
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )? {
            Some(binding)
        } else {
            resolve_csharp_static_imported_field_initializer_binding(
                source_symbol,
                receiver_name,
                &[],
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        };
        if let Some(initial_binding) = initial_binding {
            let (hops, method_name) = match remainder.rsplit_once('.') {
                Some((hops, method_name)) => {
                    let hops = if hops.is_empty() {
                        Vec::new()
                    } else {
                        hops.split('.').collect::<Vec<_>>()
                    };
                    (hops, method_name)
                }
                None => (Vec::new(), remainder),
            };
            if method_name.is_empty() || method_name.contains(['(', ')', '.']) {
                return Ok(None);
            }
            if hops.iter().any(|hop| hop.is_empty()) {
                return Ok(None);
            }
            let Some(type_path) = csharp_dispatchable_type_path(
                source_symbol,
                raw_symbols,
                &initial_binding,
                csharp_is_type_declaration,
            ) else {
                return Ok(None);
            };
            let type_indexes = semantic_path_index
                .get(&type_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                .collect::<Vec<_>>();
            if type_indexes.len() != 1 {
                return Ok(None);
            }
            let type_symbol = &raw_symbols[type_indexes[0]];
            let (binding, dispatch_source_symbol) = if hops.is_empty() {
                (initial_binding, type_symbol)
            } else {
                let Some((binding, dispatch)) = resolve_csharp_member_chain_binding(
                    type_symbol,
                    initial_binding,
                    &hops,
                    raw_symbols,
                    semantic_path_index,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                else {
                    return Ok(None);
                };
                (binding, dispatch)
            };
            let symbol_id = resolve_csharp_instance_method_on_binding(
                dispatch_source_symbol,
                &binding,
                method_name,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                factory_arity,
                deadline,
            )?;
            return Ok(symbol_id.and_then(|symbol_id| {
                raw_symbols
                    .iter()
                    .find(|candidate| candidate.symbol_id == symbol_id)
            }));
        }
    }
    if factory_name.contains('.') {
        return resolve_csharp_factory_static_method(
            source_symbol,
            &factory_name,
            factory_arity,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    let Some(scope_path) = source_symbol.scope_path.as_deref() else {
        return Ok(None);
    };
    let target_path = format!("{scope_path}::{factory_name}");
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
            factory_arity,
            CSharpCandidateRequirements {
                node_kind: "method_declaration",
                require_static: false,
                require_instance: false,
                require_same_file: true,
            },
        )
        .and_then(|symbol_id| {
            raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == symbol_id)
        }));
    }
    if let Some(base_type_binding) = csharp_source_base_type_binding(
        source_symbol,
        raw_symbols,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? {
        // A non-static caller can dispatch an instance base factory by simple
        // name; both static and non-static callers can dispatch a static base
        // factory by simple name, so the instance attempt runs first and the
        // static attempt follows for every caller.
        if !csharp_method_is_static(source_symbol)
            && let Some(target_path) = csharp_base_method_target_path(
                source_symbol,
                raw_symbols,
                semantic_path_index,
                &base_type_binding,
                &factory_name,
                factory_arity,
                false,
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
                factory_arity,
                CSharpCandidateRequirements {
                    node_kind: "method_declaration",
                    require_static: false,
                    require_instance: true,
                    require_same_file: false,
                },
            )
            .and_then(|symbol_id| {
                raw_symbols
                    .iter()
                    .find(|candidate| candidate.symbol_id == symbol_id)
            }));
        }
        if let Some(target_path) = csharp_base_method_target_path(
            source_symbol,
            raw_symbols,
            semantic_path_index,
            &base_type_binding,
            &factory_name,
            factory_arity,
            true,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            return Ok(resolve_csharp_candidate(
                raw_symbols,
                semantic_path_index,
                &target_path,
                Some(source_symbol),
                factory_arity,
                CSharpCandidateRequirements {
                    node_kind: "method_declaration",
                    require_static: true,
                    require_instance: false,
                    require_same_file: false,
                },
            )
            .and_then(|symbol_id| {
                raw_symbols
                    .iter()
                    .find(|candidate| candidate.symbol_id == symbol_id)
            }));
        }
    }
    let mut static_type_imports = resolve_csharp_static_type_imports_for_reference(
        &source_symbol.file_path,
        &factory_name,
        source_namespace_path,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    if let Some(csharp_global_import_context) = csharp_global_import_context {
        static_type_imports.extend(resolve_csharp_global_static_type_imports_for_reference(
            &factory_name,
            csharp_global_import_context,
        ));
    }
    Ok(resolve_csharp_static_type_imported_method(
        raw_symbols,
        semantic_path_index,
        &static_type_imports,
        &factory_name,
        factory_arity,
    )
    .and_then(|symbol_id| {
        raw_symbols
            .iter()
            .find(|candidate| candidate.symbol_id == symbol_id)
    }))
}

/// Resolves a `this.`-rooted factory call such as `this.MakeHelper()` on the
/// enclosing type using the interface, struct, and class/record instance
/// dispatch rules. Unknown, ambiguous, or static factories return `None` and
/// fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# this-rooted factory resolution inputs explicit"
)]
fn resolve_csharp_factory_instance_method<'a>(
    source_symbol: &'a IndexedSymbol,
    method_name: &str,
    factory_arity: usize,
    raw_symbols: &'a [IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<&'a IndexedSymbol>> {
    let Some(scope_path) = source_symbol.scope_path.as_deref() else {
        return Ok(None);
    };
    let type_candidates = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.file_path == source_symbol.file_path
                && candidate.semantic_path == scope_path
                && csharp_is_type_declaration(candidate)
        })
        .collect::<Vec<_>>();
    if type_candidates.len() != 1 {
        return Ok(None);
    }
    let type_symbol = type_candidates[0];
    let symbol_id = resolve_csharp_instance_method_on_binding(
        type_symbol,
        &CSharpBaseTypeBinding {
            semantic_type_path: scope_path.to_string(),
            is_global_qualified: true,
            alias_name: None,
            namespace_import_paths: Vec::new(),
            generic_arguments: Vec::new(),
            raw_generic_argument_spellings: Vec::new(),
            enclosing_generic_arguments: Vec::new(),
            raw_enclosing_generic_argument_spellings: Vec::new(),
        },
        method_name,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        factory_arity,
        deadline,
    )?;
    Ok(symbol_id.and_then(|symbol_id| {
        raw_symbols
            .iter()
            .find(|candidate| candidate.symbol_id == symbol_id)
    }))
}

/// Resolves a type-qualified factory call such as `Factories.MakeHelper()` as
/// a static method on the qualified type, mirroring the reference-resolution
/// order for qualified static calls: `global::`-qualified, nested-type,
/// same-namespace, then namespace-imported type roots. Unknown, ambiguous, or
/// non-static factories return `None` and fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# type-qualified factory resolution inputs explicit"
)]
fn resolve_csharp_factory_static_method<'a>(
    source_symbol: &'a IndexedSymbol,
    factory_name: &str,
    factory_arity: usize,
    raw_symbols: &'a [IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<&'a IndexedSymbol>> {
    // A constructed type-qualified static factory such as
    // `Outer<HelperA>.Inner<HelperB>.MakeStatic` strips its generic
    // type-argument lists before semantic dispatch (type declarations are
    // indexed without type arguments), so the static method resolves on the
    // constructed nested type like its plain spelling. Malformed angle lists
    // fail closed.
    let factory_name = match csharp_strip_generic_type_argument_lists(factory_name) {
        Some(factory_name) => factory_name,
        None => return Ok(None),
    };
    let factory_name = factory_name.as_str();
    let symbol_id = if let Some(target_path) =
        csharp_global_qualified_static_target_path(factory_name)
    {
        resolve_csharp_candidate(
            raw_symbols,
            semantic_path_index,
            &target_path,
            Some(source_symbol),
            factory_arity,
            CSharpCandidateRequirements {
                node_kind: "method_declaration",
                require_static: true,
                require_instance: false,
                require_same_file: false,
            },
        )
    } else if let Some(target_path) =
        csharp_nested_type_static_target_path(factory_name, source_symbol, raw_symbols)
    {
        resolve_csharp_candidate(
            raw_symbols,
            semantic_path_index,
            &target_path,
            Some(source_symbol),
            factory_arity,
            CSharpCandidateRequirements {
                node_kind: "method_declaration",
                require_static: true,
                require_instance: false,
                require_same_file: false,
            },
        )
    } else if let Some((method_name, binding)) = resolve_csharp_type_alias_binding_for_reference(
        &source_symbol.file_path,
        factory_name,
        source_namespace_path,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? {
        let Some((alias_name, _)) = factory_name.split_once('.') else {
            return Ok(None);
        };
        if !csharp_alias_name_is_unshadowed(alias_name, source_symbol, raw_symbols) {
            return Ok(None);
        }
        resolve_csharp_imported_static_method(
            source_symbol,
            raw_symbols,
            semantic_path_index,
            &binding,
            &method_name,
            factory_arity,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else if csharp_type_alias_name_is_ambiguous_for_reference(
        &source_symbol.file_path,
        factory_name,
        source_namespace_path,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? {
        return Ok(None);
    } else if let Some(csharp_global_import_context) = csharp_global_import_context
        && csharp_global_type_alias_name_is_ambiguous(factory_name, csharp_global_import_context)
    {
        return Ok(None);
    } else if let Some(csharp_global_import_context) = csharp_global_import_context
        && let Some((method_name, binding)) = resolve_csharp_global_type_alias_binding_for_reference(
            factory_name,
            csharp_global_import_context,
        )
    {
        let Some((alias_name, _)) = factory_name.split_once('.') else {
            return Ok(None);
        };
        if !csharp_alias_name_is_unshadowed(alias_name, source_symbol, raw_symbols) {
            return Ok(None);
        }
        resolve_csharp_imported_static_method(
            source_symbol,
            raw_symbols,
            semantic_path_index,
            &binding,
            &method_name,
            factory_arity,
            Some(csharp_global_import_context),
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else if let Some(target_path) =
        csharp_simple_type_static_target_path(factory_name, source_symbol, raw_symbols)
        && let Some((type_path, method_name)) = target_path.rsplit_once("::")
    {
        resolve_csharp_type_qualified_static_method(
            source_symbol,
            type_path,
            method_name,
            factory_arity,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else if let Some(target_path) = csharp_namespace_relative_dotted_static_target_path(
        factory_name,
        source_symbol,
        raw_symbols,
    ) && let Some((type_path, method_name)) = target_path.rsplit_once("::")
    {
        // A factory on a namespace-relative dotted type such as
        // `Other.Derived.Make()` may be declared directly on the type or
        // inherited through its unique class/record ancestor chain, so the
        // nearest declaring ancestor pins the target like the simple-type
        // branch above; a constructed generic receiver spelling keeps its
        // arguments for the caller's return-type substitution.
        resolve_csharp_type_qualified_static_method(
            source_symbol,
            type_path,
            method_name,
            factory_arity,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else if let Some(target_path) = csharp_namespace_imported_dotted_static_target_path(
        source_symbol,
        factory_name,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? {
        resolve_csharp_candidate(
            raw_symbols,
            semantic_path_index,
            &target_path,
            Some(source_symbol),
            factory_arity,
            CSharpCandidateRequirements {
                node_kind: "method_declaration",
                require_static: true,
                require_instance: false,
                require_same_file: false,
            },
        )
    } else if let Some(target_path) = csharp_alias_to_dotted_static_target_path(
        source_symbol,
        factory_name,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? && let Some((type_path, method_name)) = target_path.rsplit_once("::")
    {
        // An alias-rooted dotted factory such as
        // `OuterAlias.Inner<HelperB>.Make()` names a nested type under the
        // alias target, so the receiver type resolves through the existing
        // receiver-type binding rules and the nearest declaring
        // class/record ancestor pins the static target like the
        // simple-type branch above.
        resolve_csharp_type_qualified_static_method(
            source_symbol,
            type_path,
            method_name,
            factory_arity,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else if let Some((type_name, method_name)) = factory_name.split_once('.')
        && !type_name.is_empty()
        && type_name != "this"
        && !type_name.starts_with("global::")
        && !method_name.is_empty()
        && !method_name.contains('.')
        && csharp_namespace_import_type_is_unshadowed(type_name, source_symbol, raw_symbols)
    {
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
        resolve_csharp_namespace_imported_static_method(
            raw_symbols,
            semantic_path_index,
            &namespace_imports,
            type_name,
            method_name,
            factory_arity,
        )
    } else if let Some(target_path) = csharp_namespace_absolute_dotted_static_target_path(
        source_symbol,
        factory_name,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? && let Some((type_path, method_name)) = target_path.rsplit_once("::")
    {
        // A factory on a namespace-absolute dotted type such as
        // `Other.Derived.Make()` or `Other.Derived<HelperA>.Make()` names a
        // top-level namespace root rather than a relative, imported, or
        // alias-rooted path, so the receiver type resolves through the
        // existing receiver-type binding rules and the nearest declaring
        // class/record ancestor pins the static target like the simple-type
        // branch above.
        resolve_csharp_type_qualified_static_method(
            source_symbol,
            type_path,
            method_name,
            factory_arity,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else {
        None
    };
    Ok(symbol_id.and_then(|symbol_id| {
        raw_symbols
            .iter()
            .find(|candidate| candidate.symbol_id == symbol_id)
    }))
}

/// Parses an initializer chain marker binding such as `@init:helper`,
/// `@init:this.holder.helper`, `@init:holder.helper`, or `@init:Holder().helper`
/// into the field/property-access chain spelling. Non-marker bindings return
/// `None`.
fn csharp_var_initializer_chain_spelling(binding: &str) -> Option<&str> {
    binding.strip_prefix("@init:")
}

/// Resolves the receiver type binding for a `var` local initialized from a
/// field/property-access chain such as `var helper = helper`,
/// `var helper = this.holder.helper`, `var helper = holder.helper`,
/// `var helper = base.helper`, `var helper = new Holder().helper`,
/// `var helper = Util.STATIC_HELPER`, or `var helper = MakeHelper().helper`.
/// A bare chain pins the receiver to the bound value's declared type; a
/// `this.`-rooted chain walks hops on the unique enclosing type; a
/// `base.`-rooted chain walks hops on the unique base type; a `Type()`-rooted
/// chain walks hops on the constructed type; a leading bare method call
/// (`MakeHelper()` in `MakeHelper().helper`) resolves the call as a factory
/// method on the enclosing type, the unique base chain, or a static-imported
/// type and walks hops on its declared return type; a bound receiver walks
/// hops on its declared type; and a chain whose leading segment is not bound
/// resolves a static type-qualified field or property root, with method-call
/// hops resolving through the same member-chain rules.
/// Unknown, ambiguous, untyped, `void`, primitive, instance-member, and
/// missing-member chains return `None` and fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# initializer chain binding inputs explicit"
)]
fn resolve_csharp_initializer_chain_binding(
    source_symbol: &IndexedSymbol,
    chain: &str,
    bindings: &CSharpReceiverTypeBindings,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    // A bare chain names a bound field, property, local, or parameter whose
    // declared type pins the `var` receiver. An unbound bare name may be an
    // inherited member root (which shadows a same-named static import) or a
    // static-imported member root such as `STATIC_HELPER` from
    // `using static Demo.Util;`; a bound-but-unusable name shadows any
    // inherited or static-imported member and fails closed.
    if !chain.contains('.') {
        if let Some(type_name) = bindings.type_for(chain) {
            return resolve_csharp_receiver_type_binding(
                source_symbol,
                &type_name,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        if bindings.contains(chain) {
            return Ok(None);
        }
        if let Some(binding) = resolve_csharp_inherited_field_initializer_binding(
            source_symbol,
            chain,
            &[],
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some(binding));
        }
        return resolve_csharp_static_imported_field_initializer_binding(
            source_symbol,
            chain,
            &[],
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    let (receiver_name, member_chain) = if let Some((constructed_receiver, constructed_chain)) =
        csharp_constructed_receiver_split(chain)
        && csharp_receiver_is_constructed_type(
            source_symbol,
            &constructed_receiver,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        ) {
        (constructed_receiver, constructed_chain)
    } else if let Some((receiver, member)) = chain.split_once('.') {
        // A constructed-type leading segment such as `Outer<HelperA>` in
        // `Outer<HelperA>.Inner<HelperB>.StaticNested` (or a
        // `global::`-qualified type root such as `global::Lib` in
        // `global::Lib.Outer<HelperA>.Inner<HelperB>.StaticNested`) absorbs
        // any following type segments through the same namespace-imported
        // and alias dotted type rules as qualified element-access receivers,
        // so the whole constructed receiver pins the static member; plain
        // receivers keep the first-segment split.
        let (receiver_name, absorbed) =
            if receiver.contains('<') || receiver.starts_with("global::") {
                csharp_qualified_element_access_receiver(
                    source_symbol,
                    receiver,
                    member,
                    bindings,
                    raw_symbols,
                    semantic_path_index,
                    source_namespace_path,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
            } else {
                (receiver.to_string(), 0)
            };
        let member_chain = member
            .split('.')
            .skip(absorbed)
            .collect::<Vec<_>>()
            .join(".");
        (receiver_name, member_chain)
    } else {
        return Ok(None);
    };
    let (receiver_name, member_chain) = (receiver_name.as_str(), member_chain.as_str());
    if receiver_name.is_empty() || member_chain.is_empty() {
        return Ok(None);
    }
    let hops = member_chain.split('.').collect::<Vec<_>>();
    if hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    // A `this.`-rooted chain walks field/property/event hops through the
    // unique class/record ancestor chain so an inherited hop still pins the
    // next hop or final member.
    if receiver_name == "this" {
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        let type_candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.file_path == source_symbol.file_path
                    && candidate.semantic_path == scope_path
                    && csharp_is_type_declaration(candidate)
            })
            .collect::<Vec<_>>();
        if type_candidates.len() != 1 {
            return Ok(None);
        }
        let type_symbol = type_candidates[0];
        let Some((binding, _)) = resolve_csharp_member_chain_binding(
            type_symbol,
            CSharpBaseTypeBinding {
                semantic_type_path: scope_path.to_string(),
                is_global_qualified: true,
                alias_name: None,
                namespace_import_paths: Vec::new(),
                generic_arguments: Vec::new(),
                raw_generic_argument_spellings: Vec::new(),
                enclosing_generic_arguments: Vec::new(),
                raw_enclosing_generic_argument_spellings: Vec::new(),
            },
            &hops,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return Ok(Some(binding));
    }
    // A `base.`-rooted chain walks hops on the unique base type of the
    // enclosing type, mirroring the `base.`-rooted member-chain call rules.
    if receiver_name == "base" {
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        let type_candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.file_path == source_symbol.file_path
                    && candidate.semantic_path == scope_path
                    && csharp_is_type_declaration(candidate)
            })
            .collect::<Vec<_>>();
        if type_candidates.len() != 1 {
            return Ok(None);
        }
        let type_symbol = type_candidates[0];
        let Some(base_binding) = csharp_base_type_binding_for_type(
            type_symbol,
            raw_symbols,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let Some(base_type_path) = csharp_base_type_path(type_symbol, raw_symbols, &base_binding)
        else {
            return Ok(None);
        };
        let base_indexes = semantic_path_index
            .get(&base_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_base_constructible_type(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if base_indexes.len() != 1 {
            return Ok(None);
        }
        let base_symbol = &raw_symbols[base_indexes[0]];
        let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
            base_symbol,
            CSharpBaseTypeBinding {
                semantic_type_path: base_type_path,
                is_global_qualified: true,
                alias_name: None,
                namespace_import_paths: Vec::new(),
                generic_arguments: Vec::new(),
                raw_generic_argument_spellings: Vec::new(),
                enclosing_generic_arguments: Vec::new(),
                raw_enclosing_generic_argument_spellings: Vec::new(),
            },
            &hops,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return Ok(canonicalize_csharp_type_binding(
            scope_source_symbol,
            &binding,
            raw_symbols,
        ));
    }
    // A leading call-shaped segment such as `Holder()` or `MakeHelper()` is
    // shared by two initializer shapes: a constructed-type root
    // (`new Holder().helper` spells `Holder().helper`) and a bare factory-call
    // root (`MakeHelper().helper` spells `MakeHelper().helper`). The
    // constructed interpretation resolves the type and walks hops on it; the
    // factory interpretation resolves the method on the enclosing type, the
    // unique base chain, or a static-imported type and walks field, property,
    // and event hops through the unique class/record ancestor chain of its
    // declared return type. Exactly one resolving interpretation pins the
    // receiver; both or neither fail closed.
    if receiver_name.ends_with(')') {
        let mut candidate_bindings = Vec::new();
        if let Some(type_name) = receiver_name.strip_suffix("()")
            && !type_name.is_empty()
            && !type_name
                .split('.')
                .any(|segment| segment.is_empty() || segment.contains(['[', ']', '(', ')', '?']))
            && let Some(binding) = resolve_csharp_receiver_type_binding(
                source_symbol,
                type_name,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            && let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
                source_symbol,
                binding,
                &hops,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            && let Some(candidate) =
                canonicalize_csharp_type_binding(scope_source_symbol, &binding, raw_symbols)
        {
            candidate_bindings.push(candidate);
        }
        if let Some((method_name, call_arity)) = csharp_method_call_hop_spelling(receiver_name)
            && let Some(method_type_arguments) = csharp_method_type_arguments(receiver_name)
            && let Some(method) = resolve_csharp_var_factory_method(
                source_symbol,
                &method_name,
                call_arity,
                bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            && let Some(return_type) = method.return_type.as_deref()
            && !return_type.is_empty()
            && let Ok(return_type) = substitute_csharp_method_type_parameters(
                method,
                &method_type_arguments,
                return_type,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )
            && let Some(factory_binding) = resolve_csharp_receiver_type_binding(
                method,
                &return_type,
                raw_symbols,
                semantic_path_index,
                csharp_source_namespace_path(method, raw_symbols).flatten(),
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        {
            let candidate = if hops.is_empty() {
                canonicalize_csharp_type_binding(method, &factory_binding, raw_symbols)
            } else {
                resolve_csharp_member_chain_binding(
                    method,
                    factory_binding,
                    &hops,
                    raw_symbols,
                    semantic_path_index,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                .and_then(|(binding, scope_source_symbol)| {
                    canonicalize_csharp_type_binding(scope_source_symbol, &binding, raw_symbols)
                })
            };
            if let Some(candidate) = candidate
                && !candidate_bindings.contains(&candidate)
            {
                candidate_bindings.push(candidate);
            }
        }
        return if candidate_bindings.len() == 1 {
            Ok(candidate_bindings.pop())
        } else {
            Ok(None)
        };
    }
    // A receiver that is an element access on a bound local or field, such
    // as `boxes[0][0]` in `boxes[0][0].items[0].GetOuterItem()` where
    // `boxes` holds a factory-returned jagged array, resolves the base
    // array's element component type (a bound array local, a `var` local
    // initialized from a factory-returned array, or an element-access var)
    // stripping one component layer per element-access depth, then walks the
    // remaining hops through the same member-chain rules. An unbound or
    // non-array base falls through to the static type-qualified root and
    // static-imported member paths below.
    if receiver_name.ends_with(']')
        && let Some(open) = receiver_name.find('[')
        && open > 0
        && !receiver_name[..open].is_empty()
        && bindings.contains(&receiver_name[..open])
        && let Some(depth) = csharp_array_access_depth(receiver_name)
    {
        let base = &receiver_name[..open];
        let Some(binding) = csharp_array_element_component_binding(
            source_symbol,
            base,
            depth,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
            source_symbol,
            binding,
            &hops,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return Ok(canonicalize_csharp_type_binding(
            scope_source_symbol,
            &binding,
            raw_symbols,
        ));
    }
    // A bound receiver pins its declared type before the hops walk through
    // the unique class/record ancestor chain.
    if bindings.contains(receiver_name) {
        let Some(type_name) = bindings.type_for(receiver_name) else {
            return Ok(None);
        };
        let Some(binding) = resolve_csharp_receiver_type_binding(
            source_symbol,
            &type_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
            source_symbol,
            binding,
            &hops,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return Ok(canonicalize_csharp_type_binding(
            scope_source_symbol,
            &binding,
            raw_symbols,
        ));
    }
    // A chain whose leading receiver is a constructed static type such as
    // `Outer<HelperA>.Inner<HelperB>.StaticNested` in
    // `var nested = Outer<HelperA>.Inner<HelperB>.StaticNested` resolves the
    // first member as a static field, property, or element-access member on
    // the receiver's concrete generic arguments, then walks any remaining
    // hops through the same member-chain rules; unknown, instance, non-array,
    // or unresolvable members fail closed.
    if receiver_name.contains('<')
        && let Some(receiver_binding) = resolve_csharp_receiver_type_binding(
            source_symbol,
            receiver_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(first) = hops.first()
        && let Some((leading_member, leading_element_depth)) =
            csharp_static_member_element_access_spelling(first)
        && let Some(leading_binding) = resolve_csharp_constructed_static_receiver_member_binding(
            source_symbol,
            &receiver_binding,
            &leading_member,
            leading_element_depth,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        if hops.len() == 1 {
            return Ok(Some(leading_binding));
        }
        let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
            source_symbol,
            leading_binding,
            &hops[1..],
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return Ok(canonicalize_csharp_type_binding(
            scope_source_symbol,
            &binding,
            raw_symbols,
        ));
    }
    // A chain whose leading segment is neither a local binding nor
    // `this`/`base`/constructed-type rooted may be a static type-qualified
    // member root such as `Util.STATIC_HELPER`, `Outer.Util.STATIC_HELPER`,
    // or `global::Demo.Util.STATIC_HELPER`; the first member after the
    // resolved type must be a static field or property and any remaining
    // hops walk the same member-chain rules. When the leading segment is not
    // a resolvable type path either, an unbound name may be a static-imported
    // member root such as `STATIC_HELPER.entry` from `using static Demo.Util;`.
    if let Some(binding) = resolve_csharp_static_field_initializer_binding(
        source_symbol,
        chain,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? {
        return Ok(Some(binding));
    }
    if !bindings.contains(receiver_name) {
        if let Some(binding) = resolve_csharp_static_imported_field_initializer_binding(
            source_symbol,
            receiver_name,
            &hops,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some(binding));
        }
        return resolve_csharp_inherited_field_initializer_binding(
            source_symbol,
            receiver_name,
            &hops,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    Ok(None)
}

/// Resolves the receiver type binding for a `var` local initialized from a
/// static type-qualified member root such as `var helper = Util.STATIC_HELPER`,
/// `var helper = Outer.Util.STATIC_HELPER`,
/// `var helper = global::Demo.Util.STATIC_HELPER`, or
/// `var helper = Util.MakeHelper().entry`. The first member after the
/// resolved type must be a static field or property, or a unique
/// arity-matched static factory method whose declared return type pins the
/// `var` receiver; any remaining hops walk the same member-chain rules on
/// that type. A longer type prefix is tried when the current prefix does not
/// resolve to a unique type or does not declare the member, so nested and
/// namespace-qualified type paths resolve like static method calls; a prefix
/// that does not resolve as a plain declared type may still resolve through
/// the same alias and namespace-import rules as receiver type references
/// (`using U = Demo.Util;` or `using Demo;`), dispatching to exactly one type
/// declaration. A resolved type whose named member is not static, an instance
/// or missing method, an ambiguous type, and unresolvable hops return `None`
/// and fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# static field initializer binding inputs explicit"
)]
fn resolve_csharp_static_field_initializer_binding<'a>(
    source_symbol: &'a IndexedSymbol,
    chain: &str,
    raw_symbols: &'a [IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    let segments = chain.split('.').collect::<Vec<_>>();
    if segments.len() < 2 || segments.iter().any(|segment| segment.is_empty()) {
        return Ok(None);
    }
    for split in 1..segments.len() {
        let type_name = segments[..split].join(".");
        let member = segments[split];
        let hops = &segments[split + 1..];
        let method_call_hop = csharp_method_call_hop_spelling(member);
        let method_element_hop = csharp_method_call_element_access_spelling(member);
        let member_element_hop = csharp_array_access_member_name(member).and_then(|member_name| {
            csharp_array_access_depth(member).map(|depth| (member_name.to_string(), depth))
        });
        if matches!(member, "this" | "base") {
            return Ok(None);
        }
        if !is_safe_csharp_identifier(member)
            && method_call_hop.is_none()
            && method_element_hop.is_none()
            && member_element_hop.is_none()
        {
            // A non-identifier segment that is not a method call or element
            // access may be a constructed generic type segment such as
            // `Inner<HelperB>` in
            // `Outer<HelperA>.Inner<HelperB>.StaticNestedArray[0]`, so defer
            // to a longer type prefix instead of failing closed and let the
            // constructed-generic member branch resolve the root.
            continue;
        }
        // A type prefix that resolves as a plain declared type carries no
        // receiver binding; a prefix that resolves through the receiver
        // rules (an alias or a constructed generic spelling) keeps its
        // binding so the static factory's declared return type can
        // substitute the prefix's concrete generic arguments.
        let (type_path, receiver_binding) = match resolve_csharp_static_initializer_type_path(
            source_symbol,
            &type_name,
            raw_symbols,
            semantic_path_index,
        ) {
            Some(type_path) => {
                // A constructed generic spelling such as `Derived<HelperA>`
                // or `Other.Derived<HelperA>` resolves as a plain declared
                // type but keeps its concrete type arguments so inherited
                // static factories and fields declared with the base's type
                // parameters substitute the spelling's arguments; plain
                // non-generic spellings keep no binding.
                let receiver_binding =
                    if crate::language::csharp_generic_type_arguments(&type_name).is_some() {
                        resolve_csharp_receiver_type_binding(
                            source_symbol,
                            &type_name,
                            raw_symbols,
                            semantic_path_index,
                            csharp_source_namespace_path(source_symbol, raw_symbols).flatten(),
                            csharp_global_import_context,
                            file_overrides,
                            csharp_import_contexts_by_file,
                            deadline,
                        )?
                    } else {
                        None
                    };
                (type_path, receiver_binding)
            }
            None => {
                // A type prefix that does not resolve as a plain declared type
                // may still resolve through the same alias and namespace-import
                // rules as receiver type references (`using U = Demo.Util;` or
                // `using Demo;`); a dotted prefix whose first segment comes
                // from a namespace import (`Outer.Util` with `using Demo;`
                // when the nested type is `Demo.Outer.Util`) resolves through
                // the imported namespace as a nested type. The resolved
                // binding or type path must dispatch to exactly one type
                // declaration.
                match resolve_csharp_receiver_type_binding(
                    source_symbol,
                    &type_name,
                    raw_symbols,
                    semantic_path_index,
                    csharp_source_namespace_path(source_symbol, raw_symbols).flatten(),
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )? {
                    Some(binding) => {
                        let Some(type_path) = csharp_dispatchable_type_path(
                            source_symbol,
                            raw_symbols,
                            &binding,
                            csharp_is_type_declaration,
                        ) else {
                            continue;
                        };
                        (type_path, Some(binding))
                    }
                    None => {
                        if let Some(type_path) = resolve_csharp_namespace_imported_nested_type_path(
                            source_symbol,
                            &type_name,
                            raw_symbols,
                            semantic_path_index,
                            csharp_source_namespace_path(source_symbol, raw_symbols).flatten(),
                            csharp_global_import_context,
                            file_overrides,
                            csharp_import_contexts_by_file,
                            deadline,
                        )? {
                            (type_path, None)
                        } else if let Some(type_path) =
                            resolve_csharp_namespace_imported_dotted_type_path(
                                source_symbol,
                                &type_name,
                                raw_symbols,
                                semantic_path_index,
                                csharp_source_namespace_path(source_symbol, raw_symbols).flatten(),
                                csharp_global_import_context,
                                file_overrides,
                                csharp_import_contexts_by_file,
                                deadline,
                            )?
                        {
                            (type_path, None)
                        } else if let Some(type_path) = resolve_csharp_alias_to_dotted_type_path(
                            source_symbol,
                            &type_name,
                            raw_symbols,
                            semantic_path_index,
                            csharp_source_namespace_path(source_symbol, raw_symbols).flatten(),
                            csharp_global_import_context,
                            file_overrides,
                            csharp_import_contexts_by_file,
                            deadline,
                        )? {
                            (type_path, None)
                        } else {
                            continue;
                        }
                    }
                }
            }
        };
        let type_indexes = semantic_path_index
            .get(&type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if type_indexes.len() != 1 {
            return Ok(None);
        }
        let type_symbol = &raw_symbols[type_indexes[0]];
        if let Some((method_name, hop_arity)) = method_call_hop {
            // A static factory method-call root such as
            // `Util.MakeHelper().entry` dispatches the first member as a
            // unique arity-matched static method on the resolved type; its
            // declared return type pins the receiver and remaining hops walk
            // the same member-chain rules. A type with no such method defers
            // to a longer type prefix; a type with a same-named method that
            // is not a matching static factory fails closed. An explicit
            // generic method type-argument list at the call site such as
            // `Maker.MakeStatic<HelperA>()` substitutes the method's own
            // type parameters into its declared return type.
            let Some(method_type_arguments) = csharp_method_type_arguments(member) else {
                return Ok(None);
            };
            let method_path = format!("{type_path}::{method_name}");
            let method_indexes = semantic_path_index
                .get(&method_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| raw_symbols[*index].node_kind == "method_declaration")
                .collect::<Vec<_>>();
            let method_id = if method_indexes.is_empty() {
                // A type with no direct static factory may still inherit one
                // through its unique class/record ancestor chain, so
                // `Caller.Make()` with `Caller : Derived<Helper>` and
                // `Base<U>::Make` resolves to the declaring base method; a
                // type without such a method defers to a longer type prefix.
                match resolve_csharp_type_qualified_static_method(
                    source_symbol,
                    &type_path,
                    &method_name,
                    hop_arity,
                    raw_symbols,
                    semantic_path_index,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )? {
                    Some(symbol_id) => symbol_id,
                    None => continue,
                }
            } else {
                let Some(method_id) = resolve_csharp_candidate(
                    raw_symbols,
                    semantic_path_index,
                    &method_path,
                    Some(source_symbol),
                    hop_arity,
                    CSharpCandidateRequirements {
                        node_kind: "method_declaration",
                        require_static: true,
                        require_instance: false,
                        require_same_file: false,
                    },
                ) else {
                    return Ok(None);
                };
                method_id
            };
            let Some(method_symbol) = raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == method_id)
            else {
                return Ok(None);
            };
            let Some(return_type) = method_symbol.return_type.as_deref() else {
                return Ok(None);
            };
            let return_type = substitute_csharp_method_type_parameters(
                method_symbol,
                &method_type_arguments,
                return_type,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
            let return_type = csharp_substitute_qualified_factory_return_type(
                method_symbol,
                &type_path,
                receiver_binding.as_ref(),
                &return_type,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
            let Some(factory_binding) = resolve_csharp_receiver_type_binding(
                method_symbol,
                &return_type,
                raw_symbols,
                semantic_path_index,
                csharp_source_namespace_path(method_symbol, raw_symbols).flatten(),
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            if hops.is_empty() {
                return Ok(canonicalize_csharp_type_binding(
                    method_symbol,
                    &factory_binding,
                    raw_symbols,
                ));
            }
            let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
                method_symbol,
                factory_binding,
                hops,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            return Ok(canonicalize_csharp_type_binding(
                scope_source_symbol,
                &binding,
                raw_symbols,
            ));
        }
        if let Some((method_name, hop_arity, element_depth)) = method_element_hop {
            // A static factory method-call root with an element-access suffix
            // such as `Util.MakeItems()[0].entry` or
            // `Factory.MakeNestedArray()[0].innerItems[0]` dispatches the
            // first member as a unique arity-matched static method on the
            // resolved type, strips one return-array component layer per
            // element-access depth, pins the receiver to the resulting
            // element binding, and walks remaining hops through the same
            // member-chain rules. A type with no such method defers to a
            // longer type prefix; a type with a same-named method that is
            // not a matching static factory, or a non-array or
            // primitive-array return, fails closed.
            let method_path = format!("{type_path}::{method_name}");
            let method_indexes = semantic_path_index
                .get(&method_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| raw_symbols[*index].node_kind == "method_declaration")
                .collect::<Vec<_>>();
            let method_id = if method_indexes.is_empty() {
                // A type with no direct static factory may still inherit one
                // through its unique class/record ancestor chain, so
                // `Caller.Make()` with `Caller : Derived<Helper>` and
                // `Base<U>::Make` resolves to the declaring base method; a
                // type without such a method defers to a longer type prefix.
                match resolve_csharp_type_qualified_static_method(
                    source_symbol,
                    &type_path,
                    &method_name,
                    hop_arity,
                    raw_symbols,
                    semantic_path_index,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )? {
                    Some(symbol_id) => symbol_id,
                    None => continue,
                }
            } else {
                let Some(method_id) = resolve_csharp_candidate(
                    raw_symbols,
                    semantic_path_index,
                    &method_path,
                    Some(source_symbol),
                    hop_arity,
                    CSharpCandidateRequirements {
                        node_kind: "method_declaration",
                        require_static: true,
                        require_instance: false,
                        require_same_file: false,
                    },
                ) else {
                    return Ok(None);
                };
                method_id
            };
            let Some(method_symbol) = raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == method_id)
            else {
                return Ok(None);
            };
            let Some(return_type) = method_symbol.return_type.as_deref() else {
                return Ok(None);
            };
            if return_type.is_empty() {
                return Ok(None);
            }
            let Some(method_type_arguments) = csharp_method_type_arguments(member) else {
                return Ok(None);
            };
            let return_type = substitute_csharp_method_type_parameters(
                method_symbol,
                &method_type_arguments,
                return_type,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
            let return_type = csharp_substitute_qualified_factory_return_type(
                method_symbol,
                &type_path,
                receiver_binding.as_ref(),
                &return_type,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
            let Some(component_name) =
                csharp_array_component_spelling_at_depth(&return_type, element_depth)
            else {
                return Ok(None);
            };
            let Some(factory_binding) = resolve_csharp_receiver_type_binding(
                method_symbol,
                &component_name,
                raw_symbols,
                semantic_path_index,
                csharp_source_namespace_path(method_symbol, raw_symbols).flatten(),
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            if hops.is_empty() {
                return Ok(canonicalize_csharp_type_binding(
                    method_symbol,
                    &factory_binding,
                    raw_symbols,
                ));
            }
            let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
                method_symbol,
                factory_binding,
                hops,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            return Ok(canonicalize_csharp_type_binding(
                scope_source_symbol,
                &binding,
                raw_symbols,
            ));
        }
        // A member hop with an element-access suffix such as
        // `StaticNestedArray[0]` or `Nested[0][0]` resolves the named static
        // field/property's declared type and strips one array component layer
        // per element access before continuing the chain; non-array or
        // primitive-array members and element access deeper than the declared
        // array fail closed.
        let (member_name, member_depth) = match &member_element_hop {
            Some((member_name, depth)) => (member_name.as_str(), Some(*depth)),
            None => (member, None),
        };
        // A constructed generic type prefix such as `Plain<HelperB>` in
        // `Plain<HelperB>.StaticNestedArray[0].Items[0].RunB(...)` resolves
        // the leading static member on the type's concrete generic arguments
        // so the member's declared type substitutes its type parameters
        // before any element-access layers are stripped and the remaining
        // hops walk the same member-chain rules; unknown, instance,
        // non-array, or unresolvable members fail closed.
        if type_name.contains('<')
            && let Some(receiver_binding) = resolve_csharp_receiver_type_binding(
                source_symbol,
                &type_name,
                raw_symbols,
                semantic_path_index,
                csharp_source_namespace_path(source_symbol, raw_symbols).flatten(),
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            && let Some(member_binding) = resolve_csharp_constructed_static_receiver_member_binding(
                source_symbol,
                &receiver_binding,
                member_name,
                member_depth.unwrap_or(0),
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
        {
            if hops.is_empty() {
                return Ok(Some(member_binding));
            }
            let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
                type_symbol,
                member_binding,
                hops,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            return Ok(canonicalize_csharp_type_binding(
                scope_source_symbol,
                &binding,
                raw_symbols,
            ));
        }
        // A static member is inherited through the unique class/record
        // ancestor chain, so `Plain.StaticNestedArray[0]` resolves a static
        // member declared on `PlainBase` when `Plain : PlainBase`; the
        // nearest declaration pins the declaring type and its declared type.
        // Members that neither the resolved type nor any ancestor declares
        // defer to a longer type prefix that may name a nested type.
        let member_owner = {
            let mut current_type_symbol = type_symbol;
            // A type prefix resolved through the receiver rules (a type alias
            // or a constructed generic spelling) seeds the inheritance walk
            // with its concrete generic arguments, so `Alias.StaticField`
            // with `using Alias = Demo.Derived<HelperA>;` and
            // `Base<U>::StaticField` substitutes the alias target's argument
            // into the field's declared type; prefixes resolved as plain
            // declared types keep empty arguments and fail closed for
            // parameter-dependent members.
            let mut current_generic_arguments = receiver_binding
                .as_ref()
                .map(|binding| binding.generic_arguments.clone())
                .unwrap_or_default();
            let mut current_enclosing_generic_arguments = receiver_binding
                .as_ref()
                .map(|binding| binding.enclosing_generic_arguments.clone())
                .unwrap_or_default();
            let mut visited_type_paths = BTreeSet::new();
            let mut found = None;
            loop {
                let Some(bindings) = csharp_member_type_bindings_for_type(
                    &current_type_symbol.file_path,
                    current_type_symbol.byte_range,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                else {
                    return Ok(None);
                };
                if bindings.contains(member_name) {
                    found = Some((
                        bindings,
                        current_type_symbol,
                        current_generic_arguments,
                        current_enclosing_generic_arguments,
                    ));
                    break;
                }
                if current_type_symbol.node_kind == "interface_declaration" {
                    break;
                }
                let Some(base_binding) = csharp_base_type_binding_for_type(
                    current_type_symbol,
                    raw_symbols,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                else {
                    break;
                };
                let Some(base_type_path) =
                    csharp_base_type_path(current_type_symbol, raw_symbols, &base_binding)
                else {
                    break;
                };
                if !visited_type_paths.insert(base_type_path.clone()) {
                    break;
                }
                let base_indexes = semantic_path_index
                    .get(&base_type_path)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                    .collect::<Vec<_>>();
                if base_indexes.len() != 1 {
                    break;
                }
                // Compose the constructed base binding's concrete arguments
                // by substituting the current receiver's arguments into the
                // base spelling's raw type-argument spellings, so a base
                // such as `GenericBase<HelperB>` reached through a
                // non-generic `FixedDerived : GenericBase<HelperB>` or a
                // generic `Derived<T> : GenericBase<T>` pins the same
                // concrete arguments for the member's declared type.
                let parameters = csharp_type_parameter_names_for_type(
                    &current_type_symbol.file_path,
                    current_type_symbol.byte_range,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                .unwrap_or_default();
                let generic_arguments = base_binding
                    .raw_generic_argument_spellings
                    .iter()
                    .map(|spelling| {
                        substitute_csharp_type_parameters(
                            spelling,
                            &parameters,
                            &current_generic_arguments,
                        )
                    })
                    .collect::<Vec<_>>();
                let enclosing_generic_arguments = csharp_base_step_enclosing_arguments(
                    &base_binding,
                    &base_type_path,
                    current_type_symbol.semantic_path.as_str(),
                    &current_enclosing_generic_arguments,
                    &parameters,
                    &current_generic_arguments,
                    raw_symbols,
                    semantic_path_index,
                );
                current_generic_arguments = generic_arguments;
                current_enclosing_generic_arguments = enclosing_generic_arguments;
                current_type_symbol = &raw_symbols[base_indexes[0]];
            }
            found
        };
        let Some((
            member_bindings,
            declaring_type_symbol,
            declaring_generic_arguments,
            declaring_enclosing_generic_arguments,
        )) = member_owner
        else {
            // The member is not declared on this type or any ancestor; a
            // longer type prefix may name a nested type that hosts the member.
            continue;
        };
        if !member_bindings.is_static_member(member_name) {
            // An instance member reached through a type name is invalid C#
            // and fails closed.
            return Ok(None);
        }
        let Some(member_type_name) = member_bindings.type_for(member_name) else {
            return Ok(None);
        };
        let mut member_type_name = member_type_name.to_string();
        if let Some(parameters) = csharp_type_parameter_names_for_type(
            &declaring_type_symbol.file_path,
            declaring_type_symbol.byte_range,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            member_type_name = substitute_csharp_type_parameters(
                &member_type_name,
                &parameters,
                &declaring_generic_arguments,
            );
        }
        member_type_name = substitute_csharp_enclosing_type_parameters(
            declaring_type_symbol,
            &declaring_enclosing_generic_arguments,
            &member_type_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?;
        let Some(member_type_name) = (match member_depth {
            Some(depth) => csharp_array_component_spelling_at_depth(&member_type_name, depth),
            None => Some(member_type_name),
        }) else {
            return Ok(None);
        };
        let Some(member_binding) = resolve_csharp_receiver_type_binding(
            declaring_type_symbol,
            &member_type_name,
            raw_symbols,
            semantic_path_index,
            csharp_source_namespace_path(declaring_type_symbol, raw_symbols).flatten(),
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        if hops.is_empty() {
            return Ok(canonicalize_csharp_type_binding(
                declaring_type_symbol,
                &member_binding,
                raw_symbols,
            ));
        }
        let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
            declaring_type_symbol,
            member_binding,
            hops,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return Ok(canonicalize_csharp_type_binding(
            scope_source_symbol,
            &binding,
            raw_symbols,
        ));
    }
    Ok(None)
}

/// Canonicalizes a resolved receiver binding to a global-qualified semantic
/// path so the `var` receiver dispatches independently of the caller's
/// namespace. Bindings resolved inside the declaring type's file scope are
/// namespace-relative (`Helper` for `Demo::Helper`); pinning the canonical
/// path keeps a cross-namespace caller such as
/// `namespace Other { using Demo; var v = Util.STATIC_HELPER.entry; }` from
/// re-resolving the relative name in its own namespace.
fn canonicalize_csharp_type_binding(
    scope_source_symbol: &IndexedSymbol,
    binding: &CSharpBaseTypeBinding,
    raw_symbols: &[IndexedSymbol],
) -> Option<CSharpBaseTypeBinding> {
    let type_path = csharp_dispatchable_type_path(
        scope_source_symbol,
        raw_symbols,
        binding,
        csharp_is_type_declaration,
    )?;
    // The canonical binding keeps the constructed generic arguments so a
    // later hop can still substitute the type's parameters (for example a
    // `Holder<Helper>` element component must resolve `U value` to
    // `Helper`); raw base-list spellings do not survive canonicalization.
    Some(CSharpBaseTypeBinding {
        semantic_type_path: type_path,
        is_global_qualified: true,
        alias_name: None,
        namespace_import_paths: Vec::new(),
        generic_arguments: binding.generic_arguments.clone(),
        raw_generic_argument_spellings: Vec::new(),
        enclosing_generic_arguments: binding.enclosing_generic_arguments.clone(),
        raw_enclosing_generic_argument_spellings: Vec::new(),
    })
}

/// Resolves a static type-qualified member-chain call such as
/// `Util.STATIC_HELPER.entry.Run(1)`, `global::Demo.Util.STATIC_HELPER.entry.Run(1)`,
/// or `Util.MakeHelper().entry.Run(1)`. The first member after the resolved
/// type must be a static field or property, or a unique arity-matched static
/// factory method; any remaining hops walk the same member-chain rules, and
/// the final member dispatches as an instance method on the resolved receiver
/// type. Unknown, ambiguous, or unresolvable roots and hops, and missing or
/// static final members fail closed instead of falling through to a same-named
/// static type call.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# static field member-chain call inputs explicit"
)]
fn resolve_csharp_static_field_member_chain_call(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((chain_prefix, final_member)) = reference_name.rsplit_once('.') else {
        return Ok(None);
    };
    if chain_prefix.is_empty()
        || final_member.is_empty()
        || !is_safe_csharp_identifier(final_member)
        || !chain_prefix.contains('.')
    {
        return Ok(None);
    }
    // The chain prefix names a static field/factory root followed by zero or
    // more instance hops; resolving it pins the receiver type. A prefix that
    // is not a resolvable static-member chain returns `None` so the caller
    // falls through to the static type-call paths.
    let Some(binding) = resolve_csharp_static_field_initializer_binding(
        source_symbol,
        chain_prefix,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    resolve_csharp_instance_method_on_binding(
        source_symbol,
        &binding,
        final_member,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        call_arity,
        deadline,
    )
}

/// Resolves a static-imported member-chain call such as
/// `STATIC_HELPER.entry.Run(1)` or `STATIC_HELPER.inner().entry.Run(1)` with
/// `using static Demo.Util;`. The leading member must resolve as a static
/// field or property on exactly one static-imported type; remaining hops walk
/// the same member-chain rules, and the final member dispatches as an instance
/// method on the resolved receiver type. Unknown, ambiguous, or instance
/// members, unresolvable hops, and missing or static final members fail closed
/// instead of falling through to a same-named static type call.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# static-imported member-chain call inputs explicit"
)]
fn resolve_csharp_static_imported_member_chain_call(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((chain_prefix, final_member)) = reference_name.rsplit_once('.') else {
        return Ok(None);
    };
    if chain_prefix.is_empty()
        || final_member.is_empty()
        || !is_safe_csharp_identifier(final_member)
    {
        return Ok(None);
    }
    let (root_member, hops) = match chain_prefix.split_once('.') {
        Some((root, hops)) => (root, hops.split('.').collect::<Vec<_>>()),
        None => (chain_prefix, Vec::new()),
    };
    // A root with an element-access suffix such as `StaticNestedArray[0]`
    // strips the brackets in the binding helper below; any other malformed
    // root fails closed.
    if hops.iter().any(|hop| hop.is_empty())
        || (!is_safe_csharp_identifier(root_member)
            && (csharp_array_access_member_name(root_member).is_none()
                || csharp_array_access_depth(root_member).is_none()))
    {
        return Ok(None);
    }
    // The chain prefix names a static-imported member root followed by zero
    // or more instance hops; resolving it pins the receiver type. A prefix
    // that is not a resolvable static-imported member chain returns `None` so
    // the caller falls through to the remaining resolution paths.
    let Some(binding) = resolve_csharp_static_imported_field_initializer_binding(
        source_symbol,
        root_member,
        &hops,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    resolve_csharp_instance_method_on_binding(
        source_symbol,
        &binding,
        final_member,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        call_arity,
        deadline,
    )
}

/// Resolves a bare inherited member-chain call such as
/// `MATRIX[0,0].entry.Run(1)` or `holder.entry.Run(1)` where the leading
/// member (a field or property with an optional element-access suffix) is
/// declared on a class/record ancestor of the enclosing type rather than
/// bound locally or declared on the enclosing type itself. The leading
/// member resolves through the same inherited-then-static-imported rules as
/// bare `var` initializer and `foreach` roots (the nearest declaring
/// ancestor pins the declared array type, stripping one component layer per
/// element-access group, so an inherited static or instance `Helper[,]`
/// member indexed with `[0,0]` pins the element component `Helper`), then
/// any remaining hops walk the same member-chain rules before the final
/// member dispatches as an instance call. Unknown or unresolvable roots,
/// primitive or non-array roots, unresolvable hops, and missing or static
/// final members fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# bare inherited member-chain call inputs explicit"
)]
fn resolve_csharp_bare_inherited_member_chain_call(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((root_spelling, member_chain)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    if root_spelling.is_empty() || member_chain.is_empty() {
        return Ok(None);
    }
    let (root_member, root_depth) = match csharp_array_access_member_name(root_spelling) {
        Some(member_name) => match csharp_array_access_depth(root_spelling) {
            Some(depth) => (member_name.to_string(), depth),
            None => return Ok(None),
        },
        None => (root_spelling.to_string(), 0),
    };
    if !is_safe_csharp_identifier(&root_member) {
        return Ok(None);
    }
    let mut hops = member_chain.split('.').collect::<Vec<_>>();
    if hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    let Some(final_member) = hops.pop() else {
        return Ok(None);
    };
    let Some(root_binding) = resolve_csharp_unbound_bare_member_array_component_binding(
        source_symbol,
        &root_member,
        root_depth,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    if hops.is_empty() {
        return resolve_csharp_instance_method_on_binding(
            source_symbol,
            &root_binding,
            final_member,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            call_arity,
            deadline,
        );
    }
    let Some((binding, dispatch_source_symbol)) = resolve_csharp_member_chain_binding(
        source_symbol,
        root_binding,
        &hops,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    resolve_csharp_instance_method_on_binding(
        dispatch_source_symbol,
        &binding,
        final_member,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        call_arity,
        deadline,
    )
}

/// Resolves a direct call chain rooted at a bare factory method call such as
/// `MakeHelper().entry.Run(1)` or `MakeHelper().Run(1)` where the leading call
/// is a unique arity-matched factory method on the enclosing type, the unique
/// base chain, or a static-imported type. The leading call's declared return
/// type pins the receiver; remaining field, property, and event hops walk the
/// declared return type or its unique class/record ancestor chain (nearest
/// declaring ancestor pins the hop) through the same member-chain rules, and
/// the final member dispatches as an instance method on the canonical
/// receiver type. Unknown, ambiguous, arity-mismatched, or non-factory roots,
/// unresolvable hops, and missing or static final members fail closed instead
/// of falling through to a same-named static type call.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# direct bare-factory member-chain call inputs explicit"
)]
fn resolve_csharp_direct_bare_factory_member_chain_call(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((chain_prefix, final_member)) = reference_name.rsplit_once('.') else {
        return Ok(None);
    };
    if chain_prefix.is_empty()
        || final_member.is_empty()
        || !is_safe_csharp_identifier(final_member)
    {
        return Ok(None);
    }
    let (root_member, root_arity, root_spelling, hops) = match chain_prefix.split_once('.') {
        Some((root, rest)) => {
            let Some((method_name, arity)) = csharp_method_call_hop_spelling(root) else {
                return Ok(None);
            };
            let hops = rest.split('.').collect::<Vec<_>>();
            if hops.iter().any(|hop| hop.is_empty()) {
                return Ok(None);
            }
            (method_name, arity, root, hops)
        }
        None => {
            let Some((method_name, arity)) = csharp_method_call_hop_spelling(chain_prefix) else {
                return Ok(None);
            };
            (method_name, arity, chain_prefix, Vec::new())
        }
    };
    // A bare root dispatches as a unique arity-matched factory method on the
    // enclosing type, the unique base chain, or a static-imported type; the
    // declared return type pins the receiver.
    let Some(method) = resolve_csharp_var_factory_method(
        source_symbol,
        &root_member,
        root_arity,
        &CSharpReceiverTypeBindings::default(),
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(return_type) = method.return_type.as_deref() else {
        return Ok(None);
    };
    if return_type.is_empty() {
        return Ok(None);
    }
    let Some(method_type_arguments) = csharp_method_type_arguments(root_spelling) else {
        return Ok(None);
    };
    let return_type = substitute_csharp_method_type_parameters(
        method,
        &method_type_arguments,
        return_type,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    let return_type = csharp_substitute_bare_inherited_factory_return_type(
        source_symbol,
        method,
        &return_type,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    let Some(factory_binding) = resolve_csharp_receiver_type_binding(
        method,
        &return_type,
        raw_symbols,
        semantic_path_index,
        csharp_source_namespace_path(method, raw_symbols).flatten(),
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let binding = if hops.is_empty() {
        canonicalize_csharp_type_binding(method, &factory_binding, raw_symbols)
    } else {
        resolve_csharp_member_chain_binding(
            method,
            factory_binding,
            &hops,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        .and_then(|(binding, scope_source_symbol)| {
            canonicalize_csharp_type_binding(scope_source_symbol, &binding, raw_symbols)
        })
    };
    let Some(binding) = binding else {
        return Ok(None);
    };
    resolve_csharp_instance_method_on_binding(
        source_symbol,
        &binding,
        final_member,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        call_arity,
        deadline,
    )
}

/// Resolves a bare factory-call root with an element-access suffix such as
/// `makeItems()[0]` in `makeItems()[0].helper(...)` or `makeMatrix()[0][0]`
/// in `makeMatrix()[0][0].helper(...)`: the leading call resolves through the
/// same factory rules as a `var` initializer (a unique same-type method,
/// base-type method, static-imported method, or type-qualified static method
/// with matching arity) whose declared return type is an array, and the
/// trailing member chain dispatches on the array's element component type in
/// the factory's own file and enclosing scope, stripping one component layer
/// per element-access depth. Unknown or arity-mismatched factories, primitive
/// return arrays, element access deeper than the return array's layers, and
/// unresolvable hops fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# factory array root resolution inputs explicit"
)]
fn resolve_csharp_bare_factory_array_member_chain(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((root_spelling, member_chain)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    if root_spelling.is_empty() || member_chain.is_empty() {
        return Ok(None);
    }
    let Some((function_name, function_arity, element_depth)) =
        csharp_array_factory_call_root_spelling(root_spelling)
    else {
        return Ok(None);
    };
    let Some(method) = resolve_csharp_var_factory_method(
        source_symbol,
        &function_name,
        function_arity,
        &CSharpReceiverTypeBindings::default(),
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(return_type) = method.return_type.as_deref() else {
        return Ok(None);
    };
    let Some(method_type_arguments) = csharp_method_type_arguments(root_spelling) else {
        return Ok(None);
    };
    let return_type = substitute_csharp_method_type_parameters(
        method,
        &method_type_arguments,
        return_type,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    let return_type = csharp_substitute_bare_inherited_factory_return_type(
        source_symbol,
        method,
        &return_type,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    let Some(component_name) =
        csharp_array_component_spelling_at_depth(&return_type, element_depth)
    else {
        return Ok(None);
    };
    let Some(component_binding) = resolve_csharp_receiver_type_binding(
        method,
        &component_name,
        raw_symbols,
        semantic_path_index,
        csharp_source_namespace_path(method, raw_symbols).flatten(),
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(component_binding) =
        canonicalize_csharp_type_binding(method, &component_binding, raw_symbols)
    else {
        return Ok(None);
    };
    let mut hops = member_chain.split('.').collect::<Vec<_>>();
    if hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    let Some(final_member) = hops.pop() else {
        return Ok(None);
    };
    let Some((binding, dispatch_source_symbol)) = resolve_csharp_member_chain_binding(
        source_symbol,
        component_binding,
        &hops,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    resolve_csharp_instance_method_on_binding(
        dispatch_source_symbol,
        &binding,
        final_member,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        call_arity,
        deadline,
    )
}

/// Splits a member-chain spelling at the first dot that is outside any
/// balanced parenthesis group, such as `(Util.MakeGroup()).items` into
/// `(Util.MakeGroup())` and `items`. A parenthesized receiver root containing
/// dots would otherwise split at the first inner dot; spellings without a
/// top-level dot, or with an unbalanced parenthesis group, return `None` and
/// fail closed.
fn csharp_top_level_dot_split(spelling: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    for (index, byte) in spelling.bytes().enumerate() {
        match byte as char {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
            }
            '.' if depth == 0 => return Some((&spelling[..index], &spelling[index + 1..])),
            _ => {}
        }
    }
    None
}

/// Splits a member-chain spelling at the first `()`-shaped constructed-type or
/// factory-call marker segment when a member chain follows it, such as
/// `Outer<HelperA>.Inner<HelperB>().Items` into `Outer<HelperA>.Inner<HelperB>()`
/// and `Items`, or `makeGroup().GetItems().entry` into `makeGroup()` and
/// `GetItems().entry`. Dots inside `<...>` type arguments never split a
/// segment, so dotted constructed type paths stay together in the receiver.
/// A spelling whose first call-shaped marker is the final segment (no member
/// chain after it), with no call-shaped marker, or with unbalanced angle
/// groups returns `None` and lets the caller fall back to the plain first-dot
/// split.
fn csharp_constructed_receiver_split(spelling: &str) -> Option<(String, String)> {
    let mut segments = Vec::new();
    let mut depth = 0usize;
    let mut last_start = 0usize;
    for (index, character) in spelling.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.checked_sub(1)?,
            '.' if depth == 0 => {
                if index == last_start {
                    return None;
                }
                segments.push(&spelling[last_start..index]);
                last_start = index + 1;
            }
            _ => {}
        }
    }
    if last_start == spelling.len() {
        return None;
    }
    segments.push(&spelling[last_start..]);
    let marker_index = segments
        .iter()
        .position(|segment| segment.ends_with("()"))?;
    if marker_index + 1 >= segments.len() {
        return None;
    }
    Some((
        segments[..=marker_index].join("."),
        segments[marker_index + 1..].join("."),
    ))
}

/// Returns whether a constructed-receiver spelling such as
/// `Outer<HelperA>.Inner<HelperB>()` resolves as a declared type, so the
/// constructed-receiver split is only preferred when the dotted prefix really
/// is a constructed type path rather than a bound-receiver factory call such
/// as `o.GetOuterBox()` or a bare factory call such as `makeGroup()`.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# constructed-receiver type existence checks explicit"
)]
fn csharp_receiver_is_constructed_type(
    source_symbol: &IndexedSymbol,
    constructed_receiver: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> bool {
    let Some(type_name) = constructed_receiver.strip_suffix("()") else {
        return false;
    };
    if type_name.is_empty()
        || type_name
            .split('.')
            .any(|segment| segment.is_empty() || segment.contains(['[', ']', '(', ')', '?']))
    {
        return false;
    }
    let Ok(Some(binding)) = resolve_csharp_receiver_type_binding(
        source_symbol,
        type_name,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    ) else {
        return false;
    };
    csharp_dispatchable_type_path(
        source_symbol,
        raw_symbols,
        &binding,
        csharp_is_type_declaration,
    )
    .is_some()
}

/// Extends an element-access receiver such as `global::Demo` or `Sub` to the
/// longest dotted prefix that resolves as a unique type, so a root such as
/// `global::Demo.Util.holder?.items[0]` or `Sub.Util2.holder?.items[0]`
/// splits with the full type path as the receiver and the remaining member
/// chain after it. Namespace-only prefixes are skipped, so a deep namespace
/// such as `global::Demo.Sub.Util.holder?.items[0]` still absorbs
/// `Demo.Sub.Util`, a nested type such as `Outer.Inner` in
/// `Outer.Inner.holder?.items[0]` absorbs both segments, and a dotted type
/// path whose leading segment comes from a namespace import such as
/// `Sub.Util2.holder?.items[0]` with `using Root;` absorbs through the
/// imported namespace. `this` and `base` roots and receivers bound to a local
/// receiver return unchanged so locals shadow same-named type paths. Returns
/// the extended receiver and the number of leading chain segments absorbed
/// into it.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# element-access receiver absorption inputs explicit"
)]
fn csharp_qualified_element_access_receiver(
    source_symbol: &IndexedSymbol,
    receiver: &str,
    chain: &str,
    bindings: &CSharpReceiverTypeBindings,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<(String, usize)> {
    if receiver == "this" || receiver == "base" || bindings.contains(receiver) {
        return Ok((receiver.to_string(), 0));
    }
    let mut extended = receiver.to_string();
    let mut absorbed = 0usize;
    let mut best = extended.clone();
    let mut best_absorbed = 0usize;
    for segment in chain.split('.') {
        if segment.is_empty() || segment.contains(['(', '[', ']', ')']) {
            break;
        }
        extended.push('.');
        extended.push_str(segment);
        absorbed += 1;
        let resolves_as_scoped_type = resolve_csharp_static_initializer_type_path(
            source_symbol,
            &extended,
            raw_symbols,
            semantic_path_index,
        )
        .is_some();
        let resolves_as_imported_dotted_type = resolve_csharp_namespace_imported_dotted_type_path(
            source_symbol,
            &extended,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        .is_some();
        let resolves_as_alias_dotted_type = resolve_csharp_alias_to_dotted_type_path(
            source_symbol,
            &extended,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        .is_some();
        if resolves_as_scoped_type
            || resolves_as_imported_dotted_type
            || resolves_as_alias_dotted_type
        {
            best = extended.clone();
            best_absorbed = absorbed;
        }
    }
    Ok((best, best_absorbed))
}

/// Resolves the element component type binding of a qualified element-access
/// base such as `this.fieldItems` in `var fourth = this.fieldItems[0]`,
/// `base.inheritedItems` in `var sixth = base.inheritedItems[0]`,
/// `group.holder.fieldItems` in `var fifth = group.holder.fieldItems[0]`, or
/// `Util.fieldItems` in `var seventh = Util.fieldItems[0]`, or
/// `group.items` in `var first = group?.items[0]`, or `makeGroup().items` in
/// `var first = makeGroup()?.items[0]`. `this`-rooted bases start on the
/// enclosing type, `base`-rooted bases on the unique base type, other bound
/// receivers on their declared type, receivers bound to a factory or
/// member-chain marker (`var group = makeGroup()` or `var group = holder`)
/// resolve through the same factory and chain rules, a leading bare factory
/// call such as `makeGroup()` resolves as a factory on the enclosing type,
/// the unique base chain, or a static-imported type and walks the remaining
/// chain as an instance receiver, and unbound receivers on the named static
/// type (requiring a static terminal field). Intermediate hops resolve
/// through the same field/property/event and method-call-hop rules as member
/// chains, and the terminal hop must be a uniquely declared array member
/// whose element component type pins the receiver, stripping one component
/// layer per element-access depth (so `this.fieldMatrix[0][0]` over
/// `Helper[][]` dispatches on `Helper`). Unknown, ambiguous, or non-array
/// terminal members, unbound or non-array receivers, unresolved leading
/// factories, static type-qualified method-call bases, and non-static fields
/// on a static type receiver fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# qualified element-access base resolution inputs explicit"
)]
fn csharp_qualified_element_access_component_type_path(
    source_symbol: &IndexedSymbol,
    base_reference: &str,
    depth: usize,
    bindings: &CSharpReceiverTypeBindings,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    // A constructed-receiver root such as `new Group()` in
    // `new Group().holder?.items[0]` normalizes to the same dotted
    // `Type().<chain>` shape as the `@init:` chain spellings (stripping the
    // constructor argument list or object-initializer body and normalizing
    // generic type arguments), and is tracked as constructed so dispatch
    // never falls through to the bare factory-call interpretation.
    let (receiver, chain, constructed_root) =
        if let Some(rest) = base_reference.strip_prefix("new ") {
            let Some(constructed) = csharp_constructed_factory_call_spelling(rest) else {
                return Ok(None);
            };
            let Some(marker) = constructed.find("()") else {
                return Ok(None);
            };
            let receiver = &constructed[..marker + 2];
            let Some(chain) = constructed[marker + 2..].strip_prefix('.') else {
                return Ok(None);
            };
            if receiver.is_empty() || chain.is_empty() {
                return Ok(None);
            }
            (receiver.to_string(), chain.to_string(), true)
        } else {
            let (receiver, chain) = if let Some((constructed_receiver, constructed_chain)) =
                csharp_constructed_receiver_split(base_reference)
                && csharp_receiver_is_constructed_type(
                    source_symbol,
                    &constructed_receiver,
                    raw_symbols,
                    semantic_path_index,
                    source_namespace_path,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                ) {
                (constructed_receiver, constructed_chain)
            } else if let Some((receiver, chain)) = csharp_top_level_dot_split(base_reference) {
                (receiver.to_string(), chain.to_string())
            } else {
                return Ok(None);
            };
            let (receiver, chain) = (receiver.as_str(), chain.as_str());
            if receiver.is_empty() || chain.is_empty() {
                return Ok(None);
            }
            if let Some(inner) = csharp_outer_parenthesized_inner(receiver) {
                // A parenthesized receiver root such as `(new Group())` in
                // `(new Group()).holder?.items[0]`, `(Util.MakeGroup())` in
                // `(Util.MakeGroup())?.items[0]`, or `(group)` in
                // `(group)?.items[0]` unwraps to the same dispatch shapes as
                // its unparenthesized spelling.
                let normalized_base = if chain.is_empty() {
                    inner.to_string()
                } else {
                    format!("{inner}.{chain}")
                };
                return csharp_qualified_element_access_component_type_path(
                    source_symbol,
                    &normalized_base,
                    depth,
                    bindings,
                    raw_symbols,
                    semantic_path_index,
                    source_namespace_path,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                );
            }
            (
                receiver.to_string(),
                chain.to_string(),
                receiver.ends_with("()"),
            )
        };
    let (receiver, absorbed_chain_segments) = csharp_qualified_element_access_receiver(
        source_symbol,
        &receiver,
        &chain,
        bindings,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    let chain = chain
        .split('.')
        .skip(absorbed_chain_segments)
        .collect::<Vec<_>>()
        .join(".");
    let (receiver, chain) = (receiver.as_str(), chain.as_str());
    let mut hops = chain.split('.').map(str::to_string).collect::<Vec<_>>();
    if hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    // The terminal hop is the accessed array member; a call-shaped or
    // bracket-shaped terminal is not a plain field and fails closed.
    let Some(terminal) = hops.pop() else {
        return Ok(None);
    };
    if terminal.contains(['(', '[', ']', ')']) {
        return Ok(None);
    }
    let (initial_binding, scope_source_symbol, require_static_terminal) = if receiver == "this" {
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        let type_candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.file_path == source_symbol.file_path
                    && candidate.semantic_path == scope_path
                    && csharp_is_type_declaration(candidate)
            })
            .collect::<Vec<_>>();
        if type_candidates.len() != 1 {
            return Ok(None);
        }
        (
            CSharpBaseTypeBinding {
                semantic_type_path: scope_path.to_string(),
                is_global_qualified: true,
                alias_name: None,
                namespace_import_paths: Vec::new(),
                generic_arguments: Vec::new(),
                raw_generic_argument_spellings: Vec::new(),
                enclosing_generic_arguments: Vec::new(),
                raw_enclosing_generic_argument_spellings: Vec::new(),
            },
            type_candidates[0],
            false,
        )
    } else if receiver == "base" {
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        let type_candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.file_path == source_symbol.file_path
                    && candidate.semantic_path == scope_path
                    && csharp_is_type_declaration(candidate)
            })
            .collect::<Vec<_>>();
        if type_candidates.len() != 1 {
            return Ok(None);
        }
        let type_symbol = type_candidates[0];
        let Some(base_binding) = csharp_base_type_binding_for_type(
            type_symbol,
            raw_symbols,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let Some(base_type_path) = csharp_base_type_path(type_symbol, raw_symbols, &base_binding)
        else {
            return Ok(None);
        };
        let base_indexes = semantic_path_index
            .get(&base_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_base_constructible_type(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if base_indexes.len() != 1 {
            return Ok(None);
        }
        let base_symbol = &raw_symbols[base_indexes[0]];
        (
            CSharpBaseTypeBinding {
                semantic_type_path: base_type_path,
                is_global_qualified: true,
                alias_name: None,
                namespace_import_paths: Vec::new(),
                generic_arguments: Vec::new(),
                raw_generic_argument_spellings: Vec::new(),
                enclosing_generic_arguments: Vec::new(),
                raw_enclosing_generic_argument_spellings: Vec::new(),
            },
            base_symbol,
            false,
        )
    } else if let Some(open) = receiver.find('[')
        && receiver.ends_with(']')
        && open > 0
        && let Some(depth) = csharp_array_access_depth(receiver)
    {
        // An element-access receiver such as `items[0]` in
        // `var first = items[0].innerItems[0]` or
        // `var first = items[0].GetOuterBox().items[0]` dispatches on the
        // base array's element component type (a bound array local, a bound
        // `var` local initialized from a factory-returned array, or a
        // factory-call spelling), stripping one component layer per
        // element-access depth; indexing a non-array or primitive-array
        // base, or an element access deeper than the base's array layers,
        // fails closed.
        let Some(binding) = csharp_array_element_component_binding(
            source_symbol,
            &receiver[..open],
            depth,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        (binding, source_symbol, false)
    } else if bindings.contains(receiver) {
        // A bound receiver pins its declared type, whether typed directly
        // or bound to a factory, member-chain, or element-access marker
        // (`var group = makeGroup()`, `var group = holder`, or
        // `var first = items[0]`), before the terminal array member walk;
        // untyped or unresolvable bound receivers fail closed instead of
        // falling through to a same-named static type.
        let Some(binding) = resolve_csharp_bound_factory_receiver_binding(
            source_symbol,
            receiver,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let Some(binding) = canonicalize_csharp_type_binding(source_symbol, &binding, raw_symbols)
        else {
            return Ok(None);
        };
        (binding, source_symbol, false)
    } else if constructed_root
        && let Some(type_name) = receiver.strip_suffix("()")
        && !type_name.is_empty()
        && !type_name
            .split('.')
            .any(|segment| segment.is_empty() || segment.contains(['[', ']', '(', ')', '?']))
        && let Some(binding) = resolve_csharp_receiver_type_binding(
            source_symbol,
            type_name,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && csharp_dispatchable_type_path(
            source_symbol,
            raw_symbols,
            &binding,
            csharp_is_type_declaration,
        )
        .is_some()
    {
        // A constructed-receiver root such as `new Group()` in
        // `new Group().holder?.items[0]` or
        // `foreach (var item in new Group().holder?.items)` resolves the
        // constructed type, then walks any intermediate field hops and the
        // terminal array member as an instance receiver; an unbound name such
        // as `makeGroup()` in `makeGroup().items[0]` falls through to the
        // bare factory-call interpretation instead of failing the whole
        // chain, and unknown constructed types fail closed.
        let Some(binding) = canonicalize_csharp_type_binding(source_symbol, &binding, raw_symbols)
        else {
            return Ok(None);
        };
        (binding, source_symbol, false)
    } else if let Some(mut leading_call) =
        csharp_outer_parenthesized_inner(receiver).or(Some(receiver))
        && {
            while let Some(inner) = csharp_outer_parenthesized_inner(leading_call) {
                leading_call = inner;
            }
            true
        }
        && let Some((leading_name, leading_arity)) = csharp_method_call_hop_spelling(leading_call)
        && let Some(method_type_arguments) = csharp_method_type_arguments(leading_call)
        && let Some(leading_method) = resolve_csharp_var_factory_method(
            source_symbol,
            &leading_name,
            leading_arity,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(leading_return) = leading_method.return_type.as_deref()
        && !leading_return.is_empty()
        && let Ok(leading_return) = substitute_csharp_method_type_parameters(
            leading_method,
            &method_type_arguments,
            leading_return,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )
        && let Some(leading_binding) = resolve_csharp_receiver_type_binding(
            leading_method,
            &leading_return,
            raw_symbols,
            semantic_path_index,
            csharp_source_namespace_path(leading_method, raw_symbols).flatten(),
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        // A leading bare factory call such as `makeGroup()` in
        // `makeGroup()?.items[0]` or `foreach (var item in makeGroup()?.items)`
        // resolves the call as a factory on the enclosing type, the unique
        // base chain, or a static-imported type, canonicalizes its declared
        // return type, and walks the remaining chain and terminal array
        // member as an instance receiver.
        let Some(leading_binding) =
            canonicalize_csharp_type_binding(leading_method, &leading_binding, raw_symbols)
        else {
            return Ok(None);
        };
        (leading_binding, source_symbol, false)
    } else if let Some(leading_hop) = hops.first()
        && let Some((leading_hop_name, leading_hop_arity, leading_hop_depth)) =
            csharp_method_call_element_access_spelling(leading_hop)
        && let Some(method_type_arguments) = csharp_method_type_arguments(leading_hop)
        && let Some(receiver_binding) = resolve_csharp_receiver_type_binding(
            source_symbol,
            receiver,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(receiver_type_path) = csharp_dispatchable_type_path(
            source_symbol,
            raw_symbols,
            &receiver_binding,
            csharp_is_type_declaration,
        )
        && let Some(leading_factory) = resolve_csharp_var_factory_method(
            source_symbol,
            &format!("{receiver}.{leading_hop_name}"),
            leading_hop_arity,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(leading_return) = leading_factory.return_type.as_deref()
        && !leading_return.is_empty()
        && let Ok(leading_return) = substitute_csharp_method_return_type(
            leading_factory,
            &receiver_binding,
            &receiver_type_path,
            leading_return,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )
        && let Ok(leading_return) = substitute_csharp_method_type_parameters(
            leading_factory,
            &method_type_arguments,
            &leading_return,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )
        && let Some(leading_component) =
            csharp_array_component_spelling_at_depth(&leading_return, leading_hop_depth)
        && let Some(leading_binding) = resolve_csharp_member_hop_type_binding(
            leading_factory,
            &leading_component,
            &receiver_binding,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        // A leading static type-qualified factory call with an element-access
        // suffix such as `Factory.MakeNestedArray()[0]` in
        // `Factory.MakeNestedArray()[0].innerItems[0]` resolves the call as a
        // static factory on the named type, strips one return-array component
        // layer per element-access depth, canonicalizes the resulting binding,
        // consumes the leading call hop, and walks the remaining chain and
        // terminal array member as an instance receiver; unknown or
        // arity-mismatched static factories and non-array or primitive-array
        // returns fail closed.
        let Some(leading_binding) =
            canonicalize_csharp_type_binding(leading_factory, &leading_binding, raw_symbols)
        else {
            return Ok(None);
        };
        hops.remove(0);
        (leading_binding, source_symbol, false)
    } else if let Some(leading_hop) = hops.first()
        && let Some((leading_hop_name, leading_hop_arity)) =
            csharp_method_call_hop_spelling(leading_hop)
        && let Some(method_type_arguments) = csharp_method_type_arguments(leading_hop)
        && let Some(receiver_binding) = resolve_csharp_receiver_type_binding(
            source_symbol,
            receiver,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(receiver_type_path) = csharp_dispatchable_type_path(
            source_symbol,
            raw_symbols,
            &receiver_binding,
            csharp_is_type_declaration,
        )
        && let Some(leading_factory) = resolve_csharp_var_factory_method(
            source_symbol,
            &format!("{receiver}.{leading_hop_name}"),
            leading_hop_arity,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(leading_return) = leading_factory.return_type.as_deref()
        && !leading_return.is_empty()
        && let Ok(leading_return) = substitute_csharp_method_return_type(
            leading_factory,
            &receiver_binding,
            &receiver_type_path,
            leading_return,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )
        && let Ok(leading_return) = substitute_csharp_method_type_parameters(
            leading_factory,
            &method_type_arguments,
            &leading_return,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )
        && let Some(leading_binding) = resolve_csharp_member_hop_type_binding(
            leading_factory,
            &leading_return,
            &receiver_binding,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        // A leading static type-qualified factory call such as
        // `Util.MakeGroup()` in `Util.MakeGroup()?.items[0]` or
        // `foreach (var item in Util.MakeGroup()?.items)` resolves the call
        // as a static factory on the named type, canonicalizes its declared
        // return type, consumes the leading call hop, and walks the remaining
        // chain and terminal array member as an instance receiver; unknown or
        // arity-mismatched static factories fail closed.
        let Some(leading_binding) =
            canonicalize_csharp_type_binding(leading_factory, &leading_binding, raw_symbols)
        else {
            return Ok(None);
        };
        hops.remove(0);
        (leading_binding, source_symbol, false)
    } else if let Some(leading_hop) = hops.first()
        && let Some((leading_member, leading_element_depth)) =
            csharp_static_member_element_access_spelling(leading_hop)
        && receiver.contains('<')
        && let Some(receiver_binding) = resolve_csharp_receiver_type_binding(
            source_symbol,
            receiver,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(leading_binding) = resolve_csharp_constructed_static_receiver_member_binding(
            source_symbol,
            &receiver_binding,
            &leading_member,
            leading_element_depth,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        // A leading static member on a constructed static receiver such as
        // `StaticNested` in `Outer<HelperA>.Inner<HelperB>.StaticNested
        // .Items[0]` or `StaticNestedArray[0]` in
        // `Outer<HelperA>.Inner<HelperB>.StaticNestedArray[0].Items[0]`
        // resolves the member's declared type through the receiver's
        // concrete generic arguments (so `Inner<U> StaticNested` pins
        // `Inner<HelperB>` and `Inner<U>[] StaticNestedArray[0]` pins the
        // same component), strips one array component layer per
        // element-access depth, consumes the leading hop, and walks the
        // remaining chain and terminal array member as an instance receiver;
        // unknown, instance, non-array, or unresolvable members fail closed.
        hops.remove(0);
        (leading_binding, source_symbol, false)
    } else if let Some(leading_field) = hops.first()
        && !leading_field.contains(['(', '[', ']', ')'])
        && let Some(leading_binding) = resolve_csharp_static_field_initializer_binding(
            source_symbol,
            &format!("{receiver}.{leading_field}"),
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        // A leading static type-qualified field or property hop such as
        // `Util.holder` in `Util.holder?.items[0]` or
        // `foreach (var item in Util.holder?.items)` resolves the static
        // member's declared type on the named type (through the same
        // same-namespace, namespace-imported, and alias rules as
        // receiver type references), consumes the leading hop, and walks the
        // remaining chain and terminal array member as an instance receiver;
        // unknown or instance-member roots and non-array terminals fail
        // closed.
        hops.remove(0);
        (leading_binding, source_symbol, false)
    } else if !receiver.contains(['.', '(', '[', ']', ')'])
        && let Some(binding) = resolve_csharp_inherited_field_initializer_binding(
            source_symbol,
            receiver,
            &[],
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        // A bare unbound receiver that resolves as an inherited field or
        // property root such as `holder` in `holder?.items[0]` or
        // `holder?.holder?.items[0]` from a type that inherits the member from
        // a base class pins the receiver to the base member's declared type
        // and walks the remaining chain and terminal array member as an
        // instance receiver; unknown or primitive-typed base members and
        // unresolved base chains fail closed.
        (binding, source_symbol, false)
    } else if !receiver.contains(['.', '(', '[', ']', ')'])
        && let Some(binding) = resolve_csharp_static_imported_field_initializer_binding(
            source_symbol,
            receiver,
            &[],
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        // A bare unbound receiver that resolves as a static-imported field
        // or property root such as `holder` in `holder?.items[0]` with
        // `using static Root.Sub.Util2;` pins the receiver to the imported
        // member's declared type and walks the remaining chain and terminal
        // array member as an instance receiver; unknown or instance-member
        // imports fail closed.
        (binding, source_symbol, false)
    } else {
        // An unbound receiver names a static type; the terminal array member
        // must be declared static on that type.
        let Some(binding) = resolve_csharp_receiver_type_binding(
            source_symbol,
            receiver,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        (binding, source_symbol, true)
    };
    // Intermediate hops walk the same field/property/event and method-call-hop
    // rules as any other member chain.
    let intermediate_refs = hops.iter().map(String::as_str).collect::<Vec<_>>();
    let (binding, scope_source_symbol) = if intermediate_refs.is_empty() {
        (initial_binding, scope_source_symbol)
    } else {
        let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
            scope_source_symbol,
            initial_binding,
            &intermediate_refs,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        (binding, scope_source_symbol)
    };
    if require_static_terminal {
        // A static type receiver requires the terminal array member to be
        // declared static on the resolved type or a unique class/record
        // ancestor (the nearest declaration pins the declaring type and its
        // declared type); the element component type resolves in the
        // declaring type's own file and enclosing scope. Unknown members,
        // instance-member terminals, and unresolvable or cyclic base chains
        // fail closed.
        let Some(type_path) = csharp_dispatchable_type_path(
            scope_source_symbol,
            raw_symbols,
            &binding,
            csharp_is_type_declaration,
        ) else {
            return Ok(None);
        };
        let type_indexes = semantic_path_index
            .get(&type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if type_indexes.len() != 1 {
            return Ok(None);
        }
        let mut type_symbol = &raw_symbols[type_indexes[0]];
        let mut current_generic_arguments = binding.generic_arguments.clone();
        let mut current_enclosing_generic_arguments = binding.enclosing_generic_arguments.clone();
        let mut visited_type_paths = BTreeSet::new();
        let member_bindings = loop {
            let Some(bindings) = csharp_member_type_bindings_for_type(
                &type_symbol.file_path,
                type_symbol.byte_range,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            if bindings.contains(&terminal) {
                break bindings;
            }
            if type_symbol.node_kind == "interface_declaration" {
                return Ok(None);
            }
            let Some(base_binding) = csharp_base_type_binding_for_type(
                type_symbol,
                raw_symbols,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            let Some(base_type_path) =
                csharp_base_type_path(type_symbol, raw_symbols, &base_binding)
            else {
                return Ok(None);
            };
            if !visited_type_paths.insert(base_type_path.clone()) {
                return Ok(None);
            }
            let base_indexes = semantic_path_index
                .get(&base_type_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                .collect::<Vec<_>>();
            if base_indexes.len() != 1 {
                return Ok(None);
            }
            // Compose the constructed base binding's concrete arguments by
            // substituting the current receiver's arguments into the base
            // spelling's raw type-argument spellings, so a base such as
            // `GenericBase<HelperB>` reached through a non-generic
            // `FixedDerived : GenericBase<HelperB>` pins the same concrete
            // arguments for the static member's declared type.
            let parameters = csharp_type_parameter_names_for_type(
                &type_symbol.file_path,
                type_symbol.byte_range,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            .unwrap_or_default();
            let generic_arguments = base_binding
                .raw_generic_argument_spellings
                .iter()
                .map(|spelling| {
                    substitute_csharp_type_parameters(
                        spelling,
                        &parameters,
                        &current_generic_arguments,
                    )
                })
                .collect::<Vec<_>>();
            let enclosing_generic_arguments = csharp_base_step_enclosing_arguments(
                &base_binding,
                &base_type_path,
                type_symbol.semantic_path.as_str(),
                &current_enclosing_generic_arguments,
                &parameters,
                &current_generic_arguments,
                raw_symbols,
                semantic_path_index,
            );
            current_generic_arguments = generic_arguments;
            current_enclosing_generic_arguments = enclosing_generic_arguments;
            type_symbol = &raw_symbols[base_indexes[0]];
        };
        if !member_bindings.is_static_member(&terminal) {
            return Ok(None);
        }
        let Some(declared_type) = member_bindings.type_for(&terminal) else {
            return Ok(None);
        };
        // A constructed static receiver such as
        // `Outer<HelperA>.Inner<HelperB>.StaticItems` substitutes the
        // receiver's concrete generic arguments into the static member's
        // declared type before stripping the array component layers, so
        // `U[] StaticItems` resolves to `HelperB[]` (and an
        // outer-parameter member `T[]` resolves to `HelperA[]`); the
        // composed arguments follow the class/record ancestor chain so a
        // member inherited from a constructed base substitutes the base's
        // type parameters with the derived receiver's concrete arguments.
        let mut declared_type = declared_type.to_string();
        if let Some(parameters) = csharp_type_parameter_names_for_type(
            &type_symbol.file_path,
            type_symbol.byte_range,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            declared_type = substitute_csharp_type_parameters(
                &declared_type,
                &parameters,
                &current_generic_arguments,
            );
        }
        declared_type = substitute_csharp_enclosing_type_parameters(
            type_symbol,
            &current_enclosing_generic_arguments,
            &declared_type,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?;
        let Some(component_type) = csharp_array_component_spelling_at_depth(&declared_type, depth)
        else {
            return Ok(None);
        };
        let Some(result) = resolve_csharp_member_hop_type_binding(
            type_symbol,
            &component_type,
            &binding,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        // The element component binding resolves in the declaring type's own
        // file and enclosing scope; canonicalize it so callers in other
        // namespaces dispatch on the canonical declared type.
        Ok(canonicalize_csharp_type_binding(
            type_symbol,
            &result,
            raw_symbols,
        ))
    } else {
        // The terminal hop is the accessed array member; mark it as one
        // element-access hop per depth so the member-chain walk requires an
        // array member with enough layers and pins its element component
        // type.
        let terminal_refs = [format!("{terminal}{}", "[0]".repeat(depth))];
        let terminal_refs = terminal_refs.iter().map(String::as_str).collect::<Vec<_>>();
        let Some((binding, terminal_scope_symbol)) = resolve_csharp_member_chain_binding(
            scope_source_symbol,
            binding,
            &terminal_refs,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            // A `this.`-rooted bare chain whose member is not declared on
            // the enclosing type or its ancestors falls back to the unbound
            // bare member root rules (an inherited base-class or
            // static-imported array member), so
            // `foreach (var item in STATIC_MATRIX)` with
            // `using static Demo.Util;` binds the loop variable to the
            // imported array's element component type; unknown imports and
            // non-array members fail closed.
            if receiver == "this"
                && hops.is_empty()
                && let Some(binding) = resolve_csharp_unbound_bare_member_array_component_binding(
                    source_symbol,
                    &terminal,
                    depth,
                    raw_symbols,
                    semantic_path_index,
                    source_namespace_path,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
            {
                return Ok(Some(binding));
            }
            return Ok(None);
        };
        // The element component binding resolves in the declaring type's own
        // file and enclosing scope; canonicalize it so callers in other
        // namespaces dispatch on the canonical declared type.
        let Some(binding) =
            canonicalize_csharp_type_binding(terminal_scope_symbol, &binding, raw_symbols)
        else {
            return Ok(None);
        };
        Ok(Some(binding))
    }
}

/// Resolves the element component binding for a bare `var` initializer
/// chain whose leading member is not a bound local or enclosing field, such
/// as `var items = StaticNestedArray` followed by `items[0]` with
/// `using static Lib.Plain;` (a static-imported array member) or
/// `var items = baseItems` followed by `items[0]` on a bare inherited
/// base-class field. The member's declared array type resolves in its
/// declaring type's own file and enclosing scope, stripping one component
/// layer per element-access depth; unknown, instance, ambiguous, non-array,
/// and primitive-array members fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# bare unbound member array component resolution inputs explicit"
)]
fn resolve_csharp_unbound_bare_member_array_component_binding(
    source_symbol: &IndexedSymbol,
    member_name: &str,
    depth: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    if member_name.is_empty()
        || member_name.contains('.')
        || !is_safe_csharp_identifier(member_name)
    {
        return Ok(None);
    }
    // A bare inherited base-class field or property root resolves first,
    // walking the unique base-type chain from the nearest ancestor outward;
    // the first ancestor that declares the member pins the receiver.
    if let Some(scope_path) = source_symbol.scope_path.as_deref() {
        let enclosing_candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.file_path == source_symbol.file_path
                    && candidate.semantic_path == scope_path
                    && csharp_is_type_declaration(candidate)
            })
            .collect::<Vec<_>>();
        if enclosing_candidates.len() == 1 {
            let mut ancestor_symbol = enclosing_candidates[0];
            let mut current_generic_arguments = Vec::new();
            let mut current_enclosing_generic_arguments = Vec::new();
            let mut visited_type_paths = BTreeSet::new();
            for _ in 0..64 {
                let Some(base_binding) = csharp_base_type_binding_for_type(
                    ancestor_symbol,
                    raw_symbols,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                else {
                    break;
                };
                let Some(base_type_path) = csharp_dispatchable_type_path(
                    ancestor_symbol,
                    raw_symbols,
                    &base_binding,
                    csharp_is_type_declaration,
                ) else {
                    break;
                };
                if !visited_type_paths.insert(base_type_path.clone()) {
                    break;
                }
                let base_indexes = semantic_path_index
                    .get(&base_type_path)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                    .collect::<Vec<_>>();
                if base_indexes.len() != 1 {
                    break;
                }
                let base_symbol = &raw_symbols[base_indexes[0]];
                // Compose the constructed base binding's concrete arguments
                // by substituting the current receiver's arguments into the
                // base spelling's raw type-argument spellings, so a base such
                // as `Base<T>` reached through `Caller : Base<Helper>` pins
                // `T` to `Helper` (and a member declared as `T[,]` resolves
                // its element component on `Helper`).
                let parameters = csharp_type_parameter_names_for_type(
                    &ancestor_symbol.file_path,
                    ancestor_symbol.byte_range,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                .unwrap_or_default();
                let generic_arguments = base_binding
                    .raw_generic_argument_spellings
                    .iter()
                    .map(|spelling| {
                        substitute_csharp_type_parameters(
                            spelling,
                            &parameters,
                            &current_generic_arguments,
                        )
                    })
                    .collect::<Vec<_>>();
                let enclosing_generic_arguments = csharp_base_step_enclosing_arguments(
                    &base_binding,
                    &base_type_path,
                    ancestor_symbol.semantic_path.as_str(),
                    &current_enclosing_generic_arguments,
                    &parameters,
                    &current_generic_arguments,
                    raw_symbols,
                    semantic_path_index,
                );
                current_generic_arguments = generic_arguments;
                current_enclosing_generic_arguments = enclosing_generic_arguments;
                let Some(member_bindings) = csharp_member_type_bindings_for_type(
                    &base_symbol.file_path,
                    base_symbol.byte_range,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                else {
                    break;
                };
                if member_bindings.contains(member_name) {
                    let Some(declared_type) = member_bindings.type_for(member_name) else {
                        break;
                    };
                    // A member declared on a generic base substitutes the
                    // base's type parameters with the composed concrete
                    // arguments before stripping the array component layers,
                    // so `T[,]` on `Base<T>` reached through
                    // `Caller : Base<Helper>` resolves its element component
                    // on `Helper`; outer-parameter members substitute the
                    // enclosing generic arguments.
                    let mut declared_type = declared_type.to_string();
                    if let Some(parameters) = csharp_type_parameter_names_for_type(
                        &base_symbol.file_path,
                        base_symbol.byte_range,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                    )? {
                        declared_type = substitute_csharp_type_parameters(
                            &declared_type,
                            &parameters,
                            &current_generic_arguments,
                        );
                    }
                    let declared_type = substitute_csharp_enclosing_type_parameters(
                        base_symbol,
                        &current_enclosing_generic_arguments,
                        &declared_type,
                        raw_symbols,
                        semantic_path_index,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                    )?;
                    let Some(component_type) =
                        csharp_array_component_spelling_at_depth(&declared_type, depth)
                    else {
                        break;
                    };
                    let Some(binding) = resolve_csharp_receiver_type_binding(
                        base_symbol,
                        &component_type,
                        raw_symbols,
                        semantic_path_index,
                        csharp_source_namespace_path(base_symbol, raw_symbols).flatten(),
                        csharp_global_import_context,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                    )?
                    else {
                        break;
                    };
                    return Ok(canonicalize_csharp_type_binding(
                        base_symbol,
                        &binding,
                        raw_symbols,
                    ));
                }
                ancestor_symbol = base_symbol;
            }
        }
    }
    // A static-imported member root such as `StaticNestedArray` in
    // `var items = StaticNestedArray` with `using static Lib.Plain;` resolves
    // the member on the uniquely imported type, requiring a static field or
    // property whose declared array type pins the element component.
    let mut static_type_imports = resolve_csharp_static_type_imports_for_reference(
        &source_symbol.file_path,
        member_name,
        source_namespace_path,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    if let Some(csharp_global_import_context) = csharp_global_import_context {
        static_type_imports.extend(resolve_csharp_global_static_type_imports_for_reference(
            member_name,
            csharp_global_import_context,
        ));
    }
    if static_type_imports.is_empty() {
        return Ok(None);
    }
    let mut candidates = Vec::new();
    for import in &static_type_imports {
        let type_indexes = semantic_path_index
            .get(&import.semantic_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if type_indexes.len() != 1 {
            continue;
        }
        let type_symbol = &raw_symbols[type_indexes[0]];
        // A static import brings in inherited static members too, so
        // `using static Lib.Plain;` with `Plain : PlainBase` imports
        // `StaticNestedArray` declared on `PlainBase`; the nearest
        // class/record ancestor that declares the member pins the declaring
        // type and its declared type.
        let member_owner = {
            let mut current_type_symbol = type_symbol;
            let mut current_generic_arguments = Vec::new();
            let mut current_enclosing_generic_arguments = Vec::new();
            let mut visited_type_paths = BTreeSet::new();
            let mut found = None;
            loop {
                let Some(bindings) = csharp_member_type_bindings_for_type(
                    &current_type_symbol.file_path,
                    current_type_symbol.byte_range,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                else {
                    break;
                };
                if bindings.contains(member_name) {
                    found = Some((
                        bindings,
                        current_type_symbol,
                        current_generic_arguments,
                        current_enclosing_generic_arguments,
                    ));
                    break;
                }
                if current_type_symbol.node_kind == "interface_declaration" {
                    break;
                }
                let Some(base_binding) = csharp_base_type_binding_for_type(
                    current_type_symbol,
                    raw_symbols,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                else {
                    break;
                };
                let Some(base_type_path) =
                    csharp_base_type_path(current_type_symbol, raw_symbols, &base_binding)
                else {
                    break;
                };
                if !visited_type_paths.insert(base_type_path.clone()) {
                    break;
                }
                let base_indexes = semantic_path_index
                    .get(&base_type_path)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                    .collect::<Vec<_>>();
                if base_indexes.len() != 1 {
                    break;
                }
                // Compose the constructed base binding's concrete arguments
                // by substituting the current receiver's arguments into the
                // base spelling's raw type-argument spellings, so a base such
                // as `GenericBase<HelperB>` reached through a non-generic
                // `FixedDerived : GenericBase<HelperB>` pins the same
                // concrete arguments for the member's declared type.
                let parameters = csharp_type_parameter_names_for_type(
                    &current_type_symbol.file_path,
                    current_type_symbol.byte_range,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                .unwrap_or_default();
                let generic_arguments = base_binding
                    .raw_generic_argument_spellings
                    .iter()
                    .map(|spelling| {
                        substitute_csharp_type_parameters(
                            spelling,
                            &parameters,
                            &current_generic_arguments,
                        )
                    })
                    .collect::<Vec<_>>();
                let enclosing_generic_arguments = csharp_base_step_enclosing_arguments(
                    &base_binding,
                    &base_type_path,
                    current_type_symbol.semantic_path.as_str(),
                    &current_enclosing_generic_arguments,
                    &parameters,
                    &current_generic_arguments,
                    raw_symbols,
                    semantic_path_index,
                );
                current_generic_arguments = generic_arguments;
                current_enclosing_generic_arguments = enclosing_generic_arguments;
                current_type_symbol = &raw_symbols[base_indexes[0]];
            }
            found
        };
        let Some((
            member_bindings,
            member_type_symbol,
            member_generic_arguments,
            member_enclosing_generic_arguments,
        )) = member_owner
        else {
            continue;
        };
        if !member_bindings.is_static_member(member_name) {
            continue;
        }
        let Some(declared_type) = member_bindings.type_for(member_name) else {
            continue;
        };
        let mut declared_type = declared_type.to_string();
        if let Some(parameters) = csharp_type_parameter_names_for_type(
            &member_type_symbol.file_path,
            member_type_symbol.byte_range,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            declared_type = substitute_csharp_type_parameters(
                &declared_type,
                &parameters,
                &member_generic_arguments,
            );
        }
        declared_type = substitute_csharp_enclosing_type_parameters(
            member_type_symbol,
            &member_enclosing_generic_arguments,
            &declared_type,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?;
        let Some(component_type) = csharp_array_component_spelling_at_depth(&declared_type, depth)
        else {
            continue;
        };
        let Some(binding) = resolve_csharp_receiver_type_binding(
            member_type_symbol,
            &component_type,
            raw_symbols,
            semantic_path_index,
            csharp_source_namespace_path(member_type_symbol, raw_symbols).flatten(),
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            continue;
        };
        if let Some(canonical) =
            canonicalize_csharp_type_binding(member_type_symbol, &binding, raw_symbols)
        {
            candidates.push(canonical);
        }
    }
    Ok(match candidates.as_slice() {
        [_] => candidates.pop(),
        _ => None,
    })
}

/// Resolves the element component binding for an element-access base
/// spelling at a given array depth, whether the base is a bound local with
/// a directly declared array type (`items` in `items[0].GetOuterItem()`), a
/// bound `var` local initialized from a factory call whose declared return
/// type is an array (`var items = Factory.MakeNestedArray()` in
/// `items[0].GetOuterItem()`), or a factory-call spelling itself
/// (`Factory.MakeNestedArray()` in
/// `Factory.MakeNestedArray()[0].GetOuterItem()`). The element component is
/// stripped one layer per element-access depth, resolving the factory's
/// declared return array in the factory's own file and enclosing scope when
/// the base is a factory, so the component can carry constructed generic
/// arguments for later outer-parameter substitution. Non-array or
/// primitive-array bases, unresolvable or arity-mismatched factories, and
/// depths beyond the base array's layer count return `None` and fail
/// closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# element-access component resolution inputs explicit"
)]
fn csharp_array_element_component_binding(
    source_symbol: &IndexedSymbol,
    base: &str,
    depth: usize,
    bindings: &CSharpReceiverTypeBindings,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    if base.is_empty() {
        return Ok(None);
    }
    if let Some(component_type) = bindings.array_component_for(base)
        && let Some(binding) = resolve_csharp_receiver_type_binding(
            source_symbol,
            &component_type,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        return Ok(Some(binding));
    }
    // A base that is itself a `var` local bound from an element-access
    // initializer, such as `row` in `var row = Factory.MakeNestedMatrix()[0]`
    // followed by `row[0].GetInnerItem()`, resolves the initializer's base
    // array at its recorded depth and strips one component layer per
    // additional element-access depth. A factory-call base resolves through
    // the same factory-array rules, a dotted member-chain base walks the
    // terminal array member, and a bare base resolves through its declared
    // array type or factory marker before the additional layers are stripped;
    // unresolvable bases and depths beyond the base array's layers fail
    // closed.
    if let Some((base_reference, base_arity, base_depth)) = bindings.element_access_base_for(base) {
        let combined_depth = base_depth + depth;
        // A base that is itself an element-access `var` local (such as `row`
        // in `var row = Factory.MakeNestedMatrix()[0]` followed by
        // `var first = row[0]`) recurses with the accumulated depth so each
        // intermediate initializer strips its recorded layers before the
        // terminal base (factory call, dotted chain, declared array type, or
        // marker-bound collection) resolves; chains that do not terminate in
        // a resolvable base fail closed.
        if bindings.element_access_base_for(&base_reference).is_some() {
            return csharp_array_element_component_binding(
                source_symbol,
                &base_reference,
                combined_depth,
                bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        if let Some(factory_call) = base_reference.strip_suffix("()") {
            return csharp_factory_array_component_binding(
                source_symbol,
                factory_call,
                base_arity,
                combined_depth,
                bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        if base_reference.contains('.') {
            return csharp_qualified_element_access_component_type_path(
                source_symbol,
                &base_reference,
                combined_depth,
                bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        if let Some(declared_type) = bindings.type_for(&base_reference)
            && let Some(component_type) =
                csharp_array_component_spelling_at_depth(&declared_type, combined_depth)
        {
            return resolve_csharp_receiver_type_binding(
                source_symbol,
                &component_type,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        if let Some(raw_binding) = bindings.raw_for(&base_reference)
            && let Some((factory_name, factory_arity)) = csharp_var_factory_spelling(raw_binding)
        {
            return csharp_factory_array_component_binding(
                source_symbol,
                &factory_name,
                factory_arity,
                combined_depth,
                bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        return Ok(None);
    }
    // A bound `var` local initialized from a factory call whose declared
    // return type is an array, such as `var items = Factory.MakeNestedArray()`
    // in `items[0].GetOuterItem()`, resolves through the factory's array
    // return type.
    if let Some(raw_binding) = bindings.raw_for(base)
        && let Some((factory_name, factory_arity)) = csharp_var_factory_spelling(raw_binding)
    {
        return csharp_factory_array_component_binding(
            source_symbol,
            &factory_name,
            factory_arity,
            depth,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    // A base bound from a `foreach` over a factory-returned array (such as
    // `row` in `foreach (var row in Factory.MakeNestedMatrix())` followed by
    // `row[0]`) resolves the factory return array's element component type one
    // layer deeper than the requested depth, since the loop variable is
    // already the element at depth one.
    if let Some(raw_binding) = bindings.raw_for(base)
        && let Some((factory_name, factory_arity)) =
            csharp_foreach_factory_element_spelling(raw_binding)
    {
        return csharp_factory_array_component_binding(
            source_symbol,
            &factory_name,
            factory_arity,
            depth + 1,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    // A base bound from a member-chain `var` initializer (such as `x` in
    // `var x = Plain.StaticNestedArray` followed by `x[0]`) resolves the
    // chain terminal's declared array type before stripping one component
    // layer per element-access depth; a dotted chain walks the terminal
    // array member through the qualified element-access path (including a
    // constructed static receiver root such as
    // `Outer<HelperA>.Inner<HelperB>.StaticNestedArray`), and a bare chain
    // names a bound field/property/local whose declared array type pins the
    // element component type directly. Unresolvable chains, marker-bound
    // chain terminals, and non-array or primitive-array terminals fail
    // closed.
    if let Some(chain) = bindings
        .raw_for(base)
        .and_then(csharp_var_initializer_chain_spelling)
    {
        if chain.contains('.') {
            return csharp_qualified_element_access_component_type_path(
                source_symbol,
                chain,
                depth,
                bindings,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        }
        let Some(declared_type) = bindings.raw_for(chain) else {
            // An unbound bare chain names a static-imported member root or
            // an inherited base-class field/property; its declared array type
            // resolves through the same inherited-then-static-imported rules
            // as bare-chain `var` initializers, stripping one component layer
            // per element-access depth.
            return resolve_csharp_unbound_bare_member_array_component_binding(
                source_symbol,
                chain,
                depth,
                raw_symbols,
                semantic_path_index,
                source_namespace_path,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            );
        };
        if declared_type.is_empty() || declared_type.starts_with('@') {
            return Ok(None);
        }
        let Some(component_type) = csharp_array_component_spelling_at_depth(declared_type, depth)
        else {
            return Ok(None);
        };
        return resolve_csharp_receiver_type_binding(
            source_symbol,
            &component_type,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    if let Some(declared_type) = bindings.type_for(base)
        && let Some(component_type) =
            csharp_array_component_spelling_at_depth(&declared_type, depth)
        && let Some(binding) = resolve_csharp_receiver_type_binding(
            source_symbol,
            &component_type,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    {
        return Ok(Some(binding));
    }
    // A factory-call base spelling such as `Factory.MakeNestedArray()` in
    // `Factory.MakeNestedArray()[0].GetOuterItem()` resolves the factory's
    // declared return array directly.
    if let Some((factory_name, factory_arity)) = csharp_method_call_hop_spelling(base) {
        return csharp_factory_array_component_binding(
            source_symbol,
            &factory_name,
            factory_arity,
            depth,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        );
    }
    Ok(None)
}

/// Resolves the element component type binding of a factory-call
/// element-access base such as `makeItems()` in `var first = makeItems()[0]`,
/// `Util.makeItems()` in `var second = Util.makeItems()[0]`, or
/// `makeMatrix()` in `var third = makeMatrix()[0][0]`: the leading call
/// resolves through the same factory rules as a `var` initializer (a unique
/// enclosing-type instance call, base-type or static-imported method, or
/// type-qualified static method with matching arity), and the component type
/// resolves in the factory's own file and enclosing scope, canonicalized so
/// callers in other namespaces dispatch on the canonical declared type,
/// stripping one element-component layer per depth. Unknown or
/// arity-mismatched factories and primitive return arrays fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# factory array component resolution inputs explicit"
)]
fn csharp_factory_array_component_binding(
    source_symbol: &IndexedSymbol,
    factory_reference: &str,
    factory_arity: usize,
    depth: usize,
    bindings: &CSharpReceiverTypeBindings,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    // An explicit generic method type-argument list at the call site such as
    // `MakeItems<HelperA>()` or `Factory.MakeNestedArray<HelperA>()` keeps
    // the trailing type-argument spellings for return-type substitution and
    // dispatches the factory method on the bare trailing name.
    let Some(method_type_arguments) = csharp_factory_method_type_arguments(factory_reference)
    else {
        return Ok(None);
    };
    let Some(factory_name) = csharp_factory_method_dispatch_name(factory_reference) else {
        return Ok(None);
    };
    let Some(method) = resolve_csharp_var_factory_method(
        source_symbol,
        &factory_name,
        factory_arity,
        bindings,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(return_type) = method.return_type.as_deref() else {
        return Ok(None);
    };
    // A fully-parenthesized constructed-receiver factory root such as
    // `(new Box<HelperA>()).GetItems()` unwraps to the same `new`-prefixed
    // shape so the receiver substitution below applies like its
    // unparenthesized spelling.
    let factory_spelling = csharp_parenthesized_constructed_factory_spelling(factory_reference)
        .unwrap_or_else(|| factory_reference.to_string());
    // A factory dispatched on a `new`-constructed receiver or a receiver
    // chain substitutes the receiver's concrete generic arguments into the
    // method's return type, so
    // `var first = new Box<HelperA>().GetItems()[0]` resolves the declared
    // `T[]` return to `HelperA[]` and the element component to `HelperA`.
    // Other factory shapes keep the declared return type, failing closed
    // downstream when it names a type parameter.
    let substituted_return_type = if let Some((receiver_name, method_name)) =
        factory_spelling.split_once('.')
        && !receiver_name.is_empty()
        && !method_name.is_empty()
        && !method_name.contains(['(', ')', '.'])
        && let Some(receiver_binding) = resolve_csharp_bound_factory_receiver_binding(
            source_symbol,
            receiver_name,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(receiver_type_path) = csharp_dispatchable_type_path(
            source_symbol,
            raw_symbols,
            &receiver_binding,
            csharp_is_type_declaration,
        ) {
        substitute_csharp_method_return_type(
            method,
            &receiver_binding,
            &receiver_type_path,
            return_type,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else if let Some((chain, trailing_method)) = factory_spelling.rsplit_once('.')
        && !chain.is_empty()
        && !trailing_method.is_empty()
        && !trailing_method.contains(['(', ')', '.'])
        && (chain.contains('.')
            || chain.ends_with(')')
            || chain.ends_with(']')
            || chain.ends_with('}'))
        && let Some(receiver_binding) = resolve_csharp_factory_chain_receiver_binding(
            source_symbol,
            chain,
            bindings,
            raw_symbols,
            semantic_path_index,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && let Some(receiver_type_path) = csharp_dispatchable_type_path(
            source_symbol,
            raw_symbols,
            &receiver_binding,
            csharp_is_type_declaration,
        )
    {
        substitute_csharp_method_return_type(
            method,
            &receiver_binding,
            &receiver_type_path,
            return_type,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    } else {
        csharp_substitute_bare_inherited_factory_return_type(
            source_symbol,
            method,
            return_type,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    };
    let substituted_return_type = substitute_csharp_method_type_parameters(
        method,
        &method_type_arguments,
        &substituted_return_type,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    let Some(component_name) =
        csharp_array_component_spelling_at_depth(&substituted_return_type, depth)
    else {
        return Ok(None);
    };
    let Some(component_binding) = resolve_csharp_receiver_type_binding(
        method,
        &component_name,
        raw_symbols,
        semantic_path_index,
        csharp_source_namespace_path(method, raw_symbols).flatten(),
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    Ok(canonicalize_csharp_type_binding(
        method,
        &component_binding,
        raw_symbols,
    ))
}

/// Resolves a static initializer type spelling such as `Util`,
/// `Outer.Util`, or `global::Demo.Util` to the unique semantic type path of
/// its type declaration. A `global::`-qualified spelling resolves exactly at
/// global scope; other spellings resolve through the caller's namespace
/// ancestors and then the global scope, mirroring declared-type and static
/// method resolution. Missing and ambiguous type declarations return `None`.
fn resolve_csharp_static_initializer_type_path(
    source_symbol: &IndexedSymbol,
    type_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Option<String> {
    if let Some(global_name) = type_name.strip_prefix("global::") {
        let semantic_path = crate::language::csharp_generic_type_semantic_path(global_name)?;
        let candidates = csharp_receiver_type_candidates(
            raw_symbols,
            semantic_path_index,
            &semantic_path,
            csharp_is_type_declaration,
        );
        return match candidates.as_slice() {
            [_] => Some(semantic_path),
            _ => None,
        };
    }
    let semantic_path = crate::language::csharp_generic_type_semantic_path(type_name)?;
    csharp_scoped_receiver_type_path(
        source_symbol,
        raw_symbols,
        semantic_path_index,
        &semantic_path,
        csharp_is_type_declaration,
    )
}

/// Resolves the receiver type binding for a `var` local initialized from a
/// static-imported member root such as `var helper = STATIC_HELPER` or
/// `var helper = STATIC_HELPER.entry` with `using static Demo.Util;`. The
/// imported type must declare the member as a static field or property with a
/// usable declared type; remaining hops walk the same member-chain rules on
/// that type. A name bound to an unusable local receiver shadows any
/// static-imported member and is handled by the caller. Unknown, ambiguous,
/// instance-member, missing-member, and primitive imports return `None` and
/// fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# static-imported field initializer binding inputs explicit"
)]
fn resolve_csharp_static_imported_field_initializer_binding<'a>(
    source_symbol: &'a IndexedSymbol,
    member_name: &str,
    hops: &[&str],
    raw_symbols: &'a [IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    // A root with an element-access suffix such as `StaticNestedArray[0]`
    // strips the brackets so the named static member resolves and one array
    // component layer per element access is stripped from its declared type;
    // plain roots keep depth zero.
    let (root_name, root_depth) = match (
        csharp_array_access_member_name(member_name),
        csharp_array_access_depth(member_name),
    ) {
        (Some(base), Some(depth)) => (base, depth),
        _ => (member_name, 0),
    };
    if root_name.is_empty()
        || root_name.contains('.')
        || !is_safe_csharp_identifier(root_name)
        || hops.iter().any(|hop| hop.is_empty())
    {
        return Ok(None);
    }
    let mut static_type_imports = resolve_csharp_static_type_imports_for_reference(
        &source_symbol.file_path,
        root_name,
        source_namespace_path,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    if let Some(csharp_global_import_context) = csharp_global_import_context {
        static_type_imports.extend(resolve_csharp_global_static_type_imports_for_reference(
            root_name,
            csharp_global_import_context,
        ));
    }
    if static_type_imports.is_empty() {
        return Ok(None);
    }
    // A member is a candidate only when the imported type resolves to one
    // unique declaration and declares the member as a static field or
    // property with a usable declared type; instance and missing members are
    // not candidates, mirroring static-method imports.
    let mut candidates = Vec::new();
    for import in &static_type_imports {
        let type_indexes = semantic_path_index
            .get(&import.semantic_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if type_indexes.len() != 1 {
            continue;
        }
        let type_symbol = &raw_symbols[type_indexes[0]];
        // A static import brings in inherited static members too, so
        // `using static Lib.Plain;` with `Plain : PlainBase` imports
        // `StaticNestedArray` declared on `PlainBase`; the nearest
        // class/record ancestor that declares the member pins the declaring
        // type and its declared type.
        let member_owner = {
            let mut current_type_symbol = type_symbol;
            let mut current_generic_arguments = Vec::new();
            let mut current_enclosing_generic_arguments = Vec::new();
            let mut visited_type_paths = BTreeSet::new();
            let mut found = None;
            loop {
                let Some(bindings) = csharp_member_type_bindings_for_type(
                    &current_type_symbol.file_path,
                    current_type_symbol.byte_range,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                else {
                    break;
                };
                if bindings.contains(root_name) {
                    found = Some((
                        bindings,
                        current_type_symbol,
                        current_generic_arguments,
                        current_enclosing_generic_arguments,
                    ));
                    break;
                }
                if current_type_symbol.node_kind == "interface_declaration" {
                    break;
                }
                let Some(base_binding) = csharp_base_type_binding_for_type(
                    current_type_symbol,
                    raw_symbols,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                else {
                    break;
                };
                let Some(base_type_path) =
                    csharp_base_type_path(current_type_symbol, raw_symbols, &base_binding)
                else {
                    break;
                };
                if !visited_type_paths.insert(base_type_path.clone()) {
                    break;
                }
                let base_indexes = semantic_path_index
                    .get(&base_type_path)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                    .collect::<Vec<_>>();
                if base_indexes.len() != 1 {
                    break;
                }
                // Compose the constructed base binding's concrete arguments
                // by substituting the current receiver's arguments into the
                // base spelling's raw type-argument spellings, so a base such
                // as `GenericBase<HelperB>` reached through a non-generic
                // `FixedDerived : GenericBase<HelperB>` pins the same
                // concrete arguments for the member's declared type.
                let parameters = csharp_type_parameter_names_for_type(
                    &current_type_symbol.file_path,
                    current_type_symbol.byte_range,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                .unwrap_or_default();
                let generic_arguments = base_binding
                    .raw_generic_argument_spellings
                    .iter()
                    .map(|spelling| {
                        substitute_csharp_type_parameters(
                            spelling,
                            &parameters,
                            &current_generic_arguments,
                        )
                    })
                    .collect::<Vec<_>>();
                let enclosing_generic_arguments = csharp_base_step_enclosing_arguments(
                    &base_binding,
                    &base_type_path,
                    current_type_symbol.semantic_path.as_str(),
                    &current_enclosing_generic_arguments,
                    &parameters,
                    &current_generic_arguments,
                    raw_symbols,
                    semantic_path_index,
                );
                current_generic_arguments = generic_arguments;
                current_enclosing_generic_arguments = enclosing_generic_arguments;
                current_type_symbol = &raw_symbols[base_indexes[0]];
            }
            found
        };
        let Some((
            member_bindings,
            member_type_symbol,
            member_generic_arguments,
            member_enclosing_generic_arguments,
        )) = member_owner
        else {
            continue;
        };
        if !member_bindings.is_static_member(root_name) {
            continue;
        }
        let Some(member_type_name) = member_bindings.type_for(root_name) else {
            continue;
        };
        let mut member_type_name = member_type_name.to_string();
        if let Some(parameters) = csharp_type_parameter_names_for_type(
            &member_type_symbol.file_path,
            member_type_symbol.byte_range,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            member_type_name = substitute_csharp_type_parameters(
                &member_type_name,
                &parameters,
                &member_generic_arguments,
            );
        }
        member_type_name = substitute_csharp_enclosing_type_parameters(
            member_type_symbol,
            &member_enclosing_generic_arguments,
            &member_type_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?;
        let Some(member_type_name) = (match root_depth {
            0 => Some(member_type_name),
            depth => csharp_array_component_spelling_at_depth(&member_type_name, depth),
        }) else {
            continue;
        };
        let Some(member_binding) = resolve_csharp_receiver_type_binding(
            member_type_symbol,
            &member_type_name,
            raw_symbols,
            semantic_path_index,
            csharp_source_namespace_path(member_type_symbol, raw_symbols).flatten(),
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            continue;
        };
        candidates.push((member_type_symbol, member_binding));
    }
    if candidates.len() != 1 {
        return Ok(None);
    }
    let (type_symbol, member_binding) = &candidates[0];
    if hops.is_empty() {
        return Ok(canonicalize_csharp_type_binding(
            type_symbol,
            member_binding,
            raw_symbols,
        ));
    }
    let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
        type_symbol,
        member_binding.clone(),
        hops,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    Ok(canonicalize_csharp_type_binding(
        scope_source_symbol,
        &binding,
        raw_symbols,
    ))
}

/// Resolves the receiver type binding for a `var` local initialized from a
/// bare inherited member root such as `var helper = holder` or
/// `var helper = holder.entry` where `holder` is a field or property declared
/// on an ancestor of the enclosing type. The nearest ancestor that declares
/// the member pins the receiver to the member's declared type; remaining hops
/// walk the same member-chain rules on that type. Names bound to unusable
/// local receivers, static type-qualified roots, and static-imported members
/// are handled by the caller before this path. Missing, primitive, `void`,
/// and unresolvable members return `None` and fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# inherited field initializer binding inputs explicit"
)]
fn resolve_csharp_inherited_field_initializer_binding<'a>(
    source_symbol: &'a IndexedSymbol,
    member_name: &str,
    hops: &[&str],
    raw_symbols: &'a [IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    if member_name.is_empty()
        || member_name.contains('.')
        || !is_safe_csharp_identifier(member_name)
        || hops.iter().any(|hop| hop.is_empty())
    {
        return Ok(None);
    }
    let Some(scope_path) = source_symbol.scope_path.as_deref() else {
        return Ok(None);
    };
    let enclosing_candidates = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.file_path == source_symbol.file_path
                && candidate.semantic_path == scope_path
                && csharp_is_type_declaration(candidate)
        })
        .collect::<Vec<_>>();
    if enclosing_candidates.len() != 1 {
        return Ok(None);
    }
    let mut ancestor_symbol = enclosing_candidates[0];
    let mut current_generic_arguments = Vec::new();
    let mut current_enclosing_generic_arguments = Vec::new();
    let mut visited_type_paths = BTreeSet::new();
    // Walk the base-type chain from the nearest ancestor outward; the first
    // ancestor that declares the member pins the receiver. The bound keeps
    // malformed cyclic inheritance from looping.
    for _ in 0..64 {
        let Some(base_binding) = csharp_base_type_binding_for_type(
            ancestor_symbol,
            raw_symbols,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let Some(base_type_path) = csharp_dispatchable_type_path(
            ancestor_symbol,
            raw_symbols,
            &base_binding,
            csharp_is_type_declaration,
        ) else {
            return Ok(None);
        };
        if !visited_type_paths.insert(base_type_path.clone()) {
            return Ok(None);
        }
        let base_indexes = semantic_path_index
            .get(&base_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if base_indexes.len() != 1 {
            return Ok(None);
        }
        let base_symbol = &raw_symbols[base_indexes[0]];
        // Compose the constructed base binding's concrete arguments by
        // substituting the current receiver's arguments into the base
        // spelling's raw type-argument spellings, so a base such as
        // `Base<T>` reached through `Caller : Base<Helper>` pins `T` to
        // `Helper` (and a member declared as `T` resolves its receiver
        // on `Helper`).
        let parameters = csharp_type_parameter_names_for_type(
            &ancestor_symbol.file_path,
            ancestor_symbol.byte_range,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        .unwrap_or_default();
        let generic_arguments = base_binding
            .raw_generic_argument_spellings
            .iter()
            .map(|spelling| {
                substitute_csharp_type_parameters(spelling, &parameters, &current_generic_arguments)
            })
            .collect::<Vec<_>>();
        let enclosing_generic_arguments = csharp_base_step_enclosing_arguments(
            &base_binding,
            &base_type_path,
            ancestor_symbol.semantic_path.as_str(),
            &current_enclosing_generic_arguments,
            &parameters,
            &current_generic_arguments,
            raw_symbols,
            semantic_path_index,
        );
        current_generic_arguments = generic_arguments;
        current_enclosing_generic_arguments = enclosing_generic_arguments;
        let Some(member_bindings) = csharp_member_type_bindings_for_type(
            &base_symbol.file_path,
            base_symbol.byte_range,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        if member_bindings.contains(member_name) {
            let Some(member_type_name) = member_bindings.type_for(member_name) else {
                return Ok(None);
            };
            // A member declared on a generic base substitutes the base's
            // type parameters with the composed concrete arguments before
            // resolving the receiver, so `T` on `Base<T>` reached through
            // `Caller : Base<Helper>` pins `Helper`; outer-parameter
            // members substitute the enclosing generic arguments.
            let mut member_type_name = member_type_name.to_string();
            if let Some(parameters) = csharp_type_parameter_names_for_type(
                &base_symbol.file_path,
                base_symbol.byte_range,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )? {
                member_type_name = substitute_csharp_type_parameters(
                    &member_type_name,
                    &parameters,
                    &current_generic_arguments,
                );
            }
            let member_type_name = substitute_csharp_enclosing_type_parameters(
                base_symbol,
                &current_enclosing_generic_arguments,
                &member_type_name,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
            let Some(member_binding) = resolve_csharp_receiver_type_binding(
                base_symbol,
                &member_type_name,
                raw_symbols,
                semantic_path_index,
                csharp_source_namespace_path(base_symbol, raw_symbols).flatten(),
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            if hops.is_empty() {
                return Ok(canonicalize_csharp_type_binding(
                    base_symbol,
                    &member_binding,
                    raw_symbols,
                ));
            }
            let Some((binding, scope_source_symbol)) = resolve_csharp_member_chain_binding(
                base_symbol,
                member_binding,
                hops,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            return Ok(canonicalize_csharp_type_binding(
                scope_source_symbol,
                &binding,
                raw_symbols,
            ));
        }
        ancestor_symbol = base_symbol;
    }
    Ok(None)
}

/// Resolves a member-hop declared type (`hop_type_name`) into a receiver
/// binding. Dotted and `global::` spellings, array suffixes, and simple names
/// that already resolve to a dispatchable type through the namespace/import
/// rules keep their existing behavior; a simple name that would otherwise
/// fail (such as a hop on `Outer<T>.Inner<U>` whose declared type is the
/// nested `Inner<U>` or a sibling nested type) resolves relative to the
/// declaring type's own scope chain, producing a fully qualified binding that
/// keeps the enclosing generic arguments so outer-parameter members such as
/// `T[] OuterItems` still substitute `T` with the outer concrete argument.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# member-hop type binding inputs explicit"
)]
fn resolve_csharp_member_hop_type_binding(
    scope_symbol: &IndexedSymbol,
    hop_type_name: &str,
    current_binding: &CSharpBaseTypeBinding,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    let mut resolve_with_existing_rules = || {
        resolve_csharp_receiver_type_binding(
            scope_symbol,
            hop_type_name,
            raw_symbols,
            semantic_path_index,
            csharp_source_namespace_path(scope_symbol, raw_symbols).flatten(),
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )
    };
    // Dotted and fully-qualified spellings already resolve nested paths
    // through the existing rules; keep them unchanged.
    if hop_type_name.contains('.') || hop_type_name.starts_with("global::") {
        return resolve_with_existing_rules();
    }
    // A simple name keeps the existing namespace/import resolution whenever
    // it already produces a dispatchable type path outside the declaring
    // scope chain; a resolution through that chain falls through to the
    // chain walk below so the returned binding carries the enclosing
    // concrete generic arguments aligned with the receiver.
    if let Some(binding) = resolve_with_existing_rules()?
        && let Some(resolved_type_path) = csharp_dispatchable_type_path(
            scope_symbol,
            raw_symbols,
            &binding,
            csharp_is_type_declaration,
        )
        && !csharp_dispatchable_path_is_nested_in_scope_chain(
            scope_symbol,
            &resolved_type_path,
            raw_symbols,
            semantic_path_index,
        )
    {
        return Ok(Some(binding));
    }
    let Some(simple_path) = crate::language::csharp_generic_type_semantic_path(hop_type_name)
    else {
        return Ok(None);
    };
    if simple_path.contains("::") {
        return Ok(None);
    }
    let Some(scope_type_path) = csharp_scope_type_path_for_symbol(scope_symbol) else {
        return Ok(None);
    };
    // Candidate nested paths: the declaring type itself when the simple name
    // matches its own last segment, a type nested directly inside it, and a
    // sibling nested within each enclosing type of the declaring chain. Only
    // prefixes that are themselves declared types are considered, so
    // namespace-level candidates keep resolving through the existing rules.
    let mut candidate_paths = Vec::new();
    let mut prefixes = Vec::new();
    let mut prefix = scope_type_path;
    loop {
        if semantic_path_index
            .get(prefix)
            .into_iter()
            .flatten()
            .any(|index| csharp_is_type_declaration(&raw_symbols[*index]))
        {
            prefixes.push(prefix);
        }
        match prefix.rsplit_once("::") {
            Some((parent, _)) => prefix = parent,
            None => break,
        }
    }
    // The simple name resolves to the nearest enclosing declaration: the
    // candidate list walks the declaring chain innermost-first and keeps the
    // first prefix whose nested type actually declares a type with the simple
    // name, so a same-named sibling nested in an outer enclosing type (such
    // as `Outer<T>.Inner<U>` when the declaring scope is
    // `Outer<T>.Middle<U>.Inner<V>`) is shadowed by the nearer declaration
    // instead of making the chain ambiguous.
    for (index, prefix) in prefixes.iter().enumerate() {
        let candidate = if index == 0 && prefix.rsplit("::").next() == Some(simple_path.as_str()) {
            scope_type_path.to_string()
        } else {
            format!("{prefix}::{simple_path}")
        };
        if !candidate_paths.contains(&candidate)
            && semantic_path_index
                .get(&candidate)
                .into_iter()
                .flatten()
                .filter(|index| csharp_is_type_declaration(&raw_symbols[**index]))
                .count()
                == 1
        {
            candidate_paths.push(candidate);
        }
    }
    let Some(candidate) = candidate_paths.into_iter().next() else {
        return Ok(None);
    };
    // The current receiver binding records one concrete-argument vector per
    // type segment of the scope type, outermost first; the candidate's
    // enclosing arguments are the leading entries up to the candidate's own
    // enclosing type segments, so self, deeper-nested, and sibling nested
    // candidates all keep the concrete outer arguments. When the binding did
    // not track every type segment (bare `this`/`base` chains), empty entries
    // keep non-generic members resolving while generic outer-parameter
    // members fail closed downstream.
    let mut scope_type_arguments = current_binding.enclosing_generic_arguments.clone();
    scope_type_arguments.push(current_binding.generic_arguments.clone());
    let type_segment_count = prefixes
        .iter()
        .filter(|prefix| {
            let prefix_path = **prefix;
            semantic_path_index
                .get(prefix_path)
                .into_iter()
                .flatten()
                .any(|index| csharp_is_type_declaration(&raw_symbols[*index]))
        })
        .count();
    let candidate_type_segment_count =
        candidate.split("::").count() - (scope_type_path.split("::").count() - type_segment_count);
    let enclosing_count = candidate_type_segment_count.saturating_sub(1);
    let enclosing_generic_arguments = if scope_type_arguments.len() == type_segment_count {
        scope_type_arguments[..enclosing_count.min(scope_type_arguments.len())].to_vec()
    } else if let Some(composed_enclosing) = csharp_compose_enclosing_generic_arguments_to_type(
        &current_binding.semantic_type_path,
        &current_binding.generic_arguments,
        &current_binding.enclosing_generic_arguments,
        scope_type_path,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? {
        // A receiver that does not spell per-type-segment concrete arguments
        // itself, such as `new Derived()` where `Derived` inherits from
        // `Outer<HelperA>.Inner<HelperB>` or a `this`/`base`-rooted chain,
        // composes the hop scope type's enclosing and own concrete arguments
        // through the unique class/record ancestor chain so outer-parameter
        // members still substitute; unresolvable or ambiguous chains keep
        // empty entries and fail closed downstream.
        let mut composed_arguments = composed_enclosing;
        if let Some(own_arguments) = csharp_compose_generic_arguments_to_type(
            &current_binding.semantic_type_path,
            &current_binding.generic_arguments,
            scope_type_path,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            composed_arguments.push(own_arguments);
        }
        composed_arguments[..enclosing_count.min(composed_arguments.len())].to_vec()
    } else {
        vec![Vec::new(); enclosing_count]
    };
    Ok(Some(CSharpBaseTypeBinding {
        semantic_type_path: candidate.clone(),
        is_global_qualified: true,
        alias_name: None,
        namespace_import_paths: Vec::new(),
        generic_arguments: crate::language::csharp_generic_type_arguments(hop_type_name)
            .unwrap_or_default(),
        raw_generic_argument_spellings: Vec::new(),
        enclosing_generic_arguments,
        raw_enclosing_generic_argument_spellings: Vec::new(),
    }))
}

/// Returns the enclosing type path of a symbol that declares a member hop:
/// the type's own semantic path for a type declaration, or the enclosing
/// type path of a member such as a method. Symbols without an enclosing type
/// (top-level functions) return `None`.
fn csharp_scope_type_path_for_symbol(symbol: &IndexedSymbol) -> Option<&str> {
    if csharp_is_type_declaration(symbol) {
        return Some(symbol.semantic_path.as_str());
    }
    symbol
        .semantic_path
        .rsplit_once("::")
        .map(|(parent_path, _)| parent_path)
}

/// Returns whether `resolved_type_path` names a type nested inside an
/// enclosing type of the declaring scope chain of `scope_symbol` (the same
/// chain the member-hop type binding fallback walks for simple names).
/// Simple names that resolve to such a nested type fall through to the chain
/// walk so the returned binding carries the enclosing concrete generic
/// arguments aligned with the receiver; a plain namespace/import/global
/// resolution (including a simple name matching the scope type itself) keeps
/// the fast path.
fn csharp_dispatchable_path_is_nested_in_scope_chain(
    scope_symbol: &IndexedSymbol,
    resolved_type_path: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> bool {
    let Some(scope_type_path) = csharp_scope_type_path_for_symbol(scope_symbol) else {
        return false;
    };
    let mut prefix = scope_type_path;
    loop {
        if semantic_path_index
            .get(prefix)
            .into_iter()
            .flatten()
            .any(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            && resolved_type_path.starts_with(&format!("{prefix}::"))
        {
            return true;
        }
        match prefix.rsplit_once("::") {
            Some((parent_path, _)) => prefix = parent_path,
            None => break,
        }
    }
    false
}
/// Walks the intermediate hops of an instance receiver chain such as
/// `group.member.helper(...)`, `this.member.helper(...)`, or
/// `base.member.helper(...)` (everything after the leading receiver except the
/// final method). Each field, property, or event hop is looked up on the
/// current type and, when the current type does not declare it, through the
/// unique class/record ancestor chain so the nearest declaring ancestor pins
/// the hop; method-call hops dispatch as instance calls and continue on their
/// declared return type. The returned source symbol is the type that declared
/// the final hop so the final member dispatches in that scope. Unknown,
/// ambiguous, or unresolvable hops fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# member-chain hop resolution inputs explicit"
)]
fn resolve_csharp_member_chain_binding<'a>(
    source_symbol: &'a IndexedSymbol,
    mut binding: CSharpBaseTypeBinding,
    hops: &[&str],
    raw_symbols: &'a [IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(CSharpBaseTypeBinding, &'a IndexedSymbol)>> {
    let mut scope_source_symbol = source_symbol;
    for hop in hops {
        let Some(type_path) = csharp_dispatchable_type_path(
            scope_source_symbol,
            raw_symbols,
            &binding,
            csharp_is_type_declaration,
        ) else {
            return Ok(None);
        };
        let type_indexes = semantic_path_index
            .get(&type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if type_indexes.len() != 1 {
            return Ok(None);
        }
        let type_symbol = &raw_symbols[type_indexes[0]];
        // A method-call hop such as `inner()` or `inner(1)` dispatches the
        // hop method as an instance call and continues the chain on its
        // declared return type; field, property, and event hops resolve the
        // member's declared type directly. An element-access hop such as
        // `items[0]` or `fieldItems[0][0]` strips the brackets and requires
        // the named member to be an array with enough layers whose element
        // component type pins the next hop.
        let array_member_name = csharp_array_access_member_name(hop);
        let member_name = array_member_name.unwrap_or(hop);
        if let Some((method_name, hop_arity)) = csharp_method_call_hop_spelling(hop) {
            let Some(method_type_arguments) = csharp_method_type_arguments(hop) else {
                return Ok(None);
            };
            let Some((next_binding, method_symbol)) = resolve_csharp_method_call_hop_binding(
                type_symbol,
                &binding,
                &method_name,
                hop_arity,
                &method_type_arguments,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            binding = next_binding;
            scope_source_symbol = method_symbol;
            continue;
        }
        // A method-call hop with an element-access suffix such as
        // `makeItems()[0]` or `GetMatrix()[0][0]` dispatches the hop method
        // as an instance call and strips one return-array component layer
        // per element access before continuing the chain; non-array or
        // primitive-array returns and element access deeper than the return
        // array fail closed.
        if let Some(array_member_name) = array_member_name
            && let Some((method_name, hop_arity)) =
                csharp_method_call_hop_spelling(array_member_name)
            && let Some(depth) = csharp_array_access_depth(hop)
        {
            let Some(method_type_arguments) = csharp_method_type_arguments(array_member_name)
            else {
                return Ok(None);
            };
            let Some(symbol_id) = resolve_csharp_instance_method_on_binding(
                type_symbol,
                &binding,
                &method_name,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                hop_arity,
                deadline,
            )?
            else {
                return Ok(None);
            };
            let Some(method_symbol) = raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == symbol_id)
            else {
                return Ok(None);
            };
            let Some(return_type) = method_symbol.return_type.as_deref() else {
                return Ok(None);
            };
            if return_type.is_empty() {
                return Ok(None);
            }
            let return_type = substitute_csharp_method_return_type(
                method_symbol,
                &binding,
                &type_symbol.semantic_path,
                return_type,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
            let return_type = substitute_csharp_method_type_parameters(
                method_symbol,
                &method_type_arguments,
                &return_type,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
            let Some(component_name) =
                csharp_array_component_spelling_at_depth(&return_type, depth)
            else {
                return Ok(None);
            };
            let Some(next_binding) = resolve_csharp_receiver_type_binding(
                method_symbol,
                &component_name,
                raw_symbols,
                semantic_path_index,
                csharp_source_namespace_path(method_symbol, raw_symbols).flatten(),
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            binding = next_binding;
            scope_source_symbol = method_symbol;
            continue;
        }
        let (declaring_type_symbol, hop_type_name) = {
            // A field/property/event hop is looked up on the current type and,
            // when the current type does not declare it, through the unique
            // class/record ancestor chain (or the interface-extends chain for
            // interface-typed receivers) so the nearest declaration (or its
            // absence) is authoritative.
            let mut current_type_symbol = type_symbol;
            let mut visited_type_paths = BTreeSet::new();
            loop {
                let Some(member_bindings) = csharp_member_type_bindings_for_type(
                    &current_type_symbol.file_path,
                    current_type_symbol.byte_range,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                else {
                    return Ok(None);
                };
                if member_bindings.contains(member_name) {
                    // A member chain always walks an instance receiver; a
                    // static member reached through an instance reference is
                    // invalid C# (CS0176) and fails closed, so only instance
                    // members continue the chain.
                    if member_bindings.is_static_member(member_name) {
                        return Ok(None);
                    }
                    // An element-access hop requires an array-typed member;
                    // the element component type pins the next hop, stripping
                    // one component layer per element access in the hop, while
                    // non-array and primitive-array members fail closed.
                    let hop_type_name = if let Some(array_member_name) = array_member_name {
                        let Some(declared_type) = member_bindings.type_for(array_member_name)
                        else {
                            return Ok(None);
                        };
                        let Some(depth) = csharp_array_access_depth(hop) else {
                            return Ok(None);
                        };
                        csharp_array_component_spelling_at_depth(&declared_type, depth)
                    } else {
                        member_bindings.type_for(member_name)
                    };
                    let Some(mut hop_type_name) = hop_type_name else {
                        return Ok(None);
                    };
                    // A generic declaring type substitutes its type
                    // parameters with the concrete arguments composed for it
                    // through the unique class/record ancestor chain, so
                    // `items` declared as `T[]` on `Box<T>` resolves to
                    // `Helper[]` both directly on a `Box<Helper>` receiver and
                    // through a `Derived<Helper> : Box<T>` base. Interface-
                    // extends members resolve without generic argument
                    // composition and fail closed across generic interface
                    // inheritance, as do unresolvable chains, arity
                    // mismatches, and receiver bindings without concrete
                    // generic arguments (such as constructed `new Box<int>()`
                    // element-access receivers, whose spellings strip the type
                    // arguments); those leave non-generic members unchanged
                    // and defer generic-member substitution to downstream
                    // fail-closed checks.
                    let parameters = csharp_type_parameter_names_for_type(
                        &current_type_symbol.file_path,
                        current_type_symbol.byte_range,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                    )?
                    .unwrap_or_default();
                    if !parameters.is_empty() {
                        let Some(declaring_type_args) = csharp_compose_generic_arguments_to_type(
                            &type_symbol.semantic_path,
                            &binding.generic_arguments,
                            &current_type_symbol.semantic_path,
                            raw_symbols,
                            semantic_path_index,
                            csharp_global_import_context,
                            file_overrides,
                            csharp_import_contexts_by_file,
                            deadline,
                        )?
                        else {
                            return Ok(None);
                        };
                        if declaring_type_args.is_empty() {
                            // No concrete arguments were composed for the
                            // declaring type; keep the hop type as declared so
                            // non-generic members still resolve and generic
                            // members fail closed downstream.
                        } else if parameters.len() != declaring_type_args.len() {
                            return Ok(None);
                        } else {
                            hop_type_name = substitute_csharp_type_parameters(
                                &hop_type_name,
                                &parameters,
                                &declaring_type_args,
                            );
                        }
                    }
                    // A hop may also reference an outer type parameter such as
                    // `T` in `Outer<T>.Inner<U>`; compose the enclosing
                    // segments' concrete arguments from the receiver (directly
                    // or through its unique class/record ancestor chain) and
                    // substitute those parameters, leaving unresolvable
                    // parameters unchanged so they fail closed downstream.
                    if let Some(enclosing_generic_arguments) =
                        csharp_compose_enclosing_generic_arguments_to_type(
                            &type_symbol.semantic_path,
                            &binding.generic_arguments,
                            &binding.enclosing_generic_arguments,
                            &current_type_symbol.semantic_path,
                            raw_symbols,
                            semantic_path_index,
                            csharp_global_import_context,
                            file_overrides,
                            csharp_import_contexts_by_file,
                            deadline,
                        )?
                    {
                        hop_type_name = substitute_csharp_enclosing_type_parameters(
                            current_type_symbol,
                            &enclosing_generic_arguments,
                            &hop_type_name,
                            raw_symbols,
                            semantic_path_index,
                            file_overrides,
                            csharp_import_contexts_by_file,
                            deadline,
                        )?;
                    }
                    break (current_type_symbol, hop_type_name);
                }
                if current_type_symbol.node_kind == "interface_declaration" {
                    // Interfaces have no class/record base to walk; resolve
                    // the hop (including element-access hops) through the
                    // interface-extends chain instead, with the same
                    // shadowing and ambiguity rules as interface method
                    // dispatch.
                    let mut visited_interface_paths = BTreeSet::new();
                    match resolve_csharp_interface_member_hop(
                        current_type_symbol,
                        hop,
                        &binding.generic_arguments,
                        raw_symbols,
                        semantic_path_index,
                        csharp_global_import_context,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                        &mut visited_interface_paths,
                    )? {
                        CSharpInterfaceMemberHopResolution::Resolved(
                            declaring_type_symbol,
                            hop_type_name,
                        ) => break (declaring_type_symbol, hop_type_name),
                        CSharpInterfaceMemberHopResolution::NoHop
                        | CSharpInterfaceMemberHopResolution::Blocked => {
                            return Ok(None);
                        }
                    }
                }
                let Some(base_binding) = csharp_base_type_binding_for_type(
                    current_type_symbol,
                    raw_symbols,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?
                else {
                    return Ok(None);
                };
                let Some(base_type_path) =
                    csharp_base_type_path(current_type_symbol, raw_symbols, &base_binding)
                else {
                    return Ok(None);
                };
                if !visited_type_paths.insert(base_type_path.clone()) {
                    return Ok(None);
                }
                let base_indexes = semantic_path_index
                    .get(&base_type_path)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|index| csharp_is_base_constructible_type(&raw_symbols[*index]))
                    .collect::<Vec<_>>();
                if base_indexes.len() != 1 {
                    return Ok(None);
                }
                current_type_symbol = &raw_symbols[base_indexes[0]];
            }
        };
        let Some(next_binding) = resolve_csharp_member_hop_type_binding(
            declaring_type_symbol,
            &hop_type_name,
            &binding,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        binding = next_binding;
        scope_source_symbol = declaring_type_symbol;
    }
    Ok(Some((binding, scope_source_symbol)))
}

/// Returns the ordered type-parameter names of every enclosing type
/// declaration of `declaring_type_symbol`, outermost first, so a nested
/// generic type such as `Outer<T>.Inner<U>` reports `[["T"]]` for the outer
/// `Outer<T>`. Namespace segments contribute nothing, and an enclosing chain
/// that cannot be walked uniquely returns `None` so callers fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# enclosing type parameter walk inputs explicit"
)]
fn csharp_enclosing_type_parameter_names(
    declaring_type_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<Vec<Vec<String>>>> {
    let mut segments: Vec<&str> = declaring_type_symbol.semantic_path.split("::").collect();
    segments.pop();
    let mut enclosing_parameters = Vec::new();
    let mut prefix = String::new();
    for segment in segments {
        if !prefix.is_empty() {
            prefix.push_str("::");
        }
        prefix.push_str(segment);
        let type_indexes = semantic_path_index
            .get(&prefix)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .collect::<Vec<_>>();
        if type_indexes.is_empty() {
            // A namespace segment contributes no type parameters.
            continue;
        }
        if type_indexes.len() != 1 {
            return Ok(None);
        }
        let type_symbol = &raw_symbols[type_indexes[0]];
        let Some(parameters) = csharp_type_parameter_names_for_type(
            &type_symbol.file_path,
            type_symbol.byte_range,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        enclosing_parameters.push(parameters);
    }
    Ok(Some(enclosing_parameters))
}

/// Substitutes the enclosing generic segments' type parameters (outer type
/// parameters such as `T` in `Outer<T>.Inner<U>`) in `spelling` with the
/// composed per-segment concrete argument spellings. The declaring type's
/// enclosing chain must walk uniquely and each segment's parameter/argument
/// arity must match; otherwise the spelling is left unchanged so
/// unresolvable type parameters fail closed downstream.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# outer type parameter substitution inputs explicit"
)]
fn substitute_csharp_enclosing_type_parameters(
    declaring_type_symbol: &IndexedSymbol,
    enclosing_generic_arguments: &[Vec<String>],
    spelling: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<String> {
    if enclosing_generic_arguments.is_empty() {
        return Ok(spelling.to_string());
    }
    let Some(enclosing_parameters) = csharp_enclosing_type_parameter_names(
        declaring_type_symbol,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(spelling.to_string());
    };
    // Namespace and non-generic segments carry no type parameters or
    // concrete arguments; only the generic enclosing segments participate,
    // aligned by position (e.g. `Demo.Outer<int>` contributes only
    // `["int"]` for `Outer<T>`).
    let enclosing_parameters = enclosing_parameters
        .into_iter()
        .filter(|parameters| !parameters.is_empty())
        .collect::<Vec<_>>();
    let enclosing_arguments = enclosing_generic_arguments
        .iter()
        .filter(|arguments| !arguments.is_empty())
        .collect::<Vec<_>>();
    if enclosing_parameters.len() != enclosing_arguments.len() {
        return Ok(spelling.to_string());
    }
    let mut result = spelling.to_string();
    for (parameters, arguments) in enclosing_parameters.iter().zip(enclosing_arguments.iter()) {
        if parameters.len() != arguments.len() {
            return Ok(spelling.to_string());
        }
        result = substitute_csharp_type_parameters(&result, parameters, arguments);
    }
    Ok(result)
}

/// Composes the concrete enclosing generic arguments for `target_type_path`
/// when it is reached from `source_type_path` through the unique class/record
/// ancestor chain or the interface-extends chain, substituting each walked
/// type's declared type parameters with its current concrete arguments into
/// the next base-list spelling's per-segment type-argument spellings. The
/// source's own arguments seed the walk, so a `Derived<HelperA>` receiver
/// reaching base `Outer<HelperA>.Inner<HelperB>` yields `[["HelperA"]]` for
/// the nested type's enclosing `Outer` segment, a non-generic
/// `Fixed : Outer<HelperA>.Inner<HelperB>` receiver yields `[["HelperA"]]`
/// from the base spelling, and multi-level chains compose per level. When an
/// interface reaches the target through several parent branches, every
/// branch's composed enclosing arguments must agree or the mapping is
/// ambiguous. `None` means the target is not reachable through a unique
/// class/record or interface-extends walk, a base chain cannot be resolved
/// or walked uniquely, or a parameter/argument arity mismatch blocks the
/// mapping, so callers fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# enclosing generic argument composition inputs explicit"
)]
fn csharp_compose_enclosing_generic_arguments_to_type(
    source_type_path: &str,
    source_type_args: &[String],
    source_enclosing_args: &[Vec<String>],
    target_type_path: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<Vec<Vec<String>>>> {
    let mut visited_type_paths = BTreeSet::new();
    let mut composed_arguments = Vec::new();
    csharp_collect_enclosing_generic_argument_compositions(
        source_type_path,
        source_type_args,
        source_enclosing_args,
        target_type_path,
        &mut visited_type_paths,
        &mut composed_arguments,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    let Some(first) = composed_arguments.first() else {
        return Ok(None);
    };
    if composed_arguments
        .iter()
        .any(|arguments| arguments != first)
    {
        return Ok(None);
    }
    Ok(Some(first.clone()))
}

/// Composes the constructed base step's enclosing generic argument vector
/// from a base-list binding reached while walking a class/record ancestor
/// chain. A base spelled with explicit enclosing segments (such as
/// `Outer<Helper>.Inner<HelperB>`) substitutes each segment's raw
/// type-argument spellings with the current type's concrete arguments; a
/// base spelled with a simple name can name a sibling nested type inside one
/// of the current type's enclosing generic types (such as `Base<U>` nested
/// beside `Mid<U>` inside `Outer<T>`), and then the base's enclosing
/// arguments inherit the leading entries of the current type's enclosing
/// arguments aligned with the innermost enclosing type that contains the
/// base's target path, so `Caller : Outer<Helper>.Mid<Helper>` reaching
/// `Base<U>` carries `Outer<Helper>` into the base step. A base outside the
/// current type's enclosing chain keeps no enclosing arguments.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# constructed base step enclosing argument inputs explicit"
)]
fn csharp_base_step_enclosing_arguments(
    base_binding: &CSharpBaseTypeBinding,
    base_type_path: &str,
    current_type_path: &str,
    current_enclosing_args: &[Vec<String>],
    parameters: &[String],
    current_type_args: &[String],
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Vec<Vec<String>> {
    if !base_binding
        .raw_enclosing_generic_argument_spellings
        .is_empty()
    {
        return base_binding
            .raw_enclosing_generic_argument_spellings
            .iter()
            .map(|segment| {
                segment
                    .iter()
                    .map(|spelling| {
                        substitute_csharp_type_parameters(spelling, parameters, current_type_args)
                    })
                    .collect()
            })
            .collect();
    }
    let mut enclosing_type_paths = Vec::new();
    let mut prefix = current_type_path;
    while let Some((parent_path, _)) = prefix.rsplit_once("::") {
        prefix = parent_path;
        if semantic_path_index
            .get(prefix)
            .into_iter()
            .flatten()
            .any(|index| csharp_is_type_declaration(&raw_symbols[*index]))
        {
            enclosing_type_paths.push(prefix);
        }
    }
    enclosing_type_paths.reverse();
    let mut containing_index = None;
    for (index, enclosing_type_path) in enclosing_type_paths.iter().enumerate() {
        if base_type_path.starts_with(&format!("{enclosing_type_path}::")) {
            containing_index = Some(index);
        }
    }
    match containing_index {
        Some(index) => current_enclosing_args
            .iter()
            .take(index + 1)
            .cloned()
            .collect(),
        None => Vec::new(),
    }
}

/// Collects the composed enclosing generic argument vectors for every path
/// that reaches `target_type_path` from `current_type_path`, walking
/// class/record bases and interface parents recursively. Each step carries
/// the current type's own last-segment arguments and its composed enclosing
/// arguments; a base step substitutes the base spelling's raw last-segment
/// and enclosing segment spellings with the current type's parameters and
/// concrete arguments. A path that terminates without reaching the target, a
/// cycle, or an unresolvable step contributes nothing; the caller requires
/// every collected composition to agree.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# enclosing generic argument composition inputs explicit"
)]
fn csharp_collect_enclosing_generic_argument_compositions(
    current_type_path: &str,
    current_type_args: &[String],
    current_enclosing_args: &[Vec<String>],
    target_type_path: &str,
    visited_type_paths: &mut BTreeSet<String>,
    composed_arguments: &mut Vec<Vec<Vec<String>>>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<()> {
    if current_type_path == target_type_path {
        composed_arguments.push(current_enclosing_args.to_vec());
        return Ok(());
    }
    if !visited_type_paths.insert(current_type_path.to_string()) {
        return Ok(());
    }
    let Some(type_indexes) = semantic_path_index.get(current_type_path) else {
        visited_type_paths.remove(current_type_path);
        return Ok(());
    };
    let type_indexes = type_indexes
        .iter()
        .copied()
        .filter(|index| {
            csharp_is_base_constructible_type(&raw_symbols[*index])
                || raw_symbols[*index].node_kind == "interface_declaration"
        })
        .collect::<Vec<_>>();
    if type_indexes.len() != 1 {
        visited_type_paths.remove(current_type_path);
        return Ok(());
    }
    let current_type_symbol = &raw_symbols[type_indexes[0]];
    let parameters = csharp_type_parameter_names_for_type(
        &current_type_symbol.file_path,
        current_type_symbol.byte_range,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    .unwrap_or_default();
    if parameters.len() != current_type_args.len() {
        visited_type_paths.remove(current_type_path);
        return Ok(());
    }
    if current_type_symbol.node_kind == "interface_declaration" {
        let source_namespace_path =
            csharp_source_namespace_path(current_type_symbol, raw_symbols).flatten();
        let parent_bindings = match csharp_interface_parent_bindings_for_interface(
            &current_type_symbol.file_path,
            current_type_symbol.byte_range,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            CSharpInterfaceParents::None => {
                visited_type_paths.remove(current_type_path);
                return Ok(());
            }
            CSharpInterfaceParents::Blocked => {
                visited_type_paths.remove(current_type_path);
                return Ok(());
            }
            CSharpInterfaceParents::Parents(parent_bindings) => parent_bindings,
        };
        for parent_binding in parent_bindings {
            let Some(parent_interface_path) = csharp_interface_type_path(
                current_type_symbol,
                raw_symbols,
                semantic_path_index,
                &parent_binding,
            ) else {
                visited_type_paths.remove(current_type_path);
                return Ok(());
            };
            let parent_args: Vec<String> = parent_binding
                .raw_generic_argument_spellings
                .iter()
                .map(|spelling| {
                    substitute_csharp_type_parameters(spelling, &parameters, current_type_args)
                })
                .collect();
            csharp_collect_enclosing_generic_argument_compositions(
                &parent_interface_path,
                &parent_args,
                &[],
                target_type_path,
                visited_type_paths,
                composed_arguments,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
        }
    } else {
        let Some(base_binding) = csharp_base_type_binding_for_type(
            current_type_symbol,
            raw_symbols,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            visited_type_paths.remove(current_type_path);
            return Ok(());
        };
        let Some(base_type_path) =
            csharp_base_type_path(current_type_symbol, raw_symbols, &base_binding)
        else {
            visited_type_paths.remove(current_type_path);
            return Ok(());
        };
        let base_args: Vec<String> = base_binding
            .raw_generic_argument_spellings
            .iter()
            .map(|spelling| {
                substitute_csharp_type_parameters(spelling, &parameters, current_type_args)
            })
            .collect();
        let base_enclosing_args = csharp_base_step_enclosing_arguments(
            &base_binding,
            &base_type_path,
            current_type_path,
            current_enclosing_args,
            &parameters,
            current_type_args,
            raw_symbols,
            semantic_path_index,
        );
        csharp_collect_enclosing_generic_argument_compositions(
            &base_type_path,
            &base_args,
            &base_enclosing_args,
            target_type_path,
            visited_type_paths,
            composed_arguments,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?;
    }
    visited_type_paths.remove(current_type_path);
    Ok(())
}

/// Substitutes a method's declared return type with the concrete generic
/// arguments composed for its declaring type, so `GetBox()` declared as
/// returning `Box<T>` on `Box<T>` resolves to `Box<Helper>` both when the
/// receiver is `Box<Helper>` directly and when it reaches the method through
/// a generic class/record base such as `Derived<Helper> : Box<T>`. The
/// caller threads the receiver's already-resolved dispatchable type path
/// (resolved in the caller's scope), so a receiver whose simple name does not
/// resolve from the method's own namespace, such as a cross-namespace
/// `IGeneric<Helper>` interface reachable only through the caller's `using`,
/// still composes through its extends chain. A non-generic declaring type, a
/// missing or ambiguous declaring type, a base or interface chain that cannot
/// be walked uniquely, or a parameter/argument arity mismatch leaves the
/// return type unchanged and fails closed downstream.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# generic method return type substitution inputs explicit"
)]
fn substitute_csharp_method_return_type(
    method: &IndexedSymbol,
    binding: &CSharpBaseTypeBinding,
    binding_type_path: &str,
    return_type: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<String> {
    let Some(scope_path) = method.scope_path.as_deref() else {
        return Ok(return_type.to_string());
    };
    if binding_type_path.is_empty() {
        return Ok(return_type.to_string());
    }
    let type_indexes = semantic_path_index
        .get(scope_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
        .collect::<Vec<_>>();
    if type_indexes.len() != 1 {
        return Ok(return_type.to_string());
    }
    let type_symbol = &raw_symbols[type_indexes[0]];
    let parameters = csharp_type_parameter_names_for_type(
        &type_symbol.file_path,
        type_symbol.byte_range,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    .unwrap_or_default();
    let mut substituted = return_type.to_string();
    if !parameters.is_empty()
        && let Some(declaring_type_args) = csharp_compose_generic_arguments_to_type(
            binding_type_path,
            &binding.generic_arguments,
            scope_path,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        && parameters.len() == declaring_type_args.len()
    {
        substituted =
            substitute_csharp_type_parameters(&substituted, &parameters, &declaring_type_args);
    }
    // A method may also return an outer type parameter such as `T` in
    // `Outer<T>.Inner<U>`; compose the enclosing segments' concrete
    // arguments from the receiver (directly or through its unique
    // class/record ancestor chain) and substitute those parameters, leaving
    // unresolvable parameters unchanged so they fail closed downstream.
    if let Some(enclosing_generic_arguments) = csharp_compose_enclosing_generic_arguments_to_type(
        binding_type_path,
        &binding.generic_arguments,
        &binding.enclosing_generic_arguments,
        scope_path,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? {
        substitute_csharp_enclosing_type_parameters(
            type_symbol,
            &enclosing_generic_arguments,
            &substituted,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )
    } else {
        Ok(substituted)
    }
}

/// Substitutes a bare factory method's declared return type when the factory
/// was dispatched through the caller's unique class/record ancestor chain, so
/// `var helper = Make()` or `Make().Run(1)` on `Caller : Derived<Helper>`
/// with `protected static U Make()` on `Base<U>` resolves the declared `U`
/// return to `Helper`, and `T MakeOuter()` declared in a type nested in
/// `Outer<T>` resolves to the composed outer argument. The caller-side
/// binding seeds the composition walk with the caller's own type path and
/// empty concrete arguments, so the walk reaches the factory's declaring type
/// through the base chain and substitutes both the last-segment and the
/// enclosing generic arguments. A same-type factory, a factory outside the
/// caller's base chain (such as a static-imported method), an unresolvable
/// base chain, or a parameter/argument arity mismatch keeps the declared
/// return type unchanged and fails closed downstream when it names a type
/// parameter.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# bare inherited factory return type substitution inputs explicit"
)]
fn csharp_substitute_bare_inherited_factory_return_type(
    source_symbol: &IndexedSymbol,
    method: &IndexedSymbol,
    return_type: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<String> {
    let Some(method_scope_path) = method.scope_path.as_deref() else {
        return Ok(return_type.to_string());
    };
    let Some(source_type) = csharp_source_type_declaration(source_symbol, raw_symbols) else {
        return Ok(return_type.to_string());
    };
    if source_type.semantic_path == method_scope_path {
        return Ok(return_type.to_string());
    }
    let binding = CSharpBaseTypeBinding {
        semantic_type_path: source_type.semantic_path.clone(),
        is_global_qualified: true,
        alias_name: None,
        namespace_import_paths: Vec::new(),
        generic_arguments: Vec::new(),
        raw_generic_argument_spellings: Vec::new(),
        enclosing_generic_arguments: Vec::new(),
        raw_enclosing_generic_argument_spellings: Vec::new(),
    };
    let Some(binding_type_path) = csharp_dispatchable_type_path(
        source_symbol,
        raw_symbols,
        &binding,
        csharp_is_type_declaration,
    ) else {
        return Ok(return_type.to_string());
    };
    substitute_csharp_method_return_type(
        method,
        &binding,
        &binding_type_path,
        return_type,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )
}

/// Substitutes a type-qualified factory method's declared return type when
/// the method is inherited through the qualified type's unique class/record
/// ancestor chain, so `Caller.Make()` on `Caller : Derived<Helper>` with
/// `protected static U Make()` on `Base<U>` resolves the declared `U` return
/// to `Helper`. The qualified type's binding seeds the composition walk with
/// its own type path and, when the receiver was resolved through a type
/// alias or constructed generic spelling, its concrete generic arguments; a
/// method declared directly on the qualified type or a type outside its base
/// chain keeps the declared return type unchanged and fails closed
/// downstream when it names a type parameter.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# qualified factory return type substitution inputs explicit"
)]
fn csharp_substitute_qualified_factory_return_type(
    method: &IndexedSymbol,
    type_path: &str,
    receiver_binding: Option<&CSharpBaseTypeBinding>,
    return_type: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<String> {
    // The qualified type's binding seeds the composition walk with its own
    // type path and, when the receiver was resolved through a type alias or
    // a constructed generic spelling, its concrete generic arguments, so
    // `Alias.Make()` with `using Alias = Demo.Derived<HelperA>;` and
    // `Base<U>::Make` resolves the declared `U` return to `HelperA`.
    // Receivers resolved without a binding seed empty concrete arguments and
    // keep the declared return type unchanged for methods that do not
    // reference type parameters.
    let binding = receiver_binding
        .cloned()
        .unwrap_or_else(|| CSharpBaseTypeBinding {
            semantic_type_path: type_path.to_string(),
            is_global_qualified: true,
            alias_name: None,
            namespace_import_paths: Vec::new(),
            generic_arguments: Vec::new(),
            raw_generic_argument_spellings: Vec::new(),
            enclosing_generic_arguments: Vec::new(),
            raw_enclosing_generic_argument_spellings: Vec::new(),
        });
    substitute_csharp_method_return_type(
        method,
        &binding,
        type_path,
        return_type,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )
}

/// Substitutes a generic method's own type parameters with the explicit
/// type-argument spellings from its call site, so `T Make<T>()` called as
/// `Make<HelperA>()` resolves its declared return type `T` to `HelperA`.
/// The method's type-parameter names come from its declaration (such as `T`
/// in `T Make<T>()`), and a parameter/argument arity mismatch or a method
/// without its own type parameters leaves the return type unchanged and
/// fails closed downstream.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# generic method return type substitution inputs explicit"
)]
fn substitute_csharp_method_type_parameters(
    method: &IndexedSymbol,
    method_type_arguments: &[String],
    return_type: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<String> {
    if method_type_arguments.is_empty() {
        return Ok(return_type.to_string());
    }
    let Some(parameters) = csharp_type_parameter_names_for_type(
        &method.file_path,
        method.byte_range,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(return_type.to_string());
    };
    Ok(substitute_csharp_type_parameters(
        return_type,
        &parameters,
        method_type_arguments,
    ))
}

/// Splits a factory-chain spelling such as `makeGroup().GetItems`,
/// `makeGroup(2).GetItems`, or `Util.MakeGroup().GetItems` into the leading
/// call segment (scanned up to its balanced argument list) and the remaining
/// member chain. Spellings without a call root, without a trailing member
/// chain, or with an unbalanced argument list return `None` and fail closed.
/// Returns the inner expression of a spelling fully wrapped in one balanced
/// outer parenthesis group, such as `(makeGroup())` -> `makeGroup()` or
/// `(this.makeGroup())` -> `this.makeGroup()`. Spellings that are not fully
/// wrapped, that wrap a non-call expression, or with unbalanced parentheses
/// return `None` and leave the spelling unchanged.
fn csharp_outer_parenthesized_inner(spelling: &str) -> Option<&str> {
    if !spelling.starts_with('(') || !spelling.ends_with(')') {
        return None;
    }
    let mut depth = 0usize;
    for (index, byte) in spelling.bytes().enumerate() {
        match byte as char {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    if index != spelling.len() - 1 {
                        return None;
                    }
                    return Some(&spelling[1..index]);
                }
            }
            _ => {}
        }
    }
    None
}

fn csharp_factory_chain_leading_call(factory_name: &str) -> Option<(&str, &str)> {
    let open = factory_name.find('(')?;
    let mut depth = 0usize;
    for (index, byte) in factory_name.bytes().enumerate().skip(open) {
        match byte as char {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    let leading_call = &factory_name[..=index];
                    // The remainder may be a dotted member/hop chain
                    // (`GetItems` in `makeGroup().GetItems`) or an
                    // element-access suffix (`[0]` in
                    // `Factory.MakeNestedArray()[0].GetOuterItem`).
                    let mut remainder = &factory_name[index + 1..];
                    if let Some(stripped) = remainder.strip_prefix('.') {
                        remainder = stripped;
                    }
                    if leading_call.is_empty() || remainder.is_empty() {
                        return None;
                    }
                    return Some((leading_call, remainder));
                }
            }
            _ => {}
        }
    }
    None
}

/// Parses a method-call hop spelling such as `inner()`, `inner(0)`, or
/// `inner(2)` into the method name and the call arity recorded by the
/// extractor. Field, property, and event hops, malformed spellings, and
/// non-numeric argument lists return `None` so they fall through to member
/// resolution and fail closed when no such member exists.
/// Strips a trailing well-formed generic type-argument list from a method
/// call spelling such as `Make<HelperA>` or `Make<HelperA, HelperB>` (the
/// name portion before the call's argument list), leaving the bare method
/// name `Make` for dispatch. Non-generic spellings return the spelling
/// unchanged; malformed or unbalanced argument lists return `None` and fail
/// closed.
fn strip_csharp_method_type_arguments(name: &str) -> Option<&str> {
    let Some(open) = name.find('<') else {
        return Some(name);
    };
    let mut generic_depth = 0usize;
    for (offset, character) in name[open..].char_indices() {
        match character {
            '<' => generic_depth += 1,
            '>' => {
                generic_depth = generic_depth.checked_sub(1)?;
                if generic_depth == 0 {
                    let after = &name[open + offset + 1..];
                    return after.is_empty().then_some(&name[..open]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extracts the explicit type-argument spellings of a generic method-call
/// spelling such as `Make<HelperA>()` or `Make<HelperA, HelperB>()` into
/// `["HelperA"]` or `["HelperA", "HelperB"]`, so call-site type arguments can
/// substitute the method's own type parameters in its declared return type.
/// Non-generic spellings return an empty list; malformed or unbalanced
/// argument lists return `None` and fail closed.
fn csharp_method_type_arguments(spelling: &str) -> Option<Vec<String>> {
    let name = spelling.split('(').next().unwrap_or(spelling);
    if !name.contains('<') {
        return Some(Vec::new());
    }
    crate::language::csharp_generic_type_arguments(name)
}

/// Returns the trailing dotted member of a factory spelling, splitting on
/// `.` outside any generic argument list so `new Maker().Make<HelperA>`
/// yields `Make<HelperA>`, `Factory.MakeNestedArray<HelperA>` yields
/// `MakeNestedArray<HelperA>`, and `new Box<Outer<Helper>.Inner<Helper>>`
/// yields `Box<Outer<Helper>.Inner<Helper>>`. Empty spellings and unbalanced
/// angle lists return `None` and fail closed.
fn csharp_factory_trailing_member(spelling: &str) -> Option<&str> {
    let mut depth = 0usize;
    let mut last_start = 0usize;
    for (index, character) in spelling.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.checked_sub(1)?,
            '.' if depth == 0 => last_start = index + 1,
            _ => {}
        }
    }
    if last_start == spelling.len() {
        return None;
    }
    Some(&spelling[last_start..])
}

/// Extracts the explicit type-argument spellings of the trailing method call
/// in a factory spelling such as `Make<HelperA>`,
/// `holder.MakeHelper<HelperA>`, or `new Maker().Make<HelperA>` into
/// `["HelperA"]`, so call-site type arguments can substitute the method's own
/// type parameters in its declared return type. A spelling whose trailing
/// member is not a generic method call (no type-argument list, a plain
/// member, or a constructed receiver without a trailing method) returns an
/// empty list; malformed or unbalanced argument lists return `None` and fail
/// closed.
fn csharp_factory_method_type_arguments(spelling: &str) -> Option<Vec<String>> {
    let trailing = csharp_factory_trailing_member(spelling)?;
    let mut name = trailing;
    if let Some(open) = trailing.find('[') {
        name = &trailing[..open];
    }
    csharp_method_type_arguments(name)
}

/// Strips the trailing method-call segment's explicit type-argument list from
/// a factory spelling such as `MakeItems<HelperA>`,
/// `Factory.MakeNestedArray<HelperA>`, or `new Maker().MakeArray<HelperA>` to
/// the bare trailing method name, keeping the receiver-chain prefix for
/// dispatch. A spelling whose trailing member is not a generic method call
/// (no type-argument list) keeps the spelling unchanged; malformed or
/// unbalanced angle lists return `None` and fail closed.
fn csharp_factory_method_dispatch_name(spelling: &str) -> Option<String> {
    let trailing = csharp_factory_trailing_member(spelling)?;
    if !trailing.contains('<') {
        return Some(spelling.to_string());
    }
    let stripped = strip_csharp_method_type_arguments(trailing)?;
    let prefix_len = spelling.len() - trailing.len();
    Some(format!("{}{}", &spelling[..prefix_len], stripped))
}

/// Parses a method-call hop such as `inner()`, `inner(1)`, or
/// `Make<HelperA>()` into the bare method name (generic type-argument lists
/// stripped) and the call arity.
fn csharp_method_call_hop_spelling(hop: &str) -> Option<(String, usize)> {
    let open = hop.find('(')?;
    let (method_name, arguments) = hop.split_at(open);
    let method_name = strip_csharp_method_type_arguments(method_name)?;
    if method_name.is_empty() {
        return None;
    }
    let arguments = arguments.strip_prefix('(')?.strip_suffix(')')?;
    let arity = if arguments.is_empty() {
        0
    } else {
        arguments.parse::<usize>().ok()?
    };
    Some((method_name.to_string(), arity))
}

/// Parses a method-call hop with an element-access suffix such as
/// `MakeNestedArray()[0]` or `GetMatrix()[0][0]` into the method name, the
/// call arity, and the element-access depth. Hops without a trailing
/// element-access suffix return `None` so they fall through to the plain
/// method-call hop branch, and malformed suffixes fail closed.
fn csharp_method_call_element_access_spelling(hop: &str) -> Option<(String, usize, usize)> {
    let open = hop.find('(')?;
    let mut paren_depth = 0usize;
    let mut close = None;
    for (index, byte) in hop.bytes().enumerate().skip(open) {
        match byte as char {
            '(' => paren_depth += 1,
            ')' => {
                paren_depth = paren_depth.checked_sub(1)?;
                if paren_depth == 0 {
                    close = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let suffix = &hop[close + 1..];
    if !suffix.starts_with('[') {
        return None;
    }
    let (method_name, arity) = csharp_method_call_hop_spelling(&hop[..=close])?;
    let element_depth = csharp_element_access_suffix_depth(suffix)?;
    Some((method_name, arity, element_depth))
}

/// Parses an element-access hop spelling such as `items[0]` or
/// `fieldItems[1]` into the accessed member name. Plain member hops and
/// malformed brackets return `None` so they fall through to member
/// resolution and fail closed.
fn csharp_array_access_member_name(hop: &str) -> Option<&str> {
    let open = hop.find('[')?;
    let (base, bracket) = hop.split_at(open);
    if base.is_empty() || !bracket.ends_with(']') {
        return None;
    }
    Some(base)
}

/// Counts the element-access layers in a hop spelling such as `items[0]`
/// (one) or `fieldItems[0][0]` (two). The bracket suffix must be one or more
/// well-formed, non-nested bracket groups; malformed or empty bases return
/// `None` and fail closed.
fn csharp_array_access_depth(hop: &str) -> Option<usize> {
    let open = hop.find('[')?;
    if hop[..open].is_empty() {
        return None;
    }
    csharp_element_access_suffix_depth(&hop[open..])
}

/// Counts the element-access layers in a bracket suffix such as `[0]` (one),
/// `[0][0]` (two), or `[0, 0]` (one multi-dimensional group). The suffix must
/// be one or more well-formed, non-nested bracket groups; malformed suffixes
/// return `None` and fail closed.
fn csharp_element_access_suffix_depth(suffix: &str) -> Option<usize> {
    let mut rest = suffix;
    let mut depth = 0usize;
    while let Some(close) = rest.find(']') {
        if close == 0 || rest[1..close].contains(['[', ']']) {
            return None;
        }
        depth += 1;
        rest = &rest[close + 1..];
        if rest.is_empty() {
            break;
        }
        if !rest.starts_with('[') {
            return None;
        }
    }
    if depth == 0 || !rest.is_empty() {
        return None;
    }
    Some(depth)
}

/// Returns the byte length of the leading run of well-formed, non-nested
/// bracket groups in an element-access suffix such as `[0]` (length 3) or
/// `[0][0]` (length 6), or `None` when the suffix does not start with a
/// well-formed bracket group. The length splits the bracket suffix from the
/// trailing dotted member chain (`[0][0].GetItems` -> `[0][0]` and
/// `GetItems`), mirroring `csharp_element_access_suffix_depth`.
fn csharp_element_access_suffix_len(suffix: &str) -> Option<usize> {
    let mut rest = suffix;
    let mut end = 0usize;
    while let Some(close) = rest.find(']') {
        if close == 0 || rest[1..close].contains(['[', ']']) {
            return None;
        }
        end += close + 1;
        rest = &rest[close + 1..];
        if rest.is_empty() || !rest.starts_with('[') {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    Some(end)
}

/// Parses a bare factory-call root with a trailing element-access suffix such
/// as `makeItems()` in `makeItems()[0].helper(...)` or `makeMatrix()` in
/// `makeMatrix()[0][0].helper(...)` into the factory name, its call arity,
/// and the element-access depth. The root must be exactly one call followed
/// by one or more bracket groups; malformed brackets and dotted factory names
/// fail closed.
fn csharp_array_factory_call_root_spelling(hop: &str) -> Option<(String, usize, usize)> {
    let open = hop.find('(')?;
    let (method_name, rest) = hop.split_at(open);
    if method_name.is_empty() || method_name.contains('.') {
        return None;
    }
    let bracket_open = rest.find('[')?;
    let call_part = &rest[..bracket_open];
    let arguments = call_part.strip_prefix('(')?.strip_suffix(')')?;
    let arity = if arguments.is_empty() {
        0
    } else {
        arguments.parse::<usize>().ok()?
    };
    let depth = csharp_element_access_suffix_depth(&rest[bracket_open..])?;
    Some((method_name.to_string(), arity, depth))
}

/// Resolves an arity-matched non-static method-call hop such as `inner()` or
/// `inner(1)` in `group.inner().helper(...)` to the declared type of the
/// dispatched method, resolving the return type in the method's own file and
/// enclosing scope. Static hops, arity-mismatched hops, and unknown,
/// ambiguous, primitive, or `void` return types fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# method-call hop type resolution inputs explicit"
)]
fn resolve_csharp_method_call_hop_binding<'a>(
    dispatch_source_symbol: &IndexedSymbol,
    binding: &CSharpBaseTypeBinding,
    method_name: &str,
    hop_arity: usize,
    method_type_arguments: &[String],
    raw_symbols: &'a [IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(CSharpBaseTypeBinding, &'a IndexedSymbol)>> {
    let Some(symbol_id) = resolve_csharp_instance_method_on_binding(
        dispatch_source_symbol,
        binding,
        method_name,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        hop_arity,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(method) = raw_symbols
        .iter()
        .find(|candidate| candidate.symbol_id == symbol_id)
    else {
        return Ok(None);
    };
    let Some(return_type) = method.return_type.as_deref() else {
        return Ok(None);
    };
    if return_type.is_empty() {
        return Ok(None);
    }
    let return_type = substitute_csharp_method_return_type(
        method,
        binding,
        &dispatch_source_symbol.semantic_path,
        return_type,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    let return_type = substitute_csharp_method_type_parameters(
        method,
        method_type_arguments,
        &return_type,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    let Some(next_binding) = resolve_csharp_member_hop_type_binding(
        method,
        &return_type,
        binding,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    Ok(Some((next_binding, method)))
}

/// Dispatches a C# instance member call on an already-resolved receiver
/// binding using the interface, struct, and class/record instance rules.
/// `None` means the final member cannot be uniquely resolved and callers fail
/// closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# instance member dispatch inputs explicit"
)]
fn resolve_csharp_instance_method_on_binding(
    dispatch_source_symbol: &IndexedSymbol,
    binding: &CSharpBaseTypeBinding,
    member_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    // An interface-typed receiver dispatches on the interface's own method
    // declaration or its unique extends chain; class/record ancestor walking
    // below does not apply.
    if let Some(symbol_id) = resolve_csharp_interface_receiver_call(
        dispatch_source_symbol,
        binding,
        member_name,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        call_arity,
        deadline,
    )? {
        return Ok(Some(symbol_id));
    }
    // A struct-typed receiver dispatches on the struct's own method
    // declaration; structs have no ancestor chain.
    if let Some(symbol_id) = resolve_csharp_struct_receiver_call(
        dispatch_source_symbol,
        binding,
        member_name,
        raw_symbols,
        semantic_path_index,
        call_arity,
    ) {
        return Ok(Some(symbol_id));
    }
    // The declared type resolves in the caller's namespace/import scope; the
    // instance method dispatches on that type and its unique class/record
    // ancestor chain.
    let Some(target_path) = csharp_base_method_target_path(
        dispatch_source_symbol,
        raw_symbols,
        semantic_path_index,
        binding,
        member_name,
        call_arity,
        false,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    Ok(resolve_csharp_candidate(
        raw_symbols,
        semantic_path_index,
        &target_path,
        Some(dispatch_source_symbol),
        call_arity,
        CSharpCandidateRequirements {
            node_kind: "method_declaration",
            require_static: false,
            require_instance: true,
            require_same_file: false,
        },
    ))
}

/// Resolves a `this.`-rooted member chain such as `this.member.helper(...)`
/// whose intermediate hops are fields, properties, or events walked through
/// the unique class/record ancestor chain, so a hop inherited from a
/// grandparent base still pins the next hop or final member. The enclosing
/// type must be uniquely declared in the source file; unknown or unresolvable
/// hops and missing or static final members fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# this-chain member dispatch inputs explicit"
)]
fn resolve_csharp_this_member_chain_call(
    source_symbol: &IndexedSymbol,
    chain: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(scope_path) = source_symbol.scope_path.as_deref() else {
        return Ok(None);
    };
    let type_candidates = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.file_path == source_symbol.file_path
                && candidate.semantic_path == scope_path
                && csharp_is_type_declaration(candidate)
        })
        .collect::<Vec<_>>();
    if type_candidates.len() != 1 {
        return Ok(None);
    }
    let type_symbol = type_candidates[0];
    let mut hops = chain.split('.').collect::<Vec<_>>();
    if hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    let Some(final_member) = hops.pop() else {
        return Ok(None);
    };
    let Some((binding, dispatch_source_symbol)) = resolve_csharp_member_chain_binding(
        type_symbol,
        CSharpBaseTypeBinding {
            semantic_type_path: scope_path.to_string(),
            is_global_qualified: true,
            alias_name: None,
            namespace_import_paths: Vec::new(),
            generic_arguments: Vec::new(),
            raw_generic_argument_spellings: Vec::new(),
            enclosing_generic_arguments: Vec::new(),
            raw_enclosing_generic_argument_spellings: Vec::new(),
        },
        &hops,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    resolve_csharp_instance_method_on_binding(
        dispatch_source_symbol,
        &binding,
        final_member,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        call_arity,
        deadline,
    )
}

/// Resolves a `base.`-rooted member chain such as `base.member.helper(...)`
/// or `base.inner().helper(...)`. The enclosing type must be uniquely declared
/// in the source file and must have exactly one unique class/record base
/// declaration; each intermediate hop walks the member-chain and method-call
/// hop rules on the base type before the final member dispatches with the
/// existing class/record, struct, and interface instance rules. Unknown,
/// ambiguous, or unresolvable hops and missing or static final members fail
/// closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# base-rooted member chain resolution inputs explicit"
)]
fn resolve_csharp_base_member_chain_call(
    source_symbol: &IndexedSymbol,
    chain: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(scope_path) = source_symbol.scope_path.as_deref() else {
        return Ok(None);
    };
    let type_candidates = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.file_path == source_symbol.file_path
                && candidate.semantic_path == scope_path
                && csharp_is_type_declaration(candidate)
        })
        .collect::<Vec<_>>();
    if type_candidates.len() != 1 {
        return Ok(None);
    }
    let type_symbol = type_candidates[0];
    let Some(base_binding) = csharp_base_type_binding_for_type(
        type_symbol,
        raw_symbols,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(base_type_path) = csharp_base_type_path(type_symbol, raw_symbols, &base_binding)
    else {
        return Ok(None);
    };
    let base_indexes = semantic_path_index
        .get(&base_type_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| csharp_is_base_constructible_type(&raw_symbols[*index]))
        .collect::<Vec<_>>();
    if base_indexes.len() != 1 {
        return Ok(None);
    }
    let base_symbol = &raw_symbols[base_indexes[0]];
    let mut hops = chain.split('.').collect::<Vec<_>>();
    if hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    let Some(final_member) = hops.pop() else {
        return Ok(None);
    };
    let Some((binding, dispatch_source_symbol)) = resolve_csharp_member_chain_binding(
        base_symbol,
        CSharpBaseTypeBinding {
            semantic_type_path: base_type_path,
            is_global_qualified: true,
            alias_name: None,
            namespace_import_paths: Vec::new(),
            generic_arguments: Vec::new(),
            raw_generic_argument_spellings: Vec::new(),
            enclosing_generic_arguments: Vec::new(),
            raw_enclosing_generic_argument_spellings: Vec::new(),
        },
        &hops,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    resolve_csharp_instance_method_on_binding(
        dispatch_source_symbol,
        &binding,
        final_member,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        call_arity,
        deadline,
    )
}

enum CSharpConstructorReceiverResolution {
    Resolved(String),
    NotConstructorReceiver,
    Blocked,
}

/// Splits a constructed-receiver spelling on `.` outside any generic
/// argument list, so
/// `Box<Outer<HelperA>.Inner<HelperA>>().GetSingle().GetOuterItem().RunA`
/// splits as `Box<Outer<HelperA>.Inner<HelperA>>()` / `GetSingle()` /
/// `GetOuterItem()` / `RunA` instead of splitting the dots inside the type
/// arguments. Unbalanced argument lists and empty segments return `None` and
/// fail closed.
fn csharp_constructor_receiver_segments(reference_name: &str) -> Option<Vec<&str>> {
    let mut segments = Vec::new();
    let mut depth = 0usize;
    let mut last_start = 0usize;
    for (index, character) in reference_name.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.checked_sub(1)?,
            '.' if depth == 0 => {
                if index == last_start {
                    return None;
                }
                segments.push(&reference_name[last_start..index]);
                last_start = index + 1;
            }
            _ => {}
        }
    }
    if last_start == reference_name.len() {
        return None;
    }
    segments.push(&reference_name[last_start..]);
    Some(segments)
}

/// Rejects whitespace in a constructed-receiver type segment such as
/// `Box<HelperA>` or `Pair<HelperA, HelperB>` only when it appears outside a
/// balanced generic argument list; spaces inside argument lists are valid,
/// while stray spaces and unbalanced angle brackets fail closed.
fn csharp_type_segment_rejects_stray_whitespace(segment: &str) -> bool {
    let mut generic_depth = 0usize;
    for character in segment.chars() {
        match character {
            '<' => generic_depth += 1,
            '>' => {
                let Some(next_depth) = generic_depth.checked_sub(1) else {
                    return true;
                };
                generic_depth = next_depth;
            }
            ' ' if generic_depth == 0 => return true,
            _ => {}
        }
    }
    generic_depth != 0
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# constructor receiver resolution inputs explicit"
)]
fn resolve_csharp_constructor_receiver_call(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpConstructorReceiverResolution> {
    // Split on `.` outside any generic argument list so a constructed
    // spelling such as `Box<Outer<HelperA>.Inner<HelperA>>().GetSingle().RunA`
    // keeps the nested type arguments in the marker segment instead of
    // splitting the dots inside them.
    let Some(segments) = csharp_constructor_receiver_segments(reference_name) else {
        return Ok(CSharpConstructorReceiverResolution::Blocked);
    };
    let Some((marker_index, marker_base)) = segments
        .iter()
        .enumerate()
        .find_map(|(index, segment)| segment.strip_suffix("()").map(|base| (index, base)))
    else {
        return Ok(CSharpConstructorReceiverResolution::NotConstructorReceiver);
    };
    let mut type_segments = segments[..marker_index].to_vec();
    type_segments.push(marker_base);
    // The constructed type marker segment may carry a concrete generic
    // argument list such as `Box<HelperA>` or
    // `Box<Outer<HelperA>.Inner<HelperA>>`; every type segment must
    // otherwise normalize to a safe semantic type path with no brackets,
    // parentheses, nullability, or stray whitespace outside balanced generic
    // argument lists so the receiver resolves to a declared type binding that
    // can substitute the generic parameters in member declared types.
    // Malformed spellings fail closed.
    if type_segments.is_empty()
        || type_segments.iter().any(|segment| {
            segment.is_empty()
                || segment.contains(['[', ']', '(', ')', '?'])
                || csharp_type_segment_rejects_stray_whitespace(segment)
                || crate::language::csharp_generic_type_semantic_path(segment).is_none()
        })
    {
        return Ok(CSharpConstructorReceiverResolution::Blocked);
    }
    let member_chain = segments[marker_index + 1..].join(".");
    if member_chain.is_empty() {
        return Ok(CSharpConstructorReceiverResolution::Blocked);
    }
    let type_name = type_segments.join(".");
    let Some(binding) = resolve_csharp_receiver_type_binding(
        source_symbol,
        &type_name,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(CSharpConstructorReceiverResolution::Blocked);
    };
    // A constructor-rooted member chain such as `new Group().Make().Run(1)`
    // or `new Group().holder.helper.Run(1)` walks each intermediate hop as a
    // uniquely declared field, property, event, or arity-matched non-static
    // method-call hop on the constructed type or its unique class/record
    // ancestor chain (nearest declaring ancestor pins the hop) before
    // dispatching the final member; unknown, ambiguous, or unresolvable hops
    // and missing or static final members fail closed.
    if member_chain.contains('.') {
        let hops = member_chain.split('.').collect::<Vec<_>>();
        if hops.iter().any(|hop| hop.is_empty()) {
            return Ok(CSharpConstructorReceiverResolution::Blocked);
        }
        let Some(final_member) = hops.last() else {
            return Ok(CSharpConstructorReceiverResolution::Blocked);
        };
        // A var-marker factory chain keeps the call-site generic type-argument
        // list on the final member (`new Maker().Make<HelperA>()` spells
        // `Make<HelperA>`), so dispatch strips it to the bare method name;
        // direct references already strip type arguments at extraction.
        let Some(final_member) = strip_csharp_method_type_arguments(final_member) else {
            return Ok(CSharpConstructorReceiverResolution::Blocked);
        };
        let Some((binding, dispatch_source_symbol)) = resolve_csharp_member_chain_binding(
            source_symbol,
            binding,
            &hops[..hops.len() - 1],
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(CSharpConstructorReceiverResolution::Blocked);
        };
        match resolve_csharp_instance_method_on_binding(
            dispatch_source_symbol,
            &binding,
            final_member,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            Some(symbol_id) => Ok(CSharpConstructorReceiverResolution::Resolved(symbol_id)),
            None => Ok(CSharpConstructorReceiverResolution::Blocked),
        }
    } else {
        // A var-marker factory chain keeps the call-site generic type-argument
        // list on the final member (`new Maker().Make<HelperA>()` spells
        // `Make<HelperA>`), so dispatch strips it to the bare method name;
        // direct references already strip type arguments at extraction.
        let Some(member_name) = strip_csharp_method_type_arguments(&member_chain) else {
            return Ok(CSharpConstructorReceiverResolution::Blocked);
        };
        // A struct-typed receiver dispatches on the struct's own method
        // declaration; structs have no ancestor chain.
        if let Some(symbol_id) = resolve_csharp_struct_receiver_call(
            source_symbol,
            &binding,
            member_name,
            raw_symbols,
            semantic_path_index,
            call_arity,
        ) {
            return Ok(CSharpConstructorReceiverResolution::Resolved(symbol_id));
        }
        // The constructed type resolves in the caller's namespace/import
        // scope; the instance method dispatches on that type and its unique
        // class/record ancestor chain.
        let Some(target_path) = csharp_base_method_target_path(
            source_symbol,
            raw_symbols,
            semantic_path_index,
            &binding,
            member_name,
            call_arity,
            false,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(CSharpConstructorReceiverResolution::Blocked);
        };
        match resolve_csharp_candidate(
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
        ) {
            Some(symbol_id) => Ok(CSharpConstructorReceiverResolution::Resolved(symbol_id)),
            None => Ok(CSharpConstructorReceiverResolution::Blocked),
        }
    }
}

/// Resolves a declared receiver type name to a base-type binding. Dotted
/// spellings such as `NestedContainer.Inner` resolve through the caller's
/// namespace ancestors and then the global scope; simple, generic, and
/// `global::`-qualified spellings reuse the namespace/import/alias resolution
/// for declared types.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# receiver type binding inputs explicit"
)]
fn resolve_csharp_receiver_type_binding(
    source_symbol: &IndexedSymbol,
    type_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    // A nullable reference-type spelling such as `Helper?` or `Outer.Inner?`
    // dispatches on the underlying type; a nullable value type such as
    // `Point?` does not expose the underlying struct's members directly, so
    // it fails closed below.
    let nullable = type_name.ends_with('?');
    let type_name = type_name.strip_suffix('?').unwrap_or(type_name);
    if type_name.is_empty() {
        return Ok(None);
    }
    // A constructed generic spelling such as `Box<Helper>` records its
    // top-level type arguments so member-chain resolution can substitute the
    // generic type's parameters in member declared types.
    let generic_arguments =
        crate::language::csharp_generic_type_arguments(type_name).unwrap_or_default();
    let binding = if !type_name.starts_with("global::")
        && let Some(semantic_path) = csharp_receiver_type_semantic_path(type_name)
        && semantic_path.contains("::")
    {
        let scoped_type_path = csharp_scoped_receiver_type_path(
            source_symbol,
            raw_symbols,
            semantic_path_index,
            &semantic_path,
            csharp_is_type_declaration,
        );
        let type_path = match scoped_type_path {
            Some(type_path) => Some(type_path),
            None => {
                // A dotted nested spelling whose first segment is neither a
                // local namespace type nor a global type may still resolve
                // through the same namespace-import and alias rules as
                // receiver type references, so `Outer<Helper>.Inner<Helper>`
                // with `using Lib;` reaches the imported `Lib::Outer::Inner`
                // instead of failing closed.
                let mut type_path = resolve_csharp_namespace_imported_nested_type_path(
                    source_symbol,
                    type_name,
                    raw_symbols,
                    semantic_path_index,
                    source_namespace_path,
                    csharp_global_import_context,
                    file_overrides,
                    csharp_import_contexts_by_file,
                    deadline,
                )?;
                if type_path.is_none() {
                    type_path = resolve_csharp_namespace_imported_dotted_type_path(
                        source_symbol,
                        type_name,
                        raw_symbols,
                        semantic_path_index,
                        source_namespace_path,
                        csharp_global_import_context,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                    )?;
                }
                if type_path.is_none() {
                    type_path = resolve_csharp_alias_to_dotted_type_path(
                        source_symbol,
                        type_name,
                        raw_symbols,
                        semantic_path_index,
                        source_namespace_path,
                        csharp_global_import_context,
                        file_overrides,
                        csharp_import_contexts_by_file,
                        deadline,
                    )?;
                }
                type_path
            }
        };
        type_path.map(|type_path| CSharpBaseTypeBinding {
            semantic_type_path: type_path,
            is_global_qualified: true,
            alias_name: None,
            namespace_import_paths: Vec::new(),
            generic_arguments: Vec::new(),
            raw_generic_argument_spellings: Vec::new(),
            enclosing_generic_arguments: Vec::new(),
            raw_enclosing_generic_argument_spellings: Vec::new(),
        })
    } else {
        resolve_csharp_declared_type_binding_for_reference(
            &source_symbol.file_path,
            type_name,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
    };
    let Some(mut binding) = binding else {
        return Ok(None);
    };
    // An alias binding carries the alias target's generic arguments as raw
    // spellings (the alias name itself cannot take type arguments), so a
    // receiver spelled through an alias such as `Alias` with
    // `using Alias = Demo.Derived<HelperA>;` promotes those arguments into
    // the receiver's concrete arguments. Non-alias spellings keep the
    // spelling-derived arguments.
    if binding.alias_name.is_some() && generic_arguments.is_empty() {
        // An alias target's concrete arguments are raw spellings in the
        // alias's file scope (such as `HelperA` in
        // `using Alias = Other.Derived<HelperA>;`), so resolve each spelling
        // to its canonical semantic path before it is composed into member
        // declared types; otherwise a cross-namespace declaring type cannot
        // resolve the substituted spelling. Unresolvable spellings (built-in
        // or unindexed types) stay unchanged and fail closed downstream.
        binding.generic_arguments = binding
            .raw_generic_argument_spellings
            .iter()
            .map(|spelling| {
                csharp_resolve_receiver_type_argument_spelling(
                    source_symbol,
                    spelling,
                    raw_symbols,
                    semantic_path_index,
                )
                .unwrap_or_else(|| spelling.clone())
            })
            .collect();
    } else {
        // A non-alias constructed generic spelling's concrete arguments are
        // raw spellings in the receiver's file scope (such as `HelperA` in
        // `Other.Derived<HelperA>`), so resolve each spelling to its
        // canonical semantic path before it is composed into member declared
        // types; otherwise a cross-namespace declaring type cannot resolve
        // the substituted spelling. Unresolvable spellings (built-in,
        // parameter, or unindexed types) stay unchanged and fail closed
        // downstream.
        binding.generic_arguments = generic_arguments
            .iter()
            .map(|spelling| {
                csharp_resolve_receiver_type_argument_spelling(
                    source_symbol,
                    spelling,
                    raw_symbols,
                    semantic_path_index,
                )
                .unwrap_or_else(|| spelling.clone())
            })
            .collect();
    }
    // A dotted nested spelling such as `Outer<Helper>.Inner<Helper>` also
    // records the concrete type arguments of every enclosing segment
    // (outermost first) so member declared types that reference an outer type
    // parameter (`T[] outerItems` on `Inner<U>`) substitute `T`. The
    // argument vectors align with the resolved type chain's type segments:
    // leading namespace segments in the raw spelling (such as `Lib` in
    // `Lib.Outer<Helper>.Inner<Helper>` or a `global::` prefix) carry no
    // arguments, so the tail of the raw per-segment argument spellings maps
    // onto the resolved type chain regardless of how the type was spelled.
    binding.enclosing_generic_arguments =
        crate::language::csharp_generic_type_arguments_per_segment(type_name)
            .map(|segments| {
                let mut type_segment_count = 0usize;
                let mut prefix = binding.semantic_type_path.as_str();
                loop {
                    if semantic_path_index
                        .get(prefix)
                        .into_iter()
                        .flatten()
                        .any(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                    {
                        type_segment_count += 1;
                    }
                    match prefix.rsplit_once("::") {
                        Some((parent, _)) => prefix = parent,
                        None => break,
                    }
                }
                let chain_start = segments.len().saturating_sub(type_segment_count);
                let enclosing_end = segments.len().saturating_sub(1);
                if chain_start <= enclosing_end {
                    segments[chain_start..enclosing_end]
                        .iter()
                        .map(|segment| {
                            segment
                                .iter()
                                .map(|spelling| {
                                    csharp_resolve_receiver_type_argument_spelling(
                                        source_symbol,
                                        spelling,
                                        raw_symbols,
                                        semantic_path_index,
                                    )
                                    .unwrap_or_else(|| spelling.clone())
                                })
                                .collect()
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            })
            .unwrap_or_default();
    // An alias target's enclosing generic segments (such as `Outer<HelperA>`
    // in `using Alias = Demo.Outer<HelperA>.Inner<HelperB>;`) carry their
    // arguments through the alias binding; the raw per-segment spellings are
    // aligned by dropping leading namespace segments that hold no arguments.
    if binding.alias_name.is_some() && binding.enclosing_generic_arguments.is_empty() {
        binding.enclosing_generic_arguments = binding
            .raw_enclosing_generic_argument_spellings
            .iter()
            .skip_while(|segment| segment.is_empty())
            .map(|segment| {
                segment
                    .iter()
                    .map(|spelling| {
                        csharp_resolve_receiver_type_argument_spelling(
                            source_symbol,
                            spelling,
                            raw_symbols,
                            semantic_path_index,
                        )
                        .unwrap_or_else(|| spelling.clone())
                    })
                    .collect()
            })
            .collect();
    }
    if nullable && csharp_struct_type_path(source_symbol, raw_symbols, &binding).is_some() {
        // A nullable value type such as `Point?` does not expose the
        // underlying struct's members directly; the receiver must be
        // accessed through `.Value`, so dispatch fails closed.
        return Ok(None);
    }
    Ok(Some(binding))
}

/// Dispatches an instance call on an interface-typed receiver to a unique
/// non-static, non-`params`, exact-arity method declared on the interface or
/// on one branch of its unique interface-extends chain. Non-interface
/// declared types return `None` so class/record ancestor resolution can
/// proceed; unresolved, ambiguous, cyclic, method-less, or statically
/// declared interface targets also return `None` and fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# interface receiver resolution inputs explicit"
)]
fn resolve_csharp_interface_receiver_call(
    source_symbol: &IndexedSymbol,
    binding: &CSharpBaseTypeBinding,
    member_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(interface_path) =
        csharp_interface_type_path(source_symbol, raw_symbols, semantic_path_index, binding)
    else {
        return Ok(None);
    };
    let mut visited_interface_paths = BTreeSet::new();
    Ok(
        match resolve_csharp_interface_chain_method(
            &interface_path,
            member_name,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            call_arity,
            deadline,
            &mut visited_interface_paths,
        )? {
            CSharpInterfaceChainMethodResolution::Resolved(symbol_id) => Some(symbol_id),
            CSharpInterfaceChainMethodResolution::NoMethod
            | CSharpInterfaceChainMethodResolution::Blocked => None,
        },
    )
}

enum CSharpInterfaceChainMethodResolution {
    Resolved(String),
    NoMethod,
    Blocked,
}

/// Walks an interface's direct parent-interface branches recursively to
/// resolve `method_name`. A declaration on an interface shadows inheritance:
/// a non-matching (static, `params`, or arity-mismatched) declaration blocks
/// parent lookup rather than falling through. Exactly one branch must provide
/// a uniquely arity-matched non-static method, every other branch must prove
/// it has no declaration, and a declaration reached identically through
/// multiple branches still resolves once. Competing, ambiguous, cyclic,
/// unresolvable, and statically-declared targets fail closed as `Blocked`.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# interface inheritance resolution inputs explicit"
)]
fn resolve_csharp_interface_chain_method(
    interface_path: &str,
    method_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
    visited_interface_paths: &mut BTreeSet<String>,
) -> Result<CSharpInterfaceChainMethodResolution> {
    if let Some(deadline) = deadline {
        deadline.check("resolving C# interface chain method")?;
    }
    if !visited_interface_paths.insert(interface_path.to_string()) {
        return Ok(CSharpInterfaceChainMethodResolution::Blocked);
    }
    let target_path = format!("{interface_path}::{method_name}");
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
                !csharp_method_is_static(candidate)
                    && candidate.parameters.len() == call_arity
                    && !candidate
                        .parameters
                        .iter()
                        .any(|parameter| parameter.split_whitespace().any(|part| part == "params"))
            })
            .collect::<Vec<_>>();
        let resolution = match candidates.as_slice() {
            [candidate_index] => CSharpInterfaceChainMethodResolution::Resolved(
                raw_symbols[*candidate_index].symbol_id.clone(),
            ),
            _ => CSharpInterfaceChainMethodResolution::Blocked,
        };
        visited_interface_paths.remove(interface_path);
        return Ok(resolution);
    }

    let interface_candidates = semantic_path_index
        .get(interface_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| raw_symbols[*index].node_kind == "interface_declaration")
        .collect::<Vec<_>>();
    let [interface_index] = interface_candidates.as_slice() else {
        visited_interface_paths.remove(interface_path);
        return Ok(CSharpInterfaceChainMethodResolution::Blocked);
    };
    let source_interface = &raw_symbols[*interface_index];
    let source_namespace_path =
        csharp_source_namespace_path(source_interface, raw_symbols).flatten();
    let parent_bindings = match csharp_interface_parent_bindings_for_interface(
        &source_interface.file_path,
        source_interface.byte_range,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? {
        CSharpInterfaceParents::None => {
            visited_interface_paths.remove(interface_path);
            return Ok(CSharpInterfaceChainMethodResolution::NoMethod);
        }
        CSharpInterfaceParents::Blocked => {
            visited_interface_paths.remove(interface_path);
            return Ok(CSharpInterfaceChainMethodResolution::Blocked);
        }
        CSharpInterfaceParents::Parents(parent_bindings) => parent_bindings,
    };
    let mut resolved_symbol_id = None;
    for parent_binding in parent_bindings {
        let Some(parent_interface_path) = csharp_interface_type_path(
            source_interface,
            raw_symbols,
            semantic_path_index,
            &parent_binding,
        ) else {
            visited_interface_paths.remove(interface_path);
            return Ok(CSharpInterfaceChainMethodResolution::Blocked);
        };
        match resolve_csharp_interface_chain_method(
            &parent_interface_path,
            method_name,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            call_arity,
            deadline,
            visited_interface_paths,
        )? {
            CSharpInterfaceChainMethodResolution::Resolved(symbol_id) => {
                if resolved_symbol_id
                    .as_deref()
                    .is_some_and(|resolved| resolved != symbol_id)
                {
                    visited_interface_paths.remove(interface_path);
                    return Ok(CSharpInterfaceChainMethodResolution::Blocked);
                }
                resolved_symbol_id.get_or_insert(symbol_id);
            }
            CSharpInterfaceChainMethodResolution::Blocked => {
                visited_interface_paths.remove(interface_path);
                return Ok(CSharpInterfaceChainMethodResolution::Blocked);
            }
            CSharpInterfaceChainMethodResolution::NoMethod => {}
        }
    }
    visited_interface_paths.remove(interface_path);
    Ok(resolved_symbol_id
        .map(CSharpInterfaceChainMethodResolution::Resolved)
        .unwrap_or(CSharpInterfaceChainMethodResolution::NoMethod))
}

enum CSharpInterfaceMemberHopResolution<'a> {
    Resolved(&'a IndexedSymbol, String),
    NoHop,
    Blocked,
}

/// Resolves a field/property/event hop declared on an interface or on one
/// branch of its unique interface-extends chain, mirroring interface method
/// dispatch. A declaration on an interface shadows inheritance: an
/// unresolvable same-name declaration blocks parent lookup rather than
/// falling through. Exactly one branch must provide a uniquely resolvable
/// hop, every other branch must prove it has no declaration, and a
/// declaration reached identically through multiple branches still resolves
/// once. A generic interface receiver's concrete type arguments are
/// substituted through the extends chain, so `T[] items` declared on
/// `IBase<T>` resolves to `Helper[]` for an `IGeneric<Helper> : IBase<T>`
/// receiver and for non-generic parents spelled with concrete arguments
/// (`IFixed : IBase<Helper>`), while primitive element types, unknown or
/// arity-mismatched arguments, and receivers without concrete arguments fail
/// closed. Competing, ambiguous, cyclic, and unresolvable hops fail closed
/// as `Blocked`.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# interface member-hop resolution inputs explicit"
)]
fn resolve_csharp_interface_member_hop<'a>(
    interface_symbol: &'a IndexedSymbol,
    hop: &str,
    current_type_args: &[String],
    raw_symbols: &'a [IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
    visited_interface_paths: &mut BTreeSet<String>,
) -> Result<CSharpInterfaceMemberHopResolution<'a>> {
    if let Some(deadline) = deadline {
        deadline.check("resolving C# interface member hop")?;
    }
    if !visited_interface_paths.insert(interface_symbol.semantic_path.clone()) {
        return Ok(CSharpInterfaceMemberHopResolution::Blocked);
    }
    // An element-access hop such as `items[0]` or `fieldItems[0][0]` looks up
    // the named member through the same interface-extends chain as a plain
    // hop and pins the resolved array's element component type, stripping one
    // component layer per element access, while non-array members fail
    // closed.
    let array_member_name = csharp_array_access_member_name(hop);
    let element_depth = csharp_array_access_depth(hop);
    let member_name = array_member_name.unwrap_or(hop);
    let member_bindings = match csharp_member_type_bindings_for_type(
        &interface_symbol.file_path,
        interface_symbol.byte_range,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? {
        Some(member_bindings) => member_bindings,
        None => {
            visited_interface_paths.remove(&interface_symbol.semantic_path);
            return Ok(CSharpInterfaceMemberHopResolution::Blocked);
        }
    };
    let parameters = csharp_type_parameter_names_for_type(
        &interface_symbol.file_path,
        interface_symbol.byte_range,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    .unwrap_or_default();
    if member_bindings.contains(member_name) {
        let resolution = match member_bindings.type_for(member_name) {
            Some(declared_type) => {
                let hop_type_name = if array_member_name.is_some() {
                    let Some(depth) = element_depth else {
                        return Ok(CSharpInterfaceMemberHopResolution::Blocked);
                    };
                    csharp_array_component_spelling_at_depth(&declared_type, depth)
                } else {
                    Some(declared_type)
                };
                match hop_type_name {
                    // A generic declaring interface substitutes its type
                    // parameters with the concrete arguments composed for it
                    // through the extends chain; a binding without concrete
                    // arguments leaves the declared type unchanged and defers
                    // generic-member substitution to downstream fail-closed
                    // checks.
                    Some(hop_type_name) => {
                        let hop_type_name = if !parameters.is_empty()
                            && !current_type_args.is_empty()
                            && parameters.len() == current_type_args.len()
                        {
                            substitute_csharp_type_parameters(
                                &hop_type_name,
                                &parameters,
                                current_type_args,
                            )
                        } else {
                            hop_type_name
                        };
                        CSharpInterfaceMemberHopResolution::Resolved(
                            interface_symbol,
                            hop_type_name,
                        )
                    }
                    None => CSharpInterfaceMemberHopResolution::Blocked,
                }
            }
            None => CSharpInterfaceMemberHopResolution::Blocked,
        };
        visited_interface_paths.remove(&interface_symbol.semantic_path);
        return Ok(resolution);
    }

    let source_namespace_path =
        csharp_source_namespace_path(interface_symbol, raw_symbols).flatten();
    let parent_bindings = match csharp_interface_parent_bindings_for_interface(
        &interface_symbol.file_path,
        interface_symbol.byte_range,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? {
        CSharpInterfaceParents::None => {
            visited_interface_paths.remove(&interface_symbol.semantic_path);
            return Ok(CSharpInterfaceMemberHopResolution::NoHop);
        }
        CSharpInterfaceParents::Blocked => {
            visited_interface_paths.remove(&interface_symbol.semantic_path);
            return Ok(CSharpInterfaceMemberHopResolution::Blocked);
        }
        CSharpInterfaceParents::Parents(parent_bindings) => parent_bindings,
    };
    if !parameters.is_empty() && parameters.len() != current_type_args.len() {
        visited_interface_paths.remove(&interface_symbol.semantic_path);
        return Ok(CSharpInterfaceMemberHopResolution::Blocked);
    }
    let mut resolved_hop = None;
    for parent_binding in parent_bindings {
        let Some(parent_interface_path) = csharp_interface_type_path(
            interface_symbol,
            raw_symbols,
            semantic_path_index,
            &parent_binding,
        ) else {
            visited_interface_paths.remove(&interface_symbol.semantic_path);
            return Ok(CSharpInterfaceMemberHopResolution::Blocked);
        };
        let parent_indexes = semantic_path_index
            .get(&parent_interface_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| raw_symbols[*index].node_kind == "interface_declaration")
            .collect::<Vec<_>>();
        let [parent_index] = parent_indexes.as_slice() else {
            visited_interface_paths.remove(&interface_symbol.semantic_path);
            return Ok(CSharpInterfaceMemberHopResolution::Blocked);
        };
        // Compose the parent interface's concrete arguments: a generic
        // interface substitutes its type parameters with the receiver's
        // concrete arguments into the parent's raw spellings (`IGeneric<T> :
        // IBase<T>` with `["Helper"]` yields `["Helper"]`), while a
        // non-generic interface carries the parent's concrete spellings
        // directly (`IFixed : IBase<Helper>` yields `["Helper"]`).
        let parent_args = if parameters.is_empty() {
            parent_binding.raw_generic_argument_spellings.clone()
        } else {
            parent_binding
                .raw_generic_argument_spellings
                .iter()
                .map(|spelling| {
                    substitute_csharp_type_parameters(spelling, &parameters, current_type_args)
                })
                .collect()
        };
        match resolve_csharp_interface_member_hop(
            &raw_symbols[*parent_index],
            hop,
            &parent_args,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
            visited_interface_paths,
        )? {
            CSharpInterfaceMemberHopResolution::Resolved(declaring_type, hop_type_name) => {
                if resolved_hop.as_ref().is_some_and(
                    |(resolved_type, resolved_name): &(&IndexedSymbol, String)| {
                        resolved_type.symbol_id != declaring_type.symbol_id
                            || *resolved_name != hop_type_name
                    },
                ) {
                    visited_interface_paths.remove(&interface_symbol.semantic_path);
                    return Ok(CSharpInterfaceMemberHopResolution::Blocked);
                }
                resolved_hop.get_or_insert((declaring_type, hop_type_name));
            }
            CSharpInterfaceMemberHopResolution::Blocked => {
                visited_interface_paths.remove(&interface_symbol.semantic_path);
                return Ok(CSharpInterfaceMemberHopResolution::Blocked);
            }
            CSharpInterfaceMemberHopResolution::NoHop => {}
        }
    }
    visited_interface_paths.remove(&interface_symbol.semantic_path);
    Ok(resolved_hop
        .map(|(declaring_type, hop_type_name)| {
            CSharpInterfaceMemberHopResolution::Resolved(declaring_type, hop_type_name)
        })
        .unwrap_or(CSharpInterfaceMemberHopResolution::NoHop))
}

/// Dispatches an instance call on a struct-typed receiver to a unique
/// non-static, non-`params`, exact-arity method declared directly on the
/// struct. Structs cannot inherit, so there is no ancestor walk; unknown,
/// ambiguous, static, missing, or non-struct declared types return `None` and
/// fail closed.
fn resolve_csharp_struct_receiver_call(
    source_symbol: &IndexedSymbol,
    binding: &CSharpBaseTypeBinding,
    member_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    call_arity: usize,
) -> Option<String> {
    let struct_type_path = csharp_struct_type_path(source_symbol, raw_symbols, binding)?;
    let target_path = format!("{struct_type_path}::{member_name}");
    resolve_csharp_candidate(
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
    )
}

/// Resolves a declared interface type name to a unique indexed interface
/// path. Simple names resolve through the caller's namespace, enclosing
/// namespaces, namespace imports, and then the global scope; dotted names
/// walk the same scope chain with the full path. Ambiguous or missing
/// interfaces return `None` and fail closed.
fn csharp_interface_type_path(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    binding: &CSharpBaseTypeBinding,
) -> Option<String> {
    if binding.is_global_qualified {
        return csharp_unique_interface_type_path(raw_symbols, &binding.semantic_type_path);
    }
    if binding.semantic_type_path.contains("::") {
        return csharp_scoped_receiver_type_path(
            source_symbol,
            raw_symbols,
            semantic_path_index,
            &binding.semantic_type_path,
            csharp_is_interface_declaration,
        );
    }
    let local_path = csharp_source_namespace_path(source_symbol, raw_symbols)?
        .map(|namespace_path| format!("{namespace_path}::{}", binding.semantic_type_path))
        .unwrap_or_else(|| binding.semantic_type_path.clone());
    if let Some(path) = csharp_unique_interface_type_path(raw_symbols, &local_path) {
        return Some(path);
    }
    for import_path in &binding.namespace_import_paths {
        let candidate_path = format!("{import_path}::{}", binding.semantic_type_path);
        if let Some(path) = csharp_unique_interface_type_path(raw_symbols, &candidate_path) {
            return Some(path);
        }
    }
    csharp_unique_interface_type_path(raw_symbols, &binding.semantic_type_path)
}

fn csharp_unique_interface_type_path(
    raw_symbols: &[IndexedSymbol],
    type_path: &str,
) -> Option<String> {
    (csharp_interface_type_candidates(raw_symbols, type_path).len() == 1)
        .then(|| type_path.to_string())
}

fn csharp_interface_type_candidates<'a>(
    raw_symbols: &'a [IndexedSymbol],
    type_path: &str,
) -> Vec<&'a IndexedSymbol> {
    raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.semantic_path == type_path && candidate.node_kind == "interface_declaration"
        })
        .collect()
}

/// Resolves a type spelling such as `NestedContainer.Inner` to a unique type
/// path of the requested kind. The caller's namespace ancestors are searched
/// innermost-first, then the global scope; the first scope with candidates
/// must be unambiguous or resolution fails closed.
fn csharp_scoped_receiver_type_path(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    semantic_path: &str,
    is_target_type: fn(&IndexedSymbol) -> bool,
) -> Option<String> {
    let mut namespace_path = csharp_source_namespace_path(source_symbol, raw_symbols).flatten();
    while let Some(current_namespace) = namespace_path {
        let candidate_path = format!("{current_namespace}::{semantic_path}");
        let candidates = csharp_receiver_type_candidates(
            raw_symbols,
            semantic_path_index,
            &candidate_path,
            is_target_type,
        );
        match candidates.as_slice() {
            [_] => return Some(candidate_path),
            [] => {}
            _ => return None,
        }
        namespace_path = current_namespace
            .rsplit_once("::")
            .map(|(parent, _)| parent);
    }
    let candidates = csharp_receiver_type_candidates(
        raw_symbols,
        semantic_path_index,
        semantic_path,
        is_target_type,
    );
    match candidates.as_slice() {
        [_] => Some(semantic_path.to_string()),
        [] => None,
        _ => None,
    }
}

/// Computes the semantic path for a receiver type spelling, accepting either
/// a dot-qualified C# spelling such as `Other.Derived<HelperA>` (normalized
/// through the language helper) or an already-normalized canonical semantic
/// path such as `Demo::HelperA` produced by factory return-type substitution
/// when the declaring type and the substituting receiver live in different
/// namespaces.
fn csharp_receiver_type_semantic_path(type_name: &str) -> Option<String> {
    if type_name.contains("::") {
        // An already-canonical spelling separates namespace and type segments
        // with `::`; strip any balanced generic argument lists (such as the
        // substituted `Box<Lib::Helper>` hop type) so the bare semantic path
        // stays indexable, and normalize any residual dot-qualified segments.
        let normalized = type_name.strip_prefix("global::").unwrap_or(type_name);
        let mut stripped = String::with_capacity(normalized.len());
        let mut depth = 0usize;
        for character in normalized.chars() {
            match character {
                '<' => depth += 1,
                '>' => depth = depth.checked_sub(1)?,
                _ if depth == 0 => stripped.push(character),
                _ => {}
            }
        }
        if depth != 0 || stripped.is_empty() {
            return None;
        }
        let semantic_path = stripped.replace('.', "::");
        semantic_path
            .split("::")
            .all(|segment| !segment.is_empty())
            .then_some(semantic_path)
    } else {
        crate::language::csharp_generic_type_semantic_path(type_name)
    }
}

/// Resolves a concrete generic argument spelling from a type alias's target
/// (such as `HelperA` in `using Alias = Other.Derived<HelperA>;`) to its
/// canonical semantic type path in the alias's file scope, so substituted
/// member declared types resolve in the declaring type's own namespace.
/// Constructed generic spellings, unresolvable spellings, and primitive or
/// unindexed types return `None` and keep the raw spelling unchanged.
fn csharp_resolve_receiver_type_argument_spelling(
    source_symbol: &IndexedSymbol,
    spelling: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Option<String> {
    if spelling.contains('<') {
        return None;
    }
    let semantic_path = crate::language::csharp_generic_type_semantic_path(spelling)?;
    csharp_scoped_receiver_type_path(
        source_symbol,
        raw_symbols,
        semantic_path_index,
        &semantic_path,
        csharp_is_type_declaration,
    )
}

fn csharp_receiver_type_candidates(
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    type_path: &str,
    is_target_type: fn(&IndexedSymbol) -> bool,
) -> Vec<usize> {
    semantic_path_index
        .get(type_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| is_target_type(&raw_symbols[*index]))
        .collect()
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
    csharp_dispatchable_type_path(
        source_symbol,
        raw_symbols,
        binding,
        csharp_is_base_constructible_type,
    )
}

/// Composes the concrete generic type arguments for `target_type_path` when
/// it is reached from `source_type_path` through the unique class/record
/// ancestor chain or the interface-extends chain, substituting each walked
/// type's declared type parameters with its current concrete arguments into
/// the next base-list or parent-interface spelling. The source's own
/// arguments seed the walk, so a `Derived<Helper>` receiver reaching base
/// `Box<T>` yields `["Helper"]`, a non-generic `Fixed : Box<Helper>` receiver
/// reaching `Box<T>` yields `["Helper"]` from the base spelling, an
/// `IGeneric<Helper> : IBase<T>` receiver reaching `IBase<T>` yields
/// `["Helper"]`, and multi-level chains compose per level. When an interface
/// reaches the target through several parent branches, every branch's
/// composed arguments must agree or the mapping is ambiguous. `None` means
/// the target is not reachable through a unique class/record or
/// interface-extends walk, a parent or base chain cannot be resolved or
/// walked uniquely, or a parameter/argument arity mismatch blocks the
/// mapping, so callers fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# generic inheritance composition inputs explicit"
)]
fn csharp_compose_generic_arguments_to_type(
    source_type_path: &str,
    source_type_args: &[String],
    target_type_path: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<Vec<String>>> {
    let mut visited_type_paths = BTreeSet::new();
    let mut composed_arguments = Vec::new();
    csharp_collect_generic_argument_compositions(
        source_type_path,
        source_type_args,
        target_type_path,
        &mut visited_type_paths,
        &mut composed_arguments,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    let Some(first) = composed_arguments.first() else {
        return Ok(None);
    };
    if composed_arguments
        .iter()
        .any(|arguments| arguments != first)
    {
        return Ok(None);
    }
    Ok(Some(first.clone()))
}

/// Collects the composed generic argument vectors for every path that
/// reaches `target_type_path` from `current_type_path`, walking class/record
/// bases and interface parents recursively. A path that terminates without
/// reaching the target, a cycle, or an unresolvable step contributes nothing;
/// the caller requires every collected composition to agree.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# generic inheritance composition inputs explicit"
)]
fn csharp_collect_generic_argument_compositions(
    current_type_path: &str,
    current_type_args: &[String],
    target_type_path: &str,
    visited_type_paths: &mut BTreeSet<String>,
    composed_arguments: &mut Vec<Vec<String>>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<()> {
    if current_type_path == target_type_path {
        composed_arguments.push(current_type_args.to_vec());
        return Ok(());
    }
    if !visited_type_paths.insert(current_type_path.to_string()) {
        return Ok(());
    }
    let Some(type_indexes) = semantic_path_index.get(current_type_path) else {
        visited_type_paths.remove(current_type_path);
        return Ok(());
    };
    let type_indexes = type_indexes
        .iter()
        .copied()
        .filter(|index| {
            csharp_is_base_constructible_type(&raw_symbols[*index])
                || raw_symbols[*index].node_kind == "interface_declaration"
        })
        .collect::<Vec<_>>();
    if type_indexes.len() != 1 {
        visited_type_paths.remove(current_type_path);
        return Ok(());
    }
    let current_type_symbol = &raw_symbols[type_indexes[0]];
    let parameters = csharp_type_parameter_names_for_type(
        &current_type_symbol.file_path,
        current_type_symbol.byte_range,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    .unwrap_or_default();
    if parameters.len() != current_type_args.len() {
        visited_type_paths.remove(current_type_path);
        return Ok(());
    }
    if current_type_symbol.node_kind == "interface_declaration" {
        let source_namespace_path =
            csharp_source_namespace_path(current_type_symbol, raw_symbols).flatten();
        let parent_bindings = match csharp_interface_parent_bindings_for_interface(
            &current_type_symbol.file_path,
            current_type_symbol.byte_range,
            source_namespace_path,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )? {
            CSharpInterfaceParents::None => {
                visited_type_paths.remove(current_type_path);
                return Ok(());
            }
            CSharpInterfaceParents::Blocked => {
                visited_type_paths.remove(current_type_path);
                return Ok(());
            }
            CSharpInterfaceParents::Parents(parent_bindings) => parent_bindings,
        };
        for parent_binding in parent_bindings {
            let Some(parent_interface_path) = csharp_interface_type_path(
                current_type_symbol,
                raw_symbols,
                semantic_path_index,
                &parent_binding,
            ) else {
                visited_type_paths.remove(current_type_path);
                return Ok(());
            };
            let parent_args: Vec<String> = parent_binding
                .raw_generic_argument_spellings
                .iter()
                .map(|spelling| {
                    substitute_csharp_type_parameters(spelling, &parameters, current_type_args)
                })
                .collect();
            csharp_collect_generic_argument_compositions(
                &parent_interface_path,
                &parent_args,
                target_type_path,
                visited_type_paths,
                composed_arguments,
                raw_symbols,
                semantic_path_index,
                csharp_global_import_context,
                file_overrides,
                csharp_import_contexts_by_file,
                deadline,
            )?;
        }
    } else {
        let Some(base_binding) = csharp_base_type_binding_for_type(
            current_type_symbol,
            raw_symbols,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?
        else {
            visited_type_paths.remove(current_type_path);
            return Ok(());
        };
        let Some(base_type_path) =
            csharp_base_type_path(current_type_symbol, raw_symbols, &base_binding)
        else {
            visited_type_paths.remove(current_type_path);
            return Ok(());
        };
        let base_args: Vec<String> = base_binding
            .raw_generic_argument_spellings
            .iter()
            .map(|spelling| {
                substitute_csharp_type_parameters(spelling, &parameters, current_type_args)
            })
            .collect();
        csharp_collect_generic_argument_compositions(
            &base_type_path,
            &base_args,
            target_type_path,
            visited_type_paths,
            composed_arguments,
            raw_symbols,
            semantic_path_index,
            csharp_global_import_context,
            file_overrides,
            csharp_import_contexts_by_file,
            deadline,
        )?;
    }
    visited_type_paths.remove(current_type_path);
    Ok(())
}

/// Resolves the receiver's declared type to a unique struct path using the
/// same namespace/import/alias rules as base types but restricted to structs.
fn csharp_struct_type_path(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    binding: &CSharpBaseTypeBinding,
) -> Option<String> {
    csharp_dispatchable_type_path(
        source_symbol,
        raw_symbols,
        binding,
        csharp_is_struct_declaration,
    )
}

/// Returns the ordered enclosing type paths of `source_symbol`,
/// innermost first, excluding the source's own type declaration so a type's
/// base list resolves in the containing scope: `Demo::Outer::Mid` reports
/// `["Demo::Outer"]` and a method on `Demo::Outer::Mid` reports
/// `["Demo::Outer::Mid", "Demo::Outer"]`. A symbol without an enclosing
/// type, or an enclosing chain that cannot be walked uniquely, returns
/// `None` so callers fail closed.
fn csharp_enclosing_type_scope_paths<'a>(
    source_symbol: &'a IndexedSymbol,
    raw_symbols: &'a [IndexedSymbol],
) -> Option<Vec<&'a str>> {
    let mut type_paths = Vec::new();
    let mut type_path = source_symbol.scope_path.as_deref()?;
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
            0 => break,
            1 => type_paths.push(type_path),
            _ => return None,
        }
        let Some((parent_path, _)) = type_path.rsplit_once("::") else {
            break;
        };
        type_path = parent_path;
    }
    Some(type_paths)
}

fn csharp_dispatchable_type_path(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    binding: &CSharpBaseTypeBinding,
    is_target_type: fn(&IndexedSymbol) -> bool,
) -> Option<String> {
    if binding.alias_name.as_deref().is_some_and(|alias_name| {
        !csharp_alias_name_is_unshadowed(alias_name, source_symbol, raw_symbols)
    }) {
        return None;
    }
    if binding.is_global_qualified {
        return csharp_unique_dispatchable_type_path(
            raw_symbols,
            &binding.semantic_type_path,
            is_target_type,
        );
    }
    if binding.semantic_type_path.contains("::") {
        return csharp_unshadowed_qualified_dispatchable_type_path(
            source_symbol,
            raw_symbols,
            binding,
            is_target_type,
        );
    }

    // Simple names resolve through the source's enclosing type scopes (so a
    // nested type's base list can name a sibling nested type inside the same
    // containing type), then the source namespace, its enclosing namespaces,
    // then the global scope, matching C# enclosing-scope lookup; the
    // innermost scope with candidates must be unambiguous or resolution
    // fails closed.
    let source_namespace_path = csharp_source_namespace_path(source_symbol, raw_symbols)?;
    let mut type_paths = Vec::new();
    if let Some(enclosing_type_paths) =
        csharp_enclosing_type_scope_paths(source_symbol, raw_symbols)
    {
        for enclosing_type_path in enclosing_type_paths {
            type_paths.push(format!(
                "{enclosing_type_path}::{}",
                binding.semantic_type_path
            ));
        }
    }
    let mut namespace_path = source_namespace_path;
    while let Some(current_namespace) = namespace_path {
        type_paths.push(format!(
            "{current_namespace}::{}",
            binding.semantic_type_path
        ));
        namespace_path = current_namespace
            .rsplit_once("::")
            .map(|(parent_path, _)| parent_path);
    }
    if source_namespace_path.is_none() {
        type_paths.push(binding.semantic_type_path.clone());
    }
    for type_path in type_paths {
        let local_type_candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.semantic_path == type_path && csharp_is_type_declaration(candidate)
            })
            .count();
        if local_type_candidates != 0 {
            return csharp_unique_dispatchable_type_path(raw_symbols, &type_path, is_target_type);
        }
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
            [candidate] if is_target_type(candidate) => {
                imported_type_paths.insert(type_path);
            }
            _ => return None,
        }
    }
    (imported_type_paths.len() == 1).then(|| imported_type_paths.into_iter().next().unwrap())
}

fn csharp_unshadowed_qualified_dispatchable_type_path(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    binding: &CSharpBaseTypeBinding,
    is_target_type: fn(&IndexedSymbol) -> bool,
) -> Option<String> {
    let base_type_path = binding.semantic_type_path.as_str();
    // A dotted spelling such as `Demo.Base` can be read as a namespace-
    // qualified path, or as a nested-type path whose first segment is itself
    // a type name (`Outer.Inner`). A type declaration at the same relative
    // path inside an enclosing namespace makes the nested-type reading apply;
    // when both readings match the spelling is ambiguous and fails closed.
    let mut nested_type_shadow = false;
    if let Some(mut namespace_path) = csharp_source_namespace_path(source_symbol, raw_symbols)? {
        loop {
            let relative_type_path = format!("{namespace_path}::{base_type_path}");
            if raw_symbols.iter().any(|candidate| {
                candidate.semantic_path == relative_type_path
                    && csharp_is_type_declaration(candidate)
            }) {
                nested_type_shadow = true;
                break;
            }
            let Some((parent_namespace_path, _)) = namespace_path.rsplit_once("::") else {
                break;
            };
            namespace_path = parent_namespace_path;
        }
    }
    let qualified_target =
        csharp_unique_dispatchable_type_path(raw_symbols, base_type_path, is_target_type);
    if !nested_type_shadow {
        if let Some(qualified_target) = qualified_target {
            return Some(qualified_target);
        }
    } else if qualified_target.is_some() {
        return None;
    }

    // The first segment is a type name; resolve the nested type through the
    // enclosing namespaces (innermost first), then the global scope, then the
    // file's namespace imports, matching the receiver-type rules for dotted
    // nested spellings.
    let mut namespace_path = csharp_source_namespace_path(source_symbol, raw_symbols).flatten();
    while let Some(current_namespace) = namespace_path {
        let relative_type_path = format!("{current_namespace}::{base_type_path}");
        let candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.semantic_path == relative_type_path
                    && csharp_is_type_declaration(candidate)
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [candidate] if is_target_type(candidate) => return Some(relative_type_path),
            [] => {}
            _ => return None,
        }
        namespace_path = current_namespace
            .rsplit_once("::")
            .map(|(parent_namespace_path, _)| parent_namespace_path);
    }
    let mut imported_type_paths = BTreeSet::new();
    for namespace_path in binding.namespace_import_paths.iter() {
        let candidate_path = format!("{namespace_path}::{base_type_path}");
        let candidates = raw_symbols
            .iter()
            .filter(|candidate| {
                candidate.semantic_path == candidate_path && csharp_is_type_declaration(candidate)
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => {}
            [candidate] if is_target_type(candidate) => {
                imported_type_paths.insert(candidate_path);
            }
            _ => return None,
        }
    }
    (imported_type_paths.len() == 1).then(|| imported_type_paths.into_iter().next().unwrap())
}

fn csharp_unique_dispatchable_type_path(
    raw_symbols: &[IndexedSymbol],
    type_path: &str,
    is_target_type: fn(&IndexedSymbol) -> bool,
) -> Option<String> {
    let candidates = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.semantic_path == type_path && csharp_is_type_declaration(candidate)
        })
        .collect::<Vec<_>>();
    (candidates.len() == 1 && is_target_type(candidates[0])).then(|| type_path.to_string())
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
    require_static: bool,
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
                        && if require_static {
                            csharp_method_is_static(candidate)
                        } else {
                            !csharp_method_is_static(candidate)
                        }
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

/// Resolves a type-qualified static method such as `Caller.Make()` or
/// `FixedCaller.Make()` where the method may be declared directly on the
/// qualified type or inherited through its unique class/record ancestor
/// chain. The qualified type must resolve to exactly one class/record
/// declaration; the nearest ancestor declaring the arity-matched static
/// method pins the target, so a static factory inherited through a generic
/// base (`Caller : Derived<Helper>` with `Base<U>::Make`) resolves to the
/// declaring base method. Unknown or ambiguous types and missing, instance,
/// or arity-mismatched methods return `None` and fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# type-qualified static method resolution inputs explicit"
)]
fn resolve_csharp_type_qualified_static_method(
    source_symbol: &IndexedSymbol,
    type_path: &str,
    method_name: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let direct_path = format!("{type_path}::{method_name}");
    if let Some(symbol_id) = resolve_csharp_candidate(
        raw_symbols,
        semantic_path_index,
        &direct_path,
        Some(source_symbol),
        call_arity,
        CSharpCandidateRequirements {
            node_kind: "method_declaration",
            require_static: true,
            require_instance: false,
            require_same_file: false,
        },
    ) {
        return Ok(Some(symbol_id));
    }
    let type_indexes = semantic_path_index
        .get(type_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| csharp_is_base_constructible_type(&raw_symbols[*index]))
        .collect::<Vec<_>>();
    if type_indexes.len() != 1 {
        return Ok(None);
    }
    let type_symbol = &raw_symbols[type_indexes[0]];
    let Some(base_binding) = csharp_base_type_binding_for_type(
        type_symbol,
        raw_symbols,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(target_path) = csharp_base_method_target_path(
        type_symbol,
        raw_symbols,
        semantic_path_index,
        &base_binding,
        method_name,
        call_arity,
        true,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    Ok(resolve_csharp_candidate(
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
    ))
}

fn csharp_is_base_constructible_type(symbol: &IndexedSymbol) -> bool {
    matches!(
        symbol.node_kind.as_str(),
        "class_declaration" | "record_declaration"
    )
}

fn csharp_is_struct_declaration(symbol: &IndexedSymbol) -> bool {
    symbol.node_kind == "struct_declaration"
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

/// Strips generic type-argument lists from a dotted type spelling such as
/// `Outer<HelperA>.Inner<HelperB>` -> `Outer.Inner`, preserving dots and the
/// `global::` prefix, so static-member dispatch on constructed nested types
/// builds the same semantic path as the plain type declaration. Malformed or
/// unbalanced angle lists return `None` and fail closed.
fn csharp_strip_generic_type_argument_lists(reference_name: &str) -> Option<String> {
    let mut normalized = String::with_capacity(reference_name.len());
    let mut depth = 0usize;
    for character in reference_name.chars() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.checked_sub(1)?,
            _ if depth == 0 => normalized.push(character),
            _ => {}
        }
    }
    (depth == 0).then_some(normalized)
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
    let (relative_type_path, first_type_name, method_name) =
        csharp_nested_type_static_reference_parts(reference_name)?;

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
        namespace_path = {
            let current_path = namespace_path?;
            current_path.rsplit_once("::").map(|(parent, _)| parent)
        };
    }
}

fn csharp_nested_type_static_reference_parts(reference_name: &str) -> Option<(String, &str, &str)> {
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
    let mut type_segments = type_path.split('.');
    let first_type_name = type_segments.next()?;
    type_segments.next()?;
    Some((type_path.replace('.', "::"), first_type_name, method_name))
}

fn csharp_nested_type_root_is_unshadowed(
    first_type_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
) -> bool {
    let Some(mut namespace_path) = csharp_source_namespace_path(source_symbol, raw_symbols) else {
        return false;
    };
    loop {
        let root_type_path = namespace_path
            .map(|namespace_path| format!("{namespace_path}::{first_type_name}"))
            .unwrap_or_else(|| first_type_name.to_string());
        if raw_symbols.iter().any(|candidate| {
            candidate.semantic_path == root_type_path && csharp_is_type_declaration(candidate)
        }) {
            return false;
        }
        namespace_path = match namespace_path {
            Some(current_path) => current_path.rsplit_once("::").map(|(parent, _)| parent),
            None => return true,
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
        namespace_path = {
            let current_path = namespace_path?;
            current_path.rsplit_once("::").map(|(parent, _)| parent)
        };
    }
}

fn csharp_namespace_relative_dotted_static_target_path(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
) -> Option<String> {
    let (type_path, method_name) = reference_name.rsplit_once('.')?;
    if type_path.is_empty()
        || !type_path.contains('.')
        || method_name.is_empty()
        || method_name.contains('.')
        || method_name == "this"
        || type_path.starts_with("global::")
        || type_path
            .split('.')
            .any(|segment| !is_safe_csharp_identifier(segment))
    {
        return None;
    }
    let relative_type_path = type_path.replace('.', "::");
    let mut namespace_path = csharp_source_namespace_path(source_symbol, raw_symbols)?;
    loop {
        let target_type_path = namespace_path
            .map(|namespace_path| format!("{namespace_path}::{relative_type_path}"))
            .unwrap_or_else(|| relative_type_path.clone());
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
        namespace_path = {
            let current_path = namespace_path?;
            current_path.rsplit_once("::").map(|(parent, _)| parent)
        };
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# namespace-absolute dotted type resolution inputs explicit"
)]
fn csharp_namespace_absolute_dotted_static_target_path(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((type_path, method_name)) = reference_name.rsplit_once('.') else {
        return Ok(None);
    };
    if type_path.is_empty()
        || !type_path.contains('.')
        || method_name.is_empty()
        || method_name.contains('.')
        || method_name == "this"
        || type_path.starts_with("global::")
        || type_path
            .split('.')
            .any(|segment| !is_safe_csharp_identifier(segment))
    {
        return Ok(None);
    }
    let Some(binding) = resolve_csharp_receiver_type_binding(
        source_symbol,
        type_path,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    if binding.alias_name.is_some() {
        return Ok(None);
    }
    let Some(target_type_path) = csharp_dispatchable_type_path(
        source_symbol,
        raw_symbols,
        &binding,
        csharp_is_type_declaration,
    ) else {
        return Ok(None);
    };
    Ok(Some(format!("{target_type_path}::{method_name}")))
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# namespace-imported dotted type resolution inputs explicit"
)]
fn resolve_csharp_namespace_imported_dotted_type_path(
    source_symbol: &IndexedSymbol,
    type_path: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if type_path.starts_with("global::") || !type_path.contains('.') {
        return Ok(None);
    }
    let Some(semantic_path) = crate::language::csharp_generic_type_semantic_path(type_path) else {
        return Ok(None);
    };
    let Some(first_segment) = semantic_path.split("::").next() else {
        return Ok(None);
    };
    if !csharp_nested_type_root_is_unshadowed(first_segment, source_symbol, raw_symbols) {
        return Ok(None);
    }
    let mut namespace_imports = resolve_csharp_namespace_imports_for_reference(
        &source_symbol.file_path,
        first_segment,
        source_namespace_path,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    if let Some(csharp_global_import_context) = csharp_global_import_context {
        namespace_imports.extend(resolve_csharp_global_namespace_imports_for_reference(
            first_segment,
            csharp_global_import_context,
        ));
    }
    let mut target_type_paths = BTreeSet::new();
    for binding in &namespace_imports {
        let target_type_path = semantic_path.split("::").fold(
            binding.semantic_namespace_path.clone(),
            |mut current, segment| {
                current.push_str("::");
                current.push_str(segment);
                current
            },
        );
        let type_candidates = semantic_path_index
            .get(&target_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .count();
        if type_candidates > 1 {
            return Ok(None);
        }
        if type_candidates == 1 {
            target_type_paths.insert(target_type_path);
        }
    }
    let target_type_paths = target_type_paths.into_iter().collect::<Vec<_>>();
    let [target_type_path] = target_type_paths.as_slice() else {
        return Ok(None);
    };
    Ok(Some(target_type_path.clone()))
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# namespace-imported dotted static target resolution inputs explicit"
)]
fn csharp_namespace_imported_dotted_static_target_path(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((type_path, method_name)) = reference_name.rsplit_once('.') else {
        return Ok(None);
    };
    if type_path.is_empty()
        || !type_path.contains('.')
        || method_name.is_empty()
        || method_name.contains('.')
        || method_name == "this"
        || type_path.starts_with("global::")
        || type_path
            .split('.')
            .any(|segment| !is_safe_csharp_identifier(segment))
    {
        return Ok(None);
    }
    let Some(target_type_path) = resolve_csharp_namespace_imported_dotted_type_path(
        source_symbol,
        type_path,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(format!("{target_type_path}::{method_name}")))
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# alias-rooted dotted type resolution inputs explicit"
)]
fn resolve_csharp_alias_to_dotted_type_path(
    source_symbol: &IndexedSymbol,
    type_path: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if type_path.starts_with("global::") || !type_path.contains('.') {
        return Ok(None);
    }
    let Some((alias_name, rest)) = type_path.split_once('.') else {
        return Ok(None);
    };
    if alias_name.is_empty()
        || rest.is_empty()
        || !is_safe_csharp_identifier(alias_name)
        || !csharp_alias_name_is_unshadowed(alias_name, source_symbol, raw_symbols)
    {
        return Ok(None);
    }
    // The alias-relative spelling may include constructed generic segments
    // such as `Inner<HelperB>` in `OuterAlias.Inner<HelperB>`, so normalize
    // the rest like a plain constructed receiver spelling (strip balanced
    // type-argument lists and join with `::`) to match the indexed type
    // declaration; malformed spellings fail closed.
    let Some(rest_semantic_path) = crate::language::csharp_generic_type_semantic_path(rest) else {
        return Ok(None);
    };
    if rest_semantic_path.is_empty() {
        return Ok(None);
    }
    let binding = match resolve_csharp_type_alias_binding_for_name(
        &source_symbol.file_path,
        alias_name,
        source_namespace_path,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )? {
        Some(binding) => binding,
        None => {
            let Some(csharp_global_import_context) = csharp_global_import_context else {
                return Ok(None);
            };
            if csharp_global_base_type_alias_is_ambiguous(alias_name, csharp_global_import_context)
            {
                return Ok(None);
            }
            let Some(binding) =
                resolve_csharp_global_base_type_alias(alias_name, csharp_global_import_context)
            else {
                return Ok(None);
            };
            binding
        }
    };
    let mut target_type_path = binding.semantic_type_path;
    for segment in rest_semantic_path.split("::") {
        target_type_path.push_str("::");
        target_type_path.push_str(segment);
    }
    let type_candidates = semantic_path_index
        .get(&target_type_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
        .count();
    if type_candidates != 1 {
        return Ok(None);
    }
    Ok(Some(target_type_path))
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# alias-rooted dotted static target resolution inputs explicit"
)]
fn csharp_alias_to_dotted_static_target_path(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((type_path, method_name)) = reference_name.rsplit_once('.') else {
        return Ok(None);
    };
    if type_path.is_empty()
        || !type_path.contains('.')
        || method_name.is_empty()
        || method_name.contains('.')
        || method_name == "this"
        || type_path.starts_with("global::")
    {
        return Ok(None);
    }
    let Some(target_type_path) = resolve_csharp_alias_to_dotted_type_path(
        source_symbol,
        type_path,
        raw_symbols,
        semantic_path_index,
        source_namespace_path,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(format!("{target_type_path}::{method_name}")))
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

fn csharp_is_interface_declaration(symbol: &IndexedSymbol) -> bool {
    symbol.node_kind == "interface_declaration"
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

fn resolve_csharp_namespace_imported_nested_static_method(
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    bindings: &[CSharpNamespaceImportBinding],
    nested_type_path: String,
    method_name: &str,
    call_arity: usize,
) -> Option<String> {
    let mut target_type_paths = BTreeSet::new();
    for binding in bindings {
        let mut current_type_path = binding.semantic_namespace_path.clone();
        let mut type_path_is_present = false;
        for segment in nested_type_path.split("::") {
            current_type_path.push_str("::");
            current_type_path.push_str(segment);
            let type_candidates = semantic_path_index
                .get(&current_type_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                .count();
            if type_candidates > 1 {
                return None;
            }
            if type_candidates == 0 {
                if type_path_is_present {
                    return None;
                }
                break;
            }
            type_path_is_present = true;
        }
        if type_path_is_present {
            target_type_paths.insert(current_type_path);
        }
    }
    let target_type_paths = target_type_paths.into_iter().collect::<Vec<_>>();
    let [target_type_path] = target_type_paths.as_slice() else {
        return None;
    };
    let target_path = format!("{target_type_path}::{method_name}");
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

/// Resolves a dotted static-initializer type prefix whose first segment comes
/// from a namespace import, such as `Outer.Util` with `using Demo;` when the
/// nested type is `Demo.Outer.Util`. Each imported namespace is walked segment
/// by segment like nested static method resolution, and exactly one imported
/// namespace must yield a unique type declaration at every prefix before the
/// fully resolved type path is returned. A first segment that is shadowed by a
/// type in the caller's own namespace chain, an ambiguous nested path, a
/// `global::`-qualified prefix, and unresolvable prefixes return `None` and
/// fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# namespace-imported nested type inputs explicit"
)]
fn resolve_csharp_namespace_imported_nested_type_path(
    source_symbol: &IndexedSymbol,
    type_path: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if type_path.starts_with("global::") {
        return Ok(None);
    }
    let Some(semantic_path) = crate::language::csharp_generic_type_semantic_path(type_path) else {
        return Ok(None);
    };
    if !semantic_path.contains("::") {
        return Ok(None);
    }
    let Some(first_segment) = semantic_path.split("::").next() else {
        return Ok(None);
    };
    if !csharp_nested_type_root_is_unshadowed(first_segment, source_symbol, raw_symbols) {
        return Ok(None);
    }
    let mut namespace_imports = resolve_csharp_namespace_imports_for_reference(
        &source_symbol.file_path,
        first_segment,
        source_namespace_path,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )?;
    if let Some(csharp_global_import_context) = csharp_global_import_context {
        namespace_imports.extend(resolve_csharp_global_namespace_imports_for_reference(
            first_segment,
            csharp_global_import_context,
        ));
    }
    let mut target_type_paths = BTreeSet::new();
    for binding in &namespace_imports {
        let mut current_type_path = binding.semantic_namespace_path.clone();
        let mut type_path_is_present = false;
        for segment in semantic_path.split("::") {
            current_type_path.push_str("::");
            current_type_path.push_str(segment);
            let type_candidates = semantic_path_index
                .get(&current_type_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
                .count();
            if type_candidates > 1 {
                return Ok(None);
            }
            if type_candidates == 0 {
                if type_path_is_present {
                    return Ok(None);
                }
                break;
            }
            type_path_is_present = true;
        }
        if type_path_is_present {
            target_type_paths.insert(current_type_path);
        }
    }
    let target_type_paths = target_type_paths.into_iter().collect::<Vec<_>>();
    let [target_type_path] = target_type_paths.as_slice() else {
        return Ok(None);
    };
    Ok(Some(target_type_path.clone()))
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

#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# alias-rooted nested static method resolution inputs explicit"
)]
fn resolve_csharp_imported_nested_static_method(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    binding: &CSharpTypeAliasBinding,
    nested_type_path: &str,
    method_name: &str,
    call_arity: usize,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let mut type_path = binding.semantic_type_path.clone();
    for segment in std::iter::once("").chain(nested_type_path.split("::")) {
        if !segment.is_empty() {
            type_path.push_str("::");
            type_path.push_str(segment);
        }
        let type_candidates = semantic_path_index
            .get(&type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| csharp_is_type_declaration(&raw_symbols[*index]))
            .count();
        if type_candidates != 1 {
            return Ok(None);
        }
    }
    // An alias-rooted nested static member such as
    // `OuterAlias.Inner<HelperB>.VoidMethod()` dispatches through the
    // nested type's unique class/record ancestor chain, so an inherited
    // member pins the nearest declaring base method like the simple-type
    // branches above.
    resolve_csharp_type_qualified_static_method(
        source_symbol,
        &type_path,
        method_name,
        call_arity,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )
}

/// Resolves an alias-qualified static method such as `Alias.Make()` with
/// `using Alias = Demo.Derived<HelperA>;`. The alias target is resolved
/// directly first; when the target does not declare the method itself, the
/// unique class/record ancestor chain of the alias target is walked, so
/// `Alias.Make()` with `Derived<HelperA> : Base<HelperA>` and
/// `Base<U>::Make` resolves to the declaring base method. Unknown,
/// ambiguous, or non-static methods return `None` and fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps C# alias-qualified static method resolution inputs explicit"
)]
fn resolve_csharp_imported_static_method(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    binding: &CSharpTypeAliasBinding,
    method_name: &str,
    call_arity: usize,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    csharp_import_contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let direct_target_path = format!("{}::{method_name}", binding.semantic_type_path);
    if let Some(symbol_id) = resolve_csharp_candidate(
        raw_symbols,
        semantic_path_index,
        &direct_target_path,
        Some(source_symbol),
        call_arity,
        CSharpCandidateRequirements {
            node_kind: "method_declaration",
            require_static: true,
            require_instance: false,
            require_same_file: false,
        },
    ) {
        return Ok(Some(symbol_id));
    }
    resolve_csharp_type_qualified_static_method(
        source_symbol,
        &binding.semantic_type_path,
        method_name,
        call_arity,
        raw_symbols,
        semantic_path_index,
        csharp_global_import_context,
        file_overrides,
        csharp_import_contexts_by_file,
        deadline,
    )
}

enum JavaInstanceReceiverResolution {
    Resolved(String),
    NoBinding,
    Blocked,
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java instance receiver resolution inputs explicit"
)]
fn resolve_java_instance_receiver_call(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaInstanceReceiverResolution> {
    let Some((raw_receiver_name, member_chain)) = reference_name.split_once('.') else {
        return Ok(JavaInstanceReceiverResolution::NoBinding);
    };
    if raw_receiver_name.is_empty() || member_chain.is_empty() {
        return Ok(JavaInstanceReceiverResolution::NoBinding);
    }
    // A bound receiver may carry an element-access suffix such as `items[0]`
    // in `items[0].helper(...)`; the element access dispatches on the array's
    // element component type, while indexing a non-array receiver is
    // malformed and fails closed.
    let (receiver_name, array_access) = match raw_receiver_name.find('[') {
        Some(open) if raw_receiver_name.ends_with(']') => {
            let base = &raw_receiver_name[..open];
            if base.is_empty() {
                return Ok(JavaInstanceReceiverResolution::Blocked);
            }
            (base, true)
        }
        _ => (raw_receiver_name, false),
    };
    let Some(bindings) = java_receiver_type_bindings_for_function(
        &source_symbol.file_path,
        source_symbol.byte_range,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(JavaInstanceReceiverResolution::NoBinding);
    };
    if !bindings.contains(receiver_name) {
        return Ok(JavaInstanceReceiverResolution::NoBinding);
    }
    // A bound receiver is always an instance expression: receivers without a
    // resolvable declared type fail closed instead of falling through to a
    // same-named static type call. A `var` local whose initializer is a bare
    // factory call such as `var value = makeFoo()` infers its receiver type
    // from the unique factory's declared return type. An array-typed receiver
    // such as `Helper[] items` dispatches only through an element access
    // (`items[0].helper(...)`) on the element component type; a direct member
    // call on the array itself fails closed.
    let array_component = bindings.array_component_for(receiver_name);
    let (type_path, member_chain) = if array_access {
        if let Some(component_type) = array_component {
            let Some(component_path) = resolve_java_receiver_type_path(
                source_symbol,
                &component_type,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(JavaInstanceReceiverResolution::Blocked);
            };
            (component_path, member_chain)
        } else if let Some((function_name, initializer_arity)) =
            bindings.initializer_call_for(receiver_name)
            && let Some(component_path) = java_factory_array_component_type_path(
                source_symbol,
                &function_name,
                initializer_arity,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
        {
            // A `var` local initialized from a factory call such as
            // `var items = makeItems()` or `var items = Util.makeItems()`
            // whose declared return type is a single-level array dispatches
            // an element access on the array's element component type; direct
            // member calls on the array and unknown or non-array-returning
            // factories fail closed.
            (component_path, member_chain)
        } else {
            return Ok(JavaInstanceReceiverResolution::Blocked);
        }
    } else if array_component.is_some() {
        return Ok(JavaInstanceReceiverResolution::Blocked);
    } else if let Some(type_name) = bindings.type_for(receiver_name) {
        let Some(type_path) = resolve_java_receiver_type_path(
            source_symbol,
            &type_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(JavaInstanceReceiverResolution::Blocked);
        };
        (type_path, member_chain)
    } else if let Some((function_name, initializer_arity)) =
        bindings.initializer_call_for(receiver_name)
    {
        let Some(type_path) = resolve_java_initializer_type_path(
            source_symbol,
            &function_name,
            initializer_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(JavaInstanceReceiverResolution::Blocked);
        };
        (type_path, member_chain)
    } else if let Some(field_reference) = bindings.initializer_field_for(receiver_name) {
        // A `var` local whose initializer is a field-access value reference
        // such as `var value = this.helper;`, `var value = helper;`,
        // `var value = Util.STATIC_HELPER;`, or a statically imported field
        // name infers its receiver type from the referenced field's declared
        // type.
        let Some(type_path) = resolve_java_initializer_field_type_path(
            source_symbol,
            &field_reference,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(JavaInstanceReceiverResolution::Blocked);
        };
        (type_path, member_chain)
    } else if let Some((base_reference, base_arity)) =
        bindings.element_access_base_for(receiver_name)
    {
        // A `var` local bound from an element access such as
        // `var first = items[0]` resolves to the base array's element
        // component type; a qualified base such as `var fourth = this.fieldItems[0]`
        // resolves the field chain's terminal array field, and a factory-call
        // base such as `var first = makeItems()[0]` resolves through the same
        // factory rules as other `var` initializers. An unbound or non-array
        // base fails closed.
        let component_path = if let Some(factory_call) = base_reference.strip_suffix("()") {
            java_factory_array_component_type_path(
                source_symbol,
                factory_call,
                base_arity,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
        } else if base_reference.contains('.') {
            java_qualified_element_access_component_type_path(
                source_symbol,
                &base_reference,
                &bindings,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
        } else {
            let Some(component_type) = bindings.array_component_for(&base_reference) else {
                return Ok(JavaInstanceReceiverResolution::Blocked);
            };
            resolve_java_receiver_type_path(
                source_symbol,
                &component_type,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
        };
        let Some(component_path) = component_path else {
            return Ok(JavaInstanceReceiverResolution::Blocked);
        };
        (component_path, member_chain)
    } else {
        return Ok(JavaInstanceReceiverResolution::Blocked);
    };
    // Member chains such as `group.member.helper(...)` resolve each
    // intermediate field's declared type before dispatching the final method;
    // unknown, ambiguous, or unresolvable hops fail closed.
    match resolve_java_member_chain_from_type_path(
        &type_path,
        member_chain,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        call_arity,
        deadline,
    )? {
        Some(symbol_id) => Ok(JavaInstanceReceiverResolution::Resolved(symbol_id)),
        None => Ok(JavaInstanceReceiverResolution::Blocked),
    }
}

/// Resolves the declared type of a field on an owning type path, used to walk
/// member chains such as `group.member.helper(...)`. The field's declared type
/// resolves in the field's own file and enclosing scope so explicit imports in
/// the owning file apply. Unknown, ambiguous, or complex field types fail
/// closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java field type resolution inputs explicit"
)]
fn java_field_type_path(
    owner_type_path: &str,
    field_name: &str,
    require_static: bool,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let candidates = java_field_type_candidates(
        owner_type_path,
        field_name,
        require_static,
        raw_symbols,
        semantic_path_index,
    );
    if candidates.len() != 1 {
        return Ok(None);
    }
    let field = candidates[0];
    let Some(field_type) = field.return_type.as_deref() else {
        return Ok(None);
    };
    let Some(type_name) = java_dotted_type_name(field_type) else {
        return Ok(None);
    };
    resolve_java_receiver_type_path(
        field,
        &type_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

/// Returns the field declarations for `field_name` declared directly on
/// `owner_type_path`, optionally requiring the `static` modifier. Zero
/// entries mean the type does not declare the field; more than one means the
/// field is ambiguous across the indexed workspace.
fn java_field_type_candidates<'a>(
    owner_type_path: &str,
    field_name: &str,
    require_static: bool,
    raw_symbols: &'a [IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Vec<&'a IndexedSymbol> {
    if owner_type_path.is_empty() || field_name.is_empty() {
        return Vec::new();
    }
    semantic_path_index
        .get(&format!("{owner_type_path}::{field_name}"))
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "field_declaration"
                && candidate.scope_path.as_deref() == Some(owner_type_path)
                && (!require_static
                    || candidate
                        .signature
                        .as_deref()
                        .is_some_and(java_field_signature_is_static))
        })
        .map(|index| &raw_symbols[index])
        .collect()
}

/// Extracts the field name from an element-access hop such as `items[0]` or
/// `items[]`; hops without a trailing bracket return `None` so they fall
/// through to ordinary field resolution and fail closed when no such field
/// exists.
fn java_array_access_field_name(hop: &str) -> Option<&str> {
    let open = hop.find('[')?;
    let (base, bracket) = hop.split_at(open);
    if base.is_empty() || !bracket.ends_with(']') {
        return None;
    }
    Some(base)
}

/// Resolves the element component type of an array-typed field hop such as
/// `items[0]` on an owning type path: the field must be uniquely declared
/// (and static when `require_static` is set) and its declared type must be a
/// single-level array whose component resolves in the field's own file and
/// enclosing scope. Unknown, ambiguous, non-array, primitive, or
/// multi-dimensional field types fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java field type resolution inputs explicit"
)]
fn java_array_field_component_type_path(
    owner_type_path: &str,
    field_name: &str,
    require_static: bool,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let candidates = java_field_type_candidates(
        owner_type_path,
        field_name,
        require_static,
        raw_symbols,
        semantic_path_index,
    );
    if candidates.len() != 1 {
        return Ok(None);
    }
    let field = candidates[0];
    let Some(field_type) = field.return_type.as_deref() else {
        return Ok(None);
    };
    let Some(component_name) = java_array_type_component_name(field_type) else {
        return Ok(None);
    };
    resolve_java_receiver_type_path(
        field,
        &component_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

/// Resolves the element component type path of a qualified element-access base
/// such as `this.fieldItems` in `var fourth = this.fieldItems[0]`,
/// `super.inheritedItems` in `var sixth = super.inheritedItems[0]`,
/// `group.holder.fieldItems` in `var fifth = group.holder.fieldItems[0]`, or
/// `Util.fieldItems` in `var seventh = Util.fieldItems[0]`. `this`-rooted
/// bases start on the enclosing type path, `super`-rooted bases on the direct
/// superclass path, other bound receivers on their declared type, and unbound
/// receivers on the named static type (requiring a static terminal field).
/// Intermediate hops resolve through the same inherited-field rules as field
/// chains, and the terminal hop must be a uniquely declared single-level
/// array field whose component resolves in the field's own file and enclosing
/// scope. Unknown, ambiguous, or non-array terminal fields, unbound or
/// non-array receivers, method-call hops, and unresolved static types fail
/// closed.
#[allow(clippy::too_many_arguments)]
fn java_qualified_element_access_component_type_path(
    source_symbol: &IndexedSymbol,
    base_reference: &str,
    bindings: &JavaReceiverTypeBindings,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((receiver, chain)) = base_reference.split_once('.') else {
        return Ok(None);
    };
    if receiver.is_empty() || chain.is_empty() {
        return Ok(None);
    }
    let (initial_type_path, require_static_terminal) = match receiver {
        "this" => {
            let Some(scope_path) = source_symbol.scope_path.as_deref() else {
                return Ok(None);
            };
            (scope_path.to_string(), false)
        }
        "super" => {
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
            (superclass_path, false)
        }
        _ => {
            if let Some(type_name) = bindings.type_for(receiver) {
                let Some(type_path) = resolve_java_receiver_type_path(
                    source_symbol,
                    &type_name,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    java_import_contexts_by_file,
                    deadline,
                )?
                else {
                    return Ok(None);
                };
                (type_path, false)
            } else {
                let Some(type_path) = resolve_java_receiver_type_path(
                    source_symbol,
                    receiver,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    java_import_contexts_by_file,
                    deadline,
                )?
                else {
                    return Ok(None);
                };
                (type_path, true)
            }
        }
    };
    let hops = chain.split('.').collect::<Vec<_>>();
    if hops.is_empty() || hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    let mut current_type_path = initial_type_path;
    for (index, hop) in hops.iter().enumerate() {
        let is_terminal = index + 1 == hops.len();
        let next_path = if is_terminal {
            java_array_field_component_type_path(
                &current_type_path,
                hop,
                require_static_terminal,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
        } else {
            java_inherited_field_type_path(
                &current_type_path,
                hop,
                false,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
        };
        let Some(next_path) = next_path else {
            return Ok(None);
        };
        current_type_path = next_path;
    }
    Ok(Some(current_type_path))
}

/// Dispatches a member chain such as `group.member.helper(...)`,
/// `group.inner().helper(...)`, or `Group().inner().helper(...)` on an
/// already-resolved receiver type path: each intermediate hop must resolve to
/// a uniquely declared field or arity-matched method call whose declared type
/// continues the chain, and the final member must be a unique non-static,
/// non-varargs method with a matching arity. Unknown, ambiguous, or
/// unresolvable hops and missing final members fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java member-chain dispatch inputs explicit"
)]
fn resolve_java_member_chain_from_type_path(
    type_path: &str,
    member_chain: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let mut current_type_path = type_path.to_string();
    let mut hops = member_chain.split('.').collect::<Vec<_>>();
    if hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    if hops.len() > 1 {
        let Some(final_member) = hops.pop() else {
            return Ok(None);
        };
        for hop in hops {
            let next_path =
                if let Some((method_name, hop_arity)) = java_method_call_hop_spelling(hop) {
                    java_method_return_type_path(
                        &current_type_path,
                        &method_name,
                        hop_arity,
                        raw_symbols,
                        semantic_path_index,
                        file_overrides,
                        java_import_contexts_by_file,
                        deadline,
                    )?
                } else if let Some(array_field) = java_array_access_field_name(hop) {
                    java_array_field_component_type_path(
                        &current_type_path,
                        array_field,
                        false,
                        raw_symbols,
                        semantic_path_index,
                        file_overrides,
                        java_import_contexts_by_file,
                        deadline,
                    )?
                } else {
                    java_field_type_path(
                        &current_type_path,
                        hop,
                        false,
                        raw_symbols,
                        semantic_path_index,
                        file_overrides,
                        java_import_contexts_by_file,
                        deadline,
                    )?
                };
            let Some(next_path) = next_path else {
                return Ok(None);
            };
            current_type_path = next_path;
        }
        return resolve_java_instance_receiver_member(
            &current_type_path,
            final_member,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            deadline,
        );
    }
    resolve_java_instance_receiver_member(
        &current_type_path,
        member_chain,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        call_arity,
        deadline,
    )
}

/// Parses a method-call hop spelling such as `inner()`, `inner(0)`, or
/// `inner(2)` into the method name and the call arity recorded by the
/// extractor. Field hops, malformed spellings, and non-numeric argument
/// lists return `None` so they fall through to field resolution and fail
/// closed when no such field exists.
fn java_method_call_hop_spelling(hop: &str) -> Option<(String, usize)> {
    let open = hop.find('(')?;
    let (method_name, arguments) = hop.split_at(open);
    if method_name.is_empty() {
        return None;
    }
    let arguments = arguments.strip_prefix('(')?.strip_suffix(')')?;
    let arity = if arguments.is_empty() {
        0
    } else {
        arguments.parse::<usize>().ok()?
    };
    Some((method_name.to_string(), arity))
}

/// Parses a bare factory-call root with a trailing element-access suffix such
/// as `makeItems()` in `makeItems()[0].helper(...)` into the factory name and
/// its call arity. The root must be exactly one call followed by one bracket
/// pair; multi-dimensional element access such as `makeItems()[0][0]`,
/// malformed brackets, and dotted factory names fail closed.
fn java_array_factory_call_root_spelling(hop: &str) -> Option<(String, usize)> {
    let open = hop.find('(')?;
    let (method_name, rest) = hop.split_at(open);
    if method_name.is_empty() || method_name.contains('.') {
        return None;
    }
    let bracket_open = rest.find('[')?;
    let call_part = &rest[..bracket_open];
    let arguments = call_part.strip_prefix('(')?.strip_suffix(')')?;
    let arity = if arguments.is_empty() {
        0
    } else {
        arguments.parse::<usize>().ok()?
    };
    let bracket = &rest[bracket_open..];
    if !bracket.ends_with(']') || bracket[1..].contains('[') {
        return None;
    }
    Some((method_name.to_string(), arity))
}

/// Resolves the element component type path of a factory call whose declared
/// return type is a single-level array, such as `Helper[] makeItems()`, for a
/// `var` local's initializer callee. Bare callees such as `makeItems` resolve
/// through the same rules as a `var` initializer (a unique same-type method or
/// explicit static-method import with matching non-varargs arity); qualified
/// callees such as `Util.makeItems` resolve through the qualified initializer
/// rules (static type receivers, `this`/`super`-rooted callees, constructed
/// types, and bound-receiver chains). The component resolves in the factory's
/// own file and enclosing scope. Unknown or arity-mismatched factories and
/// primitive or multi-dimensional return arrays fail closed.
#[allow(clippy::too_many_arguments)]
fn java_factory_array_component_type_path(
    source_symbol: &IndexedSymbol,
    initializer_reference: &str,
    initializer_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let factory_path = if initializer_reference.contains('.') {
        resolve_java_qualified_initializer_function_path(
            source_symbol,
            initializer_reference,
            initializer_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
    } else {
        resolve_java_initializer_function_path(
            source_symbol,
            initializer_reference,
            initializer_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
    };
    let Some(factory_path) = factory_path else {
        return Ok(None);
    };
    let Some(factory) = raw_symbols
        .iter()
        .find(|candidate| candidate.symbol_id == factory_path)
    else {
        return Ok(None);
    };
    let Some(return_type) = factory.return_type.as_deref() else {
        return Ok(None);
    };
    let Some(component_name) = java_array_type_component_name(return_type) else {
        return Ok(None);
    };
    resolve_java_receiver_type_path(
        factory,
        &component_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

/// Resolves a bare factory-call root with an element-access suffix such as
/// `makeItems()[0]` in `makeItems()[0].helper(...)`: the leading call resolves
/// through the same factory rules as a `var` initializer (a unique same-type
/// method or explicit static-method import with matching non-varargs arity)
/// whose declared return type is a single-level array, and the trailing member
/// chain dispatches on the array's element component type in the factory's own
/// file and enclosing scope. Unknown or arity-mismatched factories, primitive
/// or multi-dimensional return arrays, and unresolvable hops fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java factory array root resolution inputs explicit"
)]
fn resolve_java_bare_factory_array_member_chain(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((root_spelling, member_chain)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    if root_spelling.is_empty() || member_chain.is_empty() {
        return Ok(None);
    }
    let Some((function_name, function_arity)) =
        java_array_factory_call_root_spelling(root_spelling)
    else {
        return Ok(None);
    };
    let Some(component_path) = java_factory_array_component_type_path(
        source_symbol,
        &function_name,
        function_arity,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    resolve_java_member_chain_from_type_path(
        &component_path,
        member_chain,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        call_arity,
        deadline,
    )
}

/// Resolves the declared return type of an arity-matched non-static
/// method-call hop such as `inner()` or `inner(1)` in
/// `group.inner().helper(...)`. The hop method dispatches like any other
/// instance-receiver member (class, superclass, interface, or class-receiver
/// interface-default), and its declared return
/// type resolves in the method's own file and enclosing scope. Static hops,
/// arity-mismatched hops, and unknown, ambiguous, primitive, or void return
/// types fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java method-call hop type resolution inputs explicit"
)]
fn java_method_return_type_path(
    owner_type_path: &str,
    method_name: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(method_path) = resolve_java_instance_receiver_member(
        owner_type_path,
        method_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        call_arity,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(method) = raw_symbols
        .iter()
        .find(|candidate| candidate.symbol_id == method_path)
    else {
        return Ok(None);
    };
    let Some(return_type) = method
        .return_type
        .as_deref()
        .and_then(java_dotted_type_name)
    else {
        return Ok(None);
    };
    resolve_java_receiver_type_path(
        method,
        &return_type,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

/// Resolves the declared return type of a static method-call hop such as
/// `factory()` or `factory(1)` in `Util.factory().entry`. The hop must be a
/// unique, directly declared, non-varargs static method with the hop arity,
/// and its declared return type resolves in the method's own file and
/// enclosing scope. Unknown, ambiguous, non-static, or arity-mismatched hops
/// and return types without a usable class or interface spelling fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java static method-call hop type resolution inputs explicit"
)]
fn java_static_method_return_type_path(
    owner_type_path: &str,
    method_name: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let target_path = format!("{owner_type_path}::{method_name}");
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
    if candidates.len() != 1 {
        return Ok(None);
    }
    let method = &raw_symbols[candidates[0]];
    let Some(return_type) = method
        .return_type
        .as_deref()
        .and_then(java_dotted_type_name)
    else {
        return Ok(None);
    };
    resolve_java_receiver_type_path(
        method,
        &return_type,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

/// Resolves a dotted direct-call reference whose leading segments name a
/// class or interface and whose first chain hop is a static field or static
/// factory call, such as `Util.STATIC_HELPER.helper(...)`,
/// `Util.STATIC_HELPER.entry.helper(...)`, or `Util.MakeHelper().helper(...)`.
/// Each prefix split resolves as a class or interface through the same
/// same-package, explicit-import, fully-qualified, and nested type rules as
/// other Java receivers, with generic prefixes such as `Box<Integer>`
/// normalizing to the raw base type; the first chain hop must be a uniquely
/// declared static field (walking the direct-superclass chain) or an
/// arity-matched static method call whose declared type continues the chain,
/// and the
/// remaining hops dispatch through the same member-chain rules as bound
/// receivers. Multiple or competing prefix interpretations, unknown or
/// ambiguous types and roots, non-static roots, and unresolvable hops fail
/// closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java direct type-qualified static-root resolution inputs explicit"
)]
fn resolve_java_direct_type_qualified_static_root_member_chain(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let segments = reference_name.split('.').collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.is_empty()) || segments.len() < 3 {
        return Ok(None);
    }
    let mut resolved = BTreeSet::new();
    for split in 1..segments.len() {
        let type_spelling = segments[..split].join(".");
        let chain = segments[split..].join(".");
        let Some((first_hop, remaining_chain)) = chain.split_once('.') else {
            continue;
        };
        if matches!(type_spelling.as_str(), "this" | "super") {
            continue;
        }
        // Generic type prefixes such as `Box<Integer>` normalize to their raw
        // base type before the same type-path rules apply; malformed generic
        // spellings fail closed.
        let Some(type_name) = java_dotted_type_name(&type_spelling) else {
            continue;
        };
        let Some(type_path) = resolve_java_receiver_type_path(
            source_symbol,
            &type_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            continue;
        };
        let root_type_path =
            if let Some((method_name, hop_arity)) = java_method_call_hop_spelling(first_hop) {
                java_static_method_return_type_path(
                    &type_path,
                    &method_name,
                    hop_arity,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    java_import_contexts_by_file,
                    deadline,
                )?
            } else {
                java_inherited_field_type_path(
                    &type_path,
                    first_hop,
                    true,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    java_import_contexts_by_file,
                    deadline,
                )?
            };
        let Some(root_type_path) = root_type_path else {
            continue;
        };
        if let Some(symbol_id) = resolve_java_member_chain_from_type_path(
            &root_type_path,
            remaining_chain,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            deadline,
        )? {
            resolved.insert(symbol_id);
        }
    }
    if resolved.len() != 1 {
        return Ok(None);
    }
    Ok(resolved.into_iter().next())
}

/// Dispatches the final member of an instance-receiver chain. The receiver's
/// class type resolves through the superclass chain as usual; when the
/// receiver is declared with an interface type and the interface itself does
/// not declare the method, a uniquely resolved direct super-interface chain
/// (abstract or default declarations) resolves it; and when the receiver is a
/// class that does not declare the method, a `default` method through uniquely
/// resolved direct interfaces resolves it. Multiple, unresolved, or ambiguous
/// interface branches and competing declarations fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java instance receiver member dispatch inputs explicit"
)]
fn resolve_java_instance_receiver_member(
    receiver_type_path: &str,
    method_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if let Some(symbol_id) = resolve_java_inherited_method_from_type_path(
        receiver_type_path,
        method_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        call_arity,
        true,
        deadline,
    )? {
        return Ok(Some(symbol_id));
    }
    let receiver_type_is_interface = semantic_path_index
        .get(receiver_type_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| raw_symbols[*index].node_kind == "interface_declaration")
        .count()
        == 1;
    if receiver_type_is_interface {
        match resolve_java_interface_chain_method_from_type_path(
            receiver_type_path,
            method_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            false,
            deadline,
        )? {
            JavaInterfaceChainMethodResolution::Resolved(symbol_id) => return Ok(Some(symbol_id)),
            JavaInterfaceChainMethodResolution::NoMethod
            | JavaInterfaceChainMethodResolution::Blocked => {}
        }
    }
    resolve_java_class_receiver_interface_default_method(
        receiver_type_path,
        method_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        call_arity,
        deadline,
    )
}

/// Dispatches a `default` method for a class-typed instance receiver whose
/// class and direct superclass chain do not declare the method. The receiver's
/// direct interfaces resolve in its own file and enclosing scope; exactly one
/// direct-interface chain must provide a uniquely arity-matched non-static
/// `default` method and every other chain must prove it has no declaration.
/// Any same-name method declared in the receiver class hierarchy, competing
/// or unresolved interface chains, and ambiguous chains fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java class receiver interface dispatch inputs explicit"
)]
fn resolve_java_class_receiver_interface_default_method(
    receiver_class_path: &str,
    method_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let class_candidates = semantic_path_index
        .get(receiver_class_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| raw_symbols[*index].node_kind == "class_declaration")
        .collect::<Vec<_>>();
    let [class_index] = class_candidates.as_slice() else {
        return Ok(None);
    };
    let receiver_class = &raw_symbols[*class_index];
    if java_class_hierarchy_defines_method_from_type_path(
        receiver_class_path,
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
    let path = Path::new(&receiver_class.file_path);
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
            deadline.check("locating Java receiver interfaces")?;
        }
        if node.kind() == "class_declaration"
            && (node.start_byte(), node.end_byte()) == receiver_class.byte_range
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
    let mut resolved_symbol_id = None;
    for interface_reference in interface_references {
        let Some(interface_path) = resolve_java_direct_interface_target_path(
            &receiver_class.file_path,
            receiver_class.scope_path.as_deref(),
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
        match resolve_java_interface_chain_method_from_type_path(
            &interface_path,
            method_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            true,
            deadline,
        )? {
            JavaInterfaceChainMethodResolution::Resolved(symbol_id) => {
                if resolved_symbol_id.replace(symbol_id).is_some() {
                    return Ok(None);
                }
            }
            JavaInterfaceChainMethodResolution::NoMethod => {}
            JavaInterfaceChainMethodResolution::Blocked => return Ok(None),
        }
    }
    Ok(resolved_symbol_id)
}

/// Resolves a `var` local's bare method-call initializer such as `makeFoo` in
/// `var value = makeFoo()` to a receiver type path. The factory must be a
/// unique same-file same-type method or unique explicit static-method import
/// with a declared return type matching the call arity; the return type
/// resolves in the factory's own file and package scope. Unknown, ambiguous,
/// arity-mismatched, and undeclared-return factories fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java initializer type resolution inputs explicit"
)]
fn resolve_java_initializer_type_path(
    source_symbol: &IndexedSymbol,
    function_name: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let factory_path = if function_name.contains('.') {
        resolve_java_qualified_initializer_function_path(
            source_symbol,
            function_name,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
    } else {
        resolve_java_initializer_function_path(
            source_symbol,
            function_name,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
    };
    let Some(factory_path) = factory_path else {
        return Ok(None);
    };
    let Some(factory) = raw_symbols
        .iter()
        .find(|candidate| candidate.symbol_id == factory_path)
    else {
        return Ok(None);
    };
    let Some(function_return_type) = factory
        .return_type
        .as_deref()
        .and_then(java_dotted_type_name)
    else {
        return Ok(None);
    };
    resolve_java_receiver_type_path(
        factory,
        &function_return_type,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

/// Resolves a `var` local's factory callee such as `makeFoo` in
/// `var value = makeFoo()` to a unique factory method symbol path. Same-file
/// same-type methods with a declared return type and matching non-varargs
/// arity shadow static-method imports; otherwise a unique explicit static
/// method import is eligible. A single-level array return type is eligible
/// the same way so factory-returned array receivers can dispatch on the
/// element component type. Unknown or ambiguous callees fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java initializer function resolution inputs explicit"
)]
fn resolve_java_initializer_function_path(
    source_symbol: &IndexedSymbol,
    function_name: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let same_type_candidates = source_symbol
        .scope_path
        .as_deref()
        .map(|scope_path| {
            let target_path = format!("{scope_path}::{function_name}");
            semantic_path_index
                .get(&target_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.file_path == source_symbol.file_path
                        && candidate.node_kind == "method_declaration"
                        && candidate.return_type.as_deref().is_some_and(|return_type| {
                            java_dotted_type_name(return_type).is_some()
                                || java_array_type_component_name(return_type).is_some()
                        })
                        && candidate.parameters.len() == call_arity
                        && !candidate
                            .parameters
                            .iter()
                            .any(|parameter| parameter.contains("..."))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !same_type_candidates.is_empty() {
        return Ok((same_type_candidates.len() == 1)
            .then(|| raw_symbols[same_type_candidates[0]].symbol_id.clone()));
    }
    let Some(binding) = resolve_java_static_method_import_binding_for_reference(
        &source_symbol.file_path,
        function_name,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    Ok(resolve_java_imported_static_method(
        raw_symbols,
        semantic_path_index,
        &binding,
        function_name,
        call_arity,
    ))
}

/// Resolves a `var` local's qualified method-call initializer callee such as
/// `group.makeFoo` in `var value = group.makeFoo()` to a unique method symbol
/// path. `this.`-rooted and `super.`-rooted callees resolve on the enclosing
/// or direct-superclass type path, constructor-rooted chains such as
/// `new Group().makeFoo` dispatch through the constructed type path, and
/// bound-receiver chains resolve each hop through the member-chain rules.
/// Receivers that are themselves factory-inferred `var` bindings, unbound or
/// static type receivers, and unknown or ambiguous callees fail closed so
/// initializer resolution stays acyclic and conservative.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java qualified initializer resolution inputs explicit"
)]
fn resolve_java_qualified_initializer_function_path(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let normalized = reference_name
        .strip_prefix("new ")
        .unwrap_or(reference_name);
    if let Some(chain) = normalized.strip_prefix("this.") {
        if chain.is_empty() {
            return Ok(None);
        }
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        return resolve_java_member_chain_from_type_path(
            scope_path,
            chain,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            deadline,
        );
    }
    if let Some(chain) = normalized.strip_prefix("super.") {
        if chain.is_empty() {
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
        return resolve_java_member_chain_from_type_path(
            &superclass_path,
            chain,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            deadline,
        );
    }
    if let Some(symbol_id) = resolve_java_constructor_receiver_call(
        source_symbol,
        normalized,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        call_arity,
        deadline,
    )? {
        return Ok(Some(symbol_id));
    }
    let Some((receiver_name, member_chain)) = normalized.split_once('.') else {
        return Ok(None);
    };
    if receiver_name.is_empty() || member_chain.is_empty() {
        return Ok(None);
    }
    let Some(bindings) = java_receiver_type_bindings_for_function(
        &source_symbol.file_path,
        source_symbol.byte_range,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?
    else {
        // No receiver bindings are available; fall through to the static
        // type receiver interpretation below.
        return resolve_java_static_type_initializer_function(
            source_symbol,
            normalized,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        );
    };
    // A bound receiver shadows any same-named type. Only receivers with an
    // explicit or constructor-inferred type are eligible; factory-inferred
    // `var` receivers would require recursive initializer resolution and fail
    // closed instead of falling through to a static type interpretation.
    if bindings.contains(receiver_name) {
        let Some(type_name) = bindings.type_for(receiver_name) else {
            return Ok(None);
        };
        let Some(type_path) = resolve_java_receiver_type_path(
            source_symbol,
            &type_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return resolve_java_member_chain_from_type_path(
            &type_path,
            member_chain,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            deadline,
        );
    }
    resolve_java_static_type_initializer_function(
        source_symbol,
        normalized,
        call_arity,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

/// Resolves a `var` local's qualified factory initializer whose receiver is a
/// static type, such as `var value = Util.make()` or
/// `var value = Util.Nested.nestedMake()`, to a unique directly declared
/// static method symbol path on that type. The reference splits at the last
/// dot so the leading segments name the type (including nested types); the
/// type resolves through the same same-package, explicit-import,
/// exact-qualified, and nested type rules as other Java receivers, and the
/// method must be a unique, directly declared, non-varargs static method with
/// the call arity. Unknown or ambiguous types and methods fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java static type initializer resolution inputs explicit"
)]
fn resolve_java_static_type_initializer_function(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((type_reference, method_name)) = reference_name.rsplit_once('.') else {
        return Ok(None);
    };
    if type_reference.is_empty() || method_name.is_empty() {
        return Ok(None);
    }
    let Some(type_path) = resolve_java_receiver_type_path(
        source_symbol,
        type_reference,
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

/// Splits a constructor-rooted field chain such as `new Holder().group.entry`
/// into the constructed type name (generic arguments stripped) and the
/// remaining member chain after the constructor call. Non-constructor roots,
/// malformed spellings, and unbalanced argument lists return `None`.
fn java_constructor_rooted_field_chain(reference_name: &str) -> Option<(String, String)> {
    let rest = reference_name.strip_prefix("new")?.trim_start();
    if rest.is_empty() || rest.starts_with('.') {
        return None;
    }
    let open = rest.find('(')?;
    let type_name = rest[..open].split('<').next().unwrap_or_default().trim();
    if type_name.is_empty() || type_name.contains(['>', '[', ']', '(', ')', '?', ' ']) {
        return None;
    }
    let mut depth = 0usize;
    let mut close = None;
    for (offset, byte) in rest.bytes().enumerate().skip(open) {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    close = Some(offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let remainder = rest[close + 1..].trim_start();
    let remaining = remainder.strip_prefix('.').unwrap_or(remainder).trim();
    if remaining.is_empty() {
        return None;
    }
    Some((type_name.to_string(), remaining.to_string()))
}

/// Resolves a `var` local's field-access initializer reference to the
/// referenced field's declared type path. `this.`-rooted and `super.`-rooted
/// references resolve the field chain on the enclosing or direct-superclass
/// type path, bare names resolve through the enclosing class field bindings or
/// a unique explicit static field import, qualified names such as
/// `Util.STATIC_FIELD` resolve a static field on the named type, bare names
/// and bare field chains also resolve fields inherited from a unique
/// direct-superclass chain, constructor-rooted chains such as
/// `new Holder().group.entry` resolve on the constructed class type, bound
/// receivers (parameters, declared locals, or enclosing-class fields) with a
/// usable declared type resolve field chains such as `local.entry` on that
/// type, and arity-matched method-call hops such as `makeFoo().entry` or
/// `makeFoo(1).entry` resolve the hop's declared return type through the same
/// factory rules as a `var` initializer before walking the remaining chain.
/// Unknown or ambiguous fields, fields without a usable declared type, bound
/// receivers without a usable declared type, and bound-name shadowing of
/// qualified type receivers fail closed so field-initializer inference stays
/// conservative and acyclic.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java field initializer resolution inputs explicit"
)]
fn resolve_java_initializer_field_type_path(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if let Some(chain) = reference_name.strip_prefix("this.") {
        if chain.is_empty() {
            return Ok(None);
        }
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        return resolve_java_field_chain_type_path(
            scope_path,
            chain,
            false,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        );
    }
    if let Some(chain) = reference_name.strip_prefix("super.") {
        if chain.is_empty() {
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
        return resolve_java_field_chain_type_path(
            &superclass_path,
            chain,
            false,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        );
    }
    // A constructor-rooted field chain such as `new Holder().group.entry`
    // resolves the constructed class type (generic arguments stripped) and
    // walks the remaining chain through the same field-chain rules; missing,
    // unknown, or non-class constructed types fail closed.
    if let Some((type_name, chain)) = java_constructor_rooted_field_chain(reference_name) {
        let type_reference = if type_name.contains('.') {
            JavaDirectSuperclassReference::Qualified(type_name)
        } else {
            JavaDirectSuperclassReference::Simple(type_name)
        };
        let Some(type_path) = resolve_java_direct_type_target_path(
            &source_symbol.file_path,
            source_symbol.scope_path.as_deref(),
            &type_reference,
            "class_declaration",
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return resolve_java_field_chain_type_path(
            &type_path,
            &chain,
            false,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        );
    }
    let Some(bindings) = java_receiver_type_bindings_for_function(
        &source_symbol.file_path,
        source_symbol.byte_range,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    if !reference_name.contains('.') {
        // A bound name is an enclosing-class field or a declared local or
        // parameter; either way its declared type pins the `var` receiver.
        // Factory-inferred `var` receivers and ambiguous bindings have no
        // declared type and fail closed instead of recursing.
        if let Some(type_name) = bindings.type_for(reference_name) {
            return resolve_java_receiver_type_path(
                source_symbol,
                &type_name,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            );
        }
        if bindings.contains(reference_name) {
            return Ok(None);
        }
        // Enclosing-type fields are visible to member functions even when
        // declared on a direct-superclass chain; the binding collector only
        // records directly declared fields, so resolve the bare name on the
        // enclosing type path before falling back to static imports.
        if let Some(scope_path) = source_symbol.scope_path.as_deref()
            && let Some(type_path) = java_inherited_field_type_path(
                scope_path,
                reference_name,
                false,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
        {
            return Ok(Some(type_path));
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
        return resolve_java_imported_static_field_type_path(
            &binding,
            reference_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        );
    }
    let segments = reference_name.split('.').collect::<Vec<_>>();
    if segments[0].is_empty() {
        return Ok(None);
    }
    let mut resolved = BTreeSet::new();
    // A dotted reference whose leading segment is a locally bound value
    // (formal parameter, declared local, or enclosing-class field) is a field
    // chain such as `local.entry` on that value's declared type; the bound
    // value shadows any same-named type, so no qualified `Type.field`
    // interpretation is attempted. Bound values without a usable declared
    // type (factory-inferred or ambiguous `var` receivers) fail closed
    // instead of recursing.
    if bindings.contains(segments[0]) {
        let Some(type_name) = bindings.type_for(segments[0]) else {
            return Ok(None);
        };
        if let Some(type_path) = resolve_java_receiver_type_path(
            source_symbol,
            &type_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )? && let Some(chain_type_path) = resolve_java_field_chain_type_path(
            &type_path,
            &segments[1..].join("."),
            false,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )? {
            resolved.insert(chain_type_path);
        }
        return if resolved.len() == 1 {
            Ok(resolved.into_iter().next())
        } else {
            Ok(None)
        };
    }
    // Qualified `Type.field` references resolve the type from progressively
    // longer prefixes so nested types such as `Outer.Inner.STATIC` work, and
    // require the first field hop to be static on that type. Competing
    // resolutions across prefixes fail closed.
    // A dotted reference whose leading segment names an enclosing-class or
    // inherited field is a bare field chain such as `holder.entry` and
    // resolves on the enclosing type path through the same field-chain rules;
    // it competes with qualified `Type.field` interpretations below.
    if let Some(scope_path) = source_symbol.scope_path.as_deref()
        && let Some(field_type_path) = java_inherited_field_type_path(
            scope_path,
            segments[0],
            false,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        && let Some(chain_type_path) = resolve_java_field_chain_type_path(
            &field_type_path,
            &segments[1..].join("."),
            false,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
    {
        resolved.insert(chain_type_path);
    }
    // A dotted reference whose leading segment names a uniquely explicit
    // static field import such as `import static com.example.Util.STATIC_HELPER`
    // is a field chain such as `STATIC_HELPER.entry` on the imported field's
    // declared type; it competes with the bare-field and qualified-type
    // interpretations below.
    if let Some(binding) = resolve_java_static_method_import_binding_for_reference(
        &source_symbol.file_path,
        segments[0],
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )? && let Some(field_type_path) = resolve_java_imported_static_field_type_path(
        &binding,
        segments[0],
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )? && let Some(chain_type_path) = resolve_java_field_chain_type_path(
        &field_type_path,
        &segments[1..].join("."),
        false,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )? {
        resolved.insert(chain_type_path);
    }
    // A dotted reference whose leading segment is a method-call hop such as
    // `makeFoo()` or `makeFoo(1)` in `makeFoo().entry` resolves the hop's
    // declared return type through the same factory rules as a `var`
    // initializer (a unique same-type method, unique explicit static-method
    // import, static type factory, or bound-receiver factory with matching
    // non-varargs arity) and walks the remaining chain through the same
    // field-chain rules. Unknown or ambiguous callees, arity mismatches, and
    // hops whose return type is not a usable class or interface spelling fail
    // closed.
    if let Some((method_name, hop_arity)) = java_method_call_hop_spelling(segments[0])
        && !method_name.is_empty()
        && let Some(method_type_path) = resolve_java_initializer_type_path(
            source_symbol,
            &method_name,
            hop_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        && let Some(chain_type_path) = resolve_java_field_chain_type_path(
            &method_type_path,
            &segments[1..].join("."),
            false,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
    {
        resolved.insert(chain_type_path);
    }
    for split in 1..segments.len() {
        let type_name = segments[..split].join(".");
        let field_chain = segments[split..].join(".");
        if type_name.is_empty() || field_chain.is_empty() {
            continue;
        }
        let Some(type_path) = resolve_java_receiver_type_path(
            source_symbol,
            &type_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            continue;
        };
        let Some(field_type_path) = resolve_java_field_chain_type_path(
            &type_path,
            &field_chain,
            true,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            continue;
        };
        resolved.insert(field_type_path);
    }
    if resolved.len() == 1 {
        Ok(resolved.into_iter().next())
    } else {
        Ok(None)
    }
}

/// Walks a field-access chain such as `holder.entry` on an already-resolved
/// type path, resolving each hop's declared type in the declaring field's own
/// file and enclosing scope; arity-matched method-call hops such as `inner()`
/// or `inner(1)` resolve through the same method-return-type rules as member
/// chains, while a static method-call first hop such as `factory()` in
/// `Util.factory().entry` resolves through the directly declared static
/// method's return type. `require_first_static` requires the first hop to be
/// a static field or static method call for `Type.field` references; unknown,
/// ambiguous, or unresolvable hops, arity-mismatched hops, non-static first
/// hops, and non-static method-call first hops fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java field chain resolution inputs explicit"
)]
fn resolve_java_field_chain_type_path(
    initial_type_path: &str,
    chain: &str,
    require_first_static: bool,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let hops = chain.split('.').collect::<Vec<_>>();
    if hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    let mut current_type_path = initial_type_path.to_string();
    for (index, hop) in hops.iter().enumerate() {
        let require_static = index == 0 && require_first_static;
        let next_path = if let Some((method_name, hop_arity)) = java_method_call_hop_spelling(hop) {
            if require_static {
                java_static_method_return_type_path(
                    &current_type_path,
                    &method_name,
                    hop_arity,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    java_import_contexts_by_file,
                    deadline,
                )?
            } else {
                java_method_return_type_path(
                    &current_type_path,
                    &method_name,
                    hop_arity,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    java_import_contexts_by_file,
                    deadline,
                )?
            }
        } else {
            java_inherited_field_type_path(
                &current_type_path,
                hop,
                require_static,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            )?
        };
        let Some(next_path) = next_path else {
            return Ok(None);
        };
        current_type_path = next_path;
    }
    Ok(Some(current_type_path))
}

/// Resolves a field hop's declared type path on a type, walking the
/// direct-superclass chain when the field is not declared on the type itself,
/// mirroring Java field inheritance (`Child.holder` and `this.holder` see
/// fields declared on ancestors). The hop must resolve to exactly one field
/// declaration on the first type in the chain that declares it; a field that
/// is ambiguous on any visited type, a type without a resolvable unique class
/// declaration, or an unresolvable superclass chain fails closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java inherited field resolution inputs explicit"
)]
fn java_inherited_field_type_path(
    owner_type_path: &str,
    field_name: &str,
    require_static: bool,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let mut current_type_path = owner_type_path.to_string();
    let mut visited = BTreeSet::new();
    loop {
        let candidates = java_field_type_candidates(
            &current_type_path,
            field_name,
            require_static,
            raw_symbols,
            semantic_path_index,
        );
        if !candidates.is_empty() {
            if candidates.len() != 1 {
                return Ok(None);
            }
            let field = candidates[0];
            let Some(field_type) = field.return_type.as_deref() else {
                return Ok(None);
            };
            let Some(type_name) = java_dotted_type_name(field_type) else {
                return Ok(None);
            };
            return resolve_java_receiver_type_path(
                field,
                &type_name,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                java_import_contexts_by_file,
                deadline,
            );
        }
        if !visited.insert(current_type_path.clone()) {
            return Ok(None);
        }
        let Some(superclass_path) = java_superclass_path_for_type_path(
            &current_type_path,
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

/// Resolves the direct-superclass type path of the unique class declaration
/// for `type_path`, or `None` when the type has no class declaration, multiple
/// class declarations share the path, or the class has no resolvable
/// superclass.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java superclass lookup inputs explicit"
)]
fn java_superclass_path_for_type_path(
    type_path: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let class_indices = semantic_path_index
        .get(type_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| raw_symbols[*index].node_kind == "class_declaration")
        .collect::<Vec<_>>();
    if class_indices.len() != 1 {
        return Ok(None);
    }
    java_simple_superclass_path_for_class(
        &raw_symbols[class_indices[0]],
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

/// Resolves a statically imported field's declared type path from an import
/// binding such as `import static com.example.Util.STATIC_HELPER;`. The field
/// must be a unique static field declaration in the binding's source file, and
/// its declared type resolves in that file's scope. Missing or ambiguous
/// fields fail closed.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java imported static field resolution inputs explicit"
)]
fn resolve_java_imported_static_field_type_path(
    binding: &JavaImportBinding,
    field_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let target_path = format!("{}::{field_name}", binding.semantic_path);
    let candidates = semantic_path_index
        .get(&target_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.file_path == binding.source_path
                && candidate.node_kind == "field_declaration"
                && candidate
                    .signature
                    .as_deref()
                    .is_some_and(java_field_signature_is_static)
        })
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return Ok(None);
    }
    let field = &raw_symbols[candidates[0]];
    let Some(field_type) = field.return_type.as_deref() else {
        return Ok(None);
    };
    let Some(type_name) = java_dotted_type_name(field_type) else {
        return Ok(None);
    };
    resolve_java_receiver_type_path(
        field,
        &type_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )
}

fn java_field_signature_is_static(signature: &str) -> bool {
    signature.split_whitespace().any(|token| token == "static")
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java receiver type resolution inputs explicit"
)]
fn resolve_java_receiver_type_path(
    source_symbol: &IndexedSymbol,
    type_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let reference = if type_name.contains('.') {
        JavaDirectSuperclassReference::Qualified(type_name.to_string())
    } else {
        JavaDirectSuperclassReference::Simple(type_name.to_string())
    };
    // A declared receiver type may name a class or an interface. When the same
    // name resolves to both, the binding is ambiguous and fails closed.
    let class_path = resolve_java_direct_type_target_path(
        &source_symbol.file_path,
        source_symbol.scope_path.as_deref(),
        &reference,
        "class_declaration",
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?;
    let interface_path = resolve_java_direct_type_target_path(
        &source_symbol.file_path,
        source_symbol.scope_path.as_deref(),
        &reference,
        "interface_declaration",
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?;
    match (class_path, interface_path) {
        (Some(_), Some(_)) => Ok(None),
        (Some(class_path), None) => Ok(Some(class_path)),
        (None, Some(interface_path)) => Ok(Some(interface_path)),
        (None, None) => Ok(None),
    }
}

/// Resolves a constructor-call receiver such as `new Foo().helper(...)`, which
/// the extractor records as `Foo().helper`, or `Outer.Inner().helper` for a
/// nested constructed type. The constructed type path resolves through the same
/// constructible-class rules as typed receivers, then dispatches the member as
/// an instance call. Anonymous-class bodies and malformed spellings produce no
/// fact in the extractor; member chains through additional hops and unresolved
/// or ambiguous constructed types fail closed here.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java constructor receiver resolution inputs explicit"
)]
fn resolve_java_constructor_receiver_call(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let segments = reference_name.split('.').collect::<Vec<_>>();
    let Some((marker_index, marker_base)) = segments
        .iter()
        .enumerate()
        .find_map(|(index, segment)| segment.strip_suffix("()").map(|base| (index, base)))
    else {
        return Ok(None);
    };
    let mut type_segments = segments[..marker_index].to_vec();
    type_segments.push(marker_base);
    if type_segments.is_empty()
        || type_segments.iter().any(|segment| {
            segment.is_empty() || segment.contains(['<', '>', '[', ']', '(', ')', '?', ' '])
        })
    {
        return Ok(None);
    }
    let member_chain = segments[marker_index + 1..].join(".");
    if member_chain.is_empty() {
        return Ok(None);
    }
    let type_name = type_segments.join(".");
    let reference = if type_segments.len() > 1 {
        JavaDirectSuperclassReference::Qualified(type_name)
    } else {
        JavaDirectSuperclassReference::Simple(type_name)
    };
    let Some(type_path) = resolve_java_direct_type_target_path(
        &source_symbol.file_path,
        source_symbol.scope_path.as_deref(),
        &reference,
        "class_declaration",
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    resolve_java_member_chain_from_type_path(
        &type_path,
        &member_chain,
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
        false,
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
    require_instance: bool,
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
                    && (!require_instance
                        || !candidate
                            .signature
                            .as_deref()
                            .is_some_and(java_method_signature_is_static))
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

enum JavaInterfaceChainMethodResolution {
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
        match resolve_java_interface_chain_method_from_type_path(
            &interface_path,
            method_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            true,
            deadline,
        )? {
            JavaInterfaceChainMethodResolution::Resolved(symbol_id) => {
                if resolved_symbol_id.replace(symbol_id).is_some() {
                    return Ok(None);
                }
            }
            JavaInterfaceChainMethodResolution::NoMethod => {}
            JavaInterfaceChainMethodResolution::Blocked => return Ok(None),
        }
    }
    Ok(resolved_symbol_id)
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java interface inheritance resolution inputs explicit"
)]
fn resolve_java_interface_chain_method_from_type_path(
    initial_interface_path: &str,
    method_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    require_default: bool,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaInterfaceChainMethodResolution> {
    let mut visited_interface_paths = BTreeSet::new();
    resolve_java_interface_chain_method_from_interface(
        initial_interface_path,
        method_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        java_import_contexts_by_file,
        call_arity,
        require_default,
        deadline,
        &mut visited_interface_paths,
    )
}

/// Walks an interface's direct super-interface branches recursively to resolve
/// `method_name`. Exactly one branch must provide a uniquely arity-matched
/// non-static method meeting `require_default`, and every other branch must
/// prove it has no declaration; a declaration reached identically through
/// multiple branches still resolves once. Competing, ambiguous, cyclic, and
/// unresolvable branches fail closed as `Blocked`.
#[allow(
    clippy::too_many_arguments,
    reason = "keeps Java interface inheritance resolution inputs explicit"
)]
fn resolve_java_interface_chain_method_from_interface(
    interface_path: &str,
    method_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    java_import_contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    call_arity: usize,
    require_default: bool,
    deadline: Option<&WorkspaceScanDeadline>,
    visited_interface_paths: &mut BTreeSet<String>,
) -> Result<JavaInterfaceChainMethodResolution> {
    if let Some(deadline) = deadline {
        deadline.check("resolving Java interface chain method")?;
    }
    if !visited_interface_paths.insert(interface_path.to_string()) {
        return Ok(JavaInterfaceChainMethodResolution::Blocked);
    }
    let target_path = format!("{interface_path}::{method_name}");
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
                (!require_default
                    || candidate
                        .signature
                        .as_deref()
                        .is_some_and(java_method_signature_is_default))
                    && !candidate
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
        let resolution = match candidates.as_slice() {
            [candidate_index] => JavaInterfaceChainMethodResolution::Resolved(
                raw_symbols[*candidate_index].symbol_id.clone(),
            ),
            _ => JavaInterfaceChainMethodResolution::Blocked,
        };
        visited_interface_paths.remove(interface_path);
        return Ok(resolution);
    }
    let interface_candidates = semantic_path_index
        .get(interface_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| raw_symbols[*index].node_kind == "interface_declaration")
        .collect::<Vec<_>>();
    let [interface_index] = interface_candidates.as_slice() else {
        visited_interface_paths.remove(interface_path);
        return Ok(JavaInterfaceChainMethodResolution::Blocked);
    };
    let source_interface = &raw_symbols[*interface_index];
    let Some(parent_references) =
        java_direct_interface_parent_references(source_interface, file_overrides, deadline)?
    else {
        visited_interface_paths.remove(interface_path);
        return Ok(JavaInterfaceChainMethodResolution::NoMethod);
    };
    let mut resolved_symbol_id = None;
    for parent_reference in parent_references {
        let Some(parent_interface_path) = resolve_java_direct_interface_target_path(
            &source_interface.file_path,
            source_interface.scope_path.as_deref(),
            &parent_reference,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            deadline,
        )?
        else {
            visited_interface_paths.remove(interface_path);
            return Ok(JavaInterfaceChainMethodResolution::Blocked);
        };
        match resolve_java_interface_chain_method_from_interface(
            &parent_interface_path,
            method_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            java_import_contexts_by_file,
            call_arity,
            require_default,
            deadline,
            visited_interface_paths,
        )? {
            JavaInterfaceChainMethodResolution::Resolved(symbol_id) => {
                if resolved_symbol_id
                    .as_deref()
                    .is_some_and(|resolved| resolved != symbol_id)
                {
                    visited_interface_paths.remove(interface_path);
                    return Ok(JavaInterfaceChainMethodResolution::Blocked);
                }
                resolved_symbol_id.get_or_insert(symbol_id);
            }
            JavaInterfaceChainMethodResolution::Blocked => {
                visited_interface_paths.remove(interface_path);
                return Ok(JavaInterfaceChainMethodResolution::Blocked);
            }
            JavaInterfaceChainMethodResolution::NoMethod => {}
        }
    }
    visited_interface_paths.remove(interface_path);
    Ok(resolved_symbol_id
        .map(JavaInterfaceChainMethodResolution::Resolved)
        .unwrap_or(JavaInterfaceChainMethodResolution::NoMethod))
}

/// Returns the directly extended interface references of an interface
/// declaration, or `None` when the declaration has no `extends` clause.
fn java_direct_interface_parent_references(
    source_interface: &IndexedSymbol,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<Vec<JavaDirectSuperclassReference>>> {
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
            return java_direct_interface_references_for_declaration(node, &source);
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

#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_reference_with_deadline(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    call_context: CallResolutionContext,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(call_arity) = call_context.arity else {
        return Ok(None);
    };
    if reference_name.is_empty() {
        return Ok(None);
    }
    // A `this`-rooted receiver such as `this.entry.helper(...)` dispatches on
    // the enclosing type path through the same member-chain rules as bound
    // receivers; a `super`-rooted receiver such as `super.entry.helper(...)`
    // or `super.baseHelper(...)` dispatches on the direct superclass path.
    // Unknown or unresolvable roots and hops fail closed instead of falling
    // through to static type calls, and callers outside a type (top-level
    // functions, extension functions) fail closed because `this`/`super` have
    // no enclosing type to dispatch on.
    if let Some(chain) = reference_name.strip_prefix("this.") {
        if chain.is_empty() {
            return Ok(None);
        }
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        // `this` inside a companion member refers to the companion object, so
        // a companion scope such as `Type::Companion` is accepted alongside
        // declared type scopes; package-level and extension-function scopes
        // fail closed.
        let this_root = if kotlin_path_is_type_declaration(scope_path, raw_symbols) {
            scope_path
        } else if let Some((parent, _)) = scope_path.rsplit_once("::")
            && kotlin_path_is_type_declaration(parent, raw_symbols)
        {
            scope_path
        } else {
            return Ok(None);
        };
        return resolve_kotlin_type_rooted_member_chain(
            source_symbol,
            this_root,
            chain,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    if let Some(chain) = reference_name.strip_prefix("super.") {
        if chain.is_empty() {
            return Ok(None);
        }
        let Some(superclass_path) = resolve_kotlin_superclass_path(
            source_symbol,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return resolve_kotlin_type_rooted_member_chain(
            source_symbol,
            &superclass_path,
            chain,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    // A bare `Companion` root such as `Companion.items[0].helper(...)` or
    // `Companion.groups[0].inner().helper(...)` inside a type dispatches on
    // the enclosing type's canonical companion scope, so the trailing member
    // chain resolves companion array properties and method-call hops the same
    // way as `this` inside a companion member. Callers outside a type
    // (top-level functions, extension functions) and types without a
    // companion object fail closed because `Companion` has no enclosing
    // companion object to dispatch on.
    if let Some(chain) = reference_name.strip_prefix("Companion.") {
        if chain.is_empty() {
            return Ok(None);
        }
        let Some(companion_scope) = resolve_kotlin_enclosing_companion_scope(
            source_symbol,
            raw_symbols,
            semantic_path_index,
        ) else {
            return Ok(None);
        };
        return resolve_kotlin_type_rooted_member_chain(
            source_symbol,
            &companion_scope,
            chain,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    // A constructor-call receiver such as `Outer.Inner().helper(...)` or
    // `Group().member.helper(...)` resolves the constructed type path first and
    // then dispatches the member chain like any other instance receiver. The
    // `()` marker comes from the extractor's call-rooted navigation base; a
    // function-call base such as `makeOther().helper(...)` fails closed because
    // the callee does not resolve to a constructible type.
    if let Some(marker) = reference_name.find("()")
        && marker > 0
        && reference_name[marker + 2..].starts_with('.')
    {
        return resolve_kotlin_constructor_receiver_call(
            source_symbol,
            reference_name,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    // A factory-call element-access receiver such as
    // `Util.makeItems()[0].helper(...)` or
    // `group.makeGroups()[0].helper(...)` splits at the final dot so the
    // element-access dispatch receives the full receiver spelling; the chained
    // receiver path cannot consume a `()[0]` factory hop, and unknown or
    // unresolvable factories fail closed. A multi-hop receiver such as
    // `h.items[0].make()[0].helper(...)` fails this parse (its second bracket
    // is a factory element access, not multi-dimensional indexing) and falls
    // through to the chained receiver path, which walks each element-access
    // and factory hop.
    if let Some((element_receiver, method)) = reference_name.rsplit_once('.')
        && !method.contains('.')
        && element_receiver.contains("()")
        && element_receiver.ends_with(']')
        && kotlin_array_access_spelling(element_receiver).is_some()
    {
        return resolve_kotlin_qualified_receiver_call(
            source_symbol,
            element_receiver,
            method,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    // A qualified call such as `other.helper(...)` resolves the receiver's type from the
    // caller's local scope and then dispatches to that type's member function. A chained
    // call such as `group.member.helper(...)` additionally resolves each intermediate
    // property's declared type before dispatching the final member.
    if let Some((receiver, method)) = reference_name.split_once('.') {
        if receiver.is_empty() || method.is_empty() {
            return Ok(None);
        }
        if method.contains('.') {
            return resolve_kotlin_chained_receiver_call(
                source_symbol,
                reference_name,
                call_arity,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            );
        }
        if let Some(target) = resolve_kotlin_qualified_receiver_call(
            source_symbol,
            receiver,
            method,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some(target));
        }
        // A qualified call such as `Outer.Inner(...)` may also construct a
        // nested class when the member interpretation fails; the constructor
        // path resolves the dotted type and requires a unique constructible
        // class, so unknown or non-constructible names still fail closed.
        return resolve_kotlin_constructor_call(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    if reference_name.contains("::") {
        return Ok(None);
    }

    // Unqualified calls resolve first against the caller's own scope: an enclosing-type
    // member shadows a package-level function, and a top-level caller's scope is its package.
    if let Some(scope_path) = source_symbol.scope_path.as_deref() {
        let same_scope_candidates = semantic_path_index
            .get(&format!("{scope_path}::{reference_name}"))
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.node_kind == "function_declaration"
                    && candidate.parameters.len() == call_arity
            })
            .collect::<Vec<_>>();
        if same_scope_candidates.len() == 1 {
            return Ok(Some(
                raw_symbols[same_scope_candidates[0]].symbol_id.clone(),
            ));
        }
        if same_scope_candidates.len() > 1 {
            return Ok(None);
        }
        // A bare call inside a member function may also dispatch to a member
        // function inherited from a direct or transitive superclass (an
        // implicit `this` receiver) such as `helper(...)` inside a subclass
        // of a base class that declares `fun helper(...)`, through the same
        // direct, inherited, and extension rules as an explicit `this.`-rooted
        // call. Callers outside a type and unknown or ambiguous inherited
        // members fail closed.
        if let Some(target) = resolve_kotlin_implicit_this_member_chain(
            source_symbol,
            reference_name,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some(target));
        }
        // Companion members are callable unqualified from within the enclosing
        // class, so a type-scoped caller falls back to `Type::Companion::name`
        // before package-level and imported functions.
        if kotlin_package_scope(source_symbol, raw_symbols) != Some(scope_path) {
            let companion_candidates = semantic_path_index
                .get(&format!("{scope_path}::Companion::{reference_name}"))
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.node_kind == "function_declaration"
                        && candidate.parameters.len() == call_arity
                })
                .collect::<Vec<_>>();
            if companion_candidates.len() == 1 {
                return Ok(Some(raw_symbols[companion_candidates[0]].symbol_id.clone()));
            }
            if companion_candidates.len() > 1 {
                return Ok(None);
            }
        }
    }

    // Callers nested inside a type fall through to a package-level top-level function.
    if let Some(package_scope) = kotlin_package_scope(source_symbol, raw_symbols) {
        let target_path = format!("{package_scope}::{reference_name}");
        let package_candidates = semantic_path_index
            .get(&target_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.node_kind == "function_declaration"
                    && candidate.scope_path.as_deref() == Some(package_scope)
                    && candidate.parameters.len() == call_arity
            })
            .collect::<Vec<_>>();
        if package_candidates.len() == 1 {
            return Ok(Some(raw_symbols[package_candidates[0]].symbol_id.clone()));
        }
        if package_candidates.len() > 1 {
            return Ok(None);
        }
    }

    // A unique explicit import can bind an unqualified call to a top-level function in
    // another package. Wildcard imports, aliases that collide, and multiple matching
    // declarations fail closed.
    if let Some(binding) = resolve_kotlin_import_binding_for_reference(
        &source_symbol.file_path,
        reference_name,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        let imported_candidates = semantic_path_index
            .get(&binding.semantic_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.node_kind == "function_declaration"
                    && candidate.parameters.len() == call_arity
            })
            .collect::<Vec<_>>();
        if imported_candidates.len() == 1 {
            return Ok(Some(raw_symbols[imported_candidates[0]].symbol_id.clone()));
        }
    }

    // A bare call to a class name such as `Other(...)` is a constructor call. It
    // resolves only when no function candidate matched and the name uniquely names
    // a constructible class in the caller's scope, package, or explicit imports.
    if let Some(target) = resolve_kotlin_constructor_call(
        source_symbol,
        reference_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        return Ok(Some(target));
    }
    Ok(None)
}

/// Splits an element-access receiver or hop such as `items[0]` into its base
/// name and bracket text. Multi-dimensional element access such as
/// `matrix[0][0]`, empty bases, and malformed brackets return `None` so
/// element-access resolution fails closed.
fn kotlin_array_access_spelling(segment: &str) -> Option<(&str, &str)> {
    let open = segment.find('[')?;
    if !segment.ends_with(']') {
        return None;
    }
    let base = &segment[..open];
    let bracket = &segment[open..];
    // The bracket slice includes its own leading `[`; only a second bracket
    // marks multi-dimensional element access such as `matrix[0][0]`.
    if base.is_empty() || bracket[1..].contains('[') {
        return None;
    }
    Some((base, bracket))
}

/// Parses a factory-call element-access base such as `makeItems()` or
/// `Util.makeItems()` in `makeItems()[0].helper(...)` into the factory callee
/// spelling. The base must be exactly one call with a plain or safe dotted
/// callee; `this`/`super` roots, non-name roots, and malformed spellings fail
/// closed. Multi-dimensional element access is rejected earlier by
/// `kotlin_array_access_spelling`.
fn kotlin_array_factory_call_root_spelling(base: &str) -> Option<String> {
    let open = base.find('(')?;
    let (method_name, rest) = base.split_at(open);
    if method_name.is_empty()
        || method_name.contains(['(', '[', ' ', '?'])
        || method_name.contains("::")
        || method_name
            .split('.')
            .next()
            .is_some_and(|first| first == "this" || first == "super")
    {
        return None;
    }
    if !rest.ends_with(')') {
        return None;
    }
    Some(method_name.to_string())
}

#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_qualified_receiver_call(
    source_symbol: &IndexedSymbol,
    receiver: &str,
    method: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let bindings = kotlin_receiver_type_bindings_for_function(
        &source_symbol.file_path,
        source_symbol.byte_range,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    // An element-access receiver such as `items[0]` strips the bracket and
    // dispatches on the base array's element component type; the base must be
    // bound with a usable single-level array component or a factory-call
    // initializer whose declared return type is a single-level array, and
    // multi-dimensional element access such as `matrix[0][0]` fails closed.
    let (receiver_name, array_access) =
        if let Some((base, _)) = kotlin_array_access_spelling(receiver) {
            (base, true)
        } else if receiver.contains('[') {
            return Ok(None);
        } else {
            (receiver, false)
        };
    // A locally bound receiver (parameter, local property, or enclosing-class
    // property) resolves first; an ambiguous local binding fails closed instead
    // of falling through to a same-named object or type. A direct member call
    // on an array-typed receiver fails closed; only element-access receivers
    // dispatch on the array's element component type.
    if bindings
        .as_ref()
        .is_some_and(|bindings| bindings.contains(receiver_name))
    {
        // A `val` local bound from an `if`/`when` expression initializer such
        // as `val first = if (flag) h.make().items[0].item else
        // Holder().make().items[0].item` has no usable type until all branch
        // spellings resolve to a common declared type; resolve the common
        // type here and dispatch the member on it. Divergent or unresolvable
        // branches fail closed.
        if !array_access
            && let Some(bindings) = bindings.as_ref()
            && let Some(type_path) = resolve_kotlin_branch_initializer_type_path(
                source_symbol,
                receiver_name,
                bindings,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
        {
            let type_name = type_path.rsplit("::").next().unwrap_or(method).to_string();
            return resolve_kotlin_member_or_extension(
                source_symbol,
                &type_path,
                &type_name,
                method,
                call_arity,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            );
        }
        // A `val` local bound from a property-chain initializer such as
        // `val first = holder.item` (including `this`- and `super`-rooted
        // chains and inherited first hops) has no usable type until the chain
        // is walked; resolve the terminal property type here and dispatch the
        // member on it. Unknown or unresolvable chains fail closed.
        if !array_access
            && let Some(chain) = bindings
                .as_ref()
                .and_then(|bindings| bindings.property_chain_base_for(receiver_name))
            && let Some(type_path) = resolve_kotlin_property_chain_initializer_type_path(
                source_symbol,
                &chain,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
        {
            let type_name = type_path.rsplit("::").next().unwrap_or(method).to_string();
            return resolve_kotlin_member_or_extension(
                source_symbol,
                &type_path,
                &type_name,
                method,
                call_arity,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            );
        }
        let type_name = if array_access {
            if let Some(component_type) = bindings
                .as_ref()
                .and_then(|bindings| bindings.array_component_for(receiver_name))
            {
                component_type
            } else if let Some(initializer_name) = bindings
                .as_ref()
                .and_then(|bindings| bindings.type_for(receiver_name))
                && !initializer_name.is_empty()
                && resolve_kotlin_receiver_type_path(
                    source_symbol,
                    &initializer_name,
                    raw_symbols,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                )?
                .is_none()
            {
                // A `val` local initialized from a factory call whose declared
                // return type is a single-level array, such as
                // `val items = makeItems()` with
                // `fun makeItems(): Array<Helper>` or
                // `val items = Util.makeItems()` with a companion, object, or
                // bound-receiver callee, dispatches an element access on the
                // array's element component type through the same factory
                // rules as a direct factory-call element-access receiver.
                // Direct member calls on the array, unknown factories, and
                // non-array return types fail closed.
                return resolve_kotlin_factory_array_element_member_call(
                    source_symbol,
                    &initializer_name,
                    method,
                    call_arity,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                );
            } else if let Some(chain) = bindings
                .as_ref()
                .and_then(|bindings| bindings.property_chain_base_for(receiver_name))
                && let Some(component_path) =
                    resolve_kotlin_property_chain_array_component_type_path(
                        source_symbol,
                        &chain,
                        bindings.as_ref(),
                        raw_symbols,
                        semantic_path_index,
                        file_overrides,
                        kotlin_import_contexts_by_file,
                        deadline,
                    )?
            {
                // A `val` local bound from a property-chain initializer whose
                // terminal property is a single-level array such as
                // `val first = holder.items` (including `this`- and
                // `super`-rooted chains and inherited terminal arrays)
                // dispatches an element access such as `first[0].helper(...)`
                // on the terminal array's element component type through the
                // same member rules as a directly bound array receiver;
                // unknown chains, non-array terminals, and unresolvable hops
                // fail closed.
                let type_name = component_path
                    .rsplit("::")
                    .next()
                    .unwrap_or(method)
                    .to_string();
                return resolve_kotlin_member_or_extension(
                    source_symbol,
                    &component_path,
                    &type_name,
                    method,
                    call_arity,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                );
            } else {
                return Ok(None);
            }
        } else if let Some(type_name) = bindings
            .as_ref()
            .and_then(|bindings| bindings.type_for(receiver_name))
        {
            type_name
        } else {
            // A `val` bound from a qualified element-access initializer such as
            // `val x = group.holder.fieldItems[0]` has no usable type until
            // the chain is walked; resolve the terminal array field's element
            // component type here and dispatch the member on it.
            return resolve_kotlin_qualified_element_access_receiver_call(
                source_symbol,
                receiver_name,
                method,
                call_arity,
                bindings.as_ref(),
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            );
        };
        let Some(type_path) = resolve_kotlin_initializer_type_path(
            source_symbol,
            &type_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let resolved_type_name = type_path
            .rsplit("::")
            .next()
            .unwrap_or(&type_name)
            .to_string();
        return resolve_kotlin_member_or_extension(
            source_symbol,
            &type_path,
            &resolved_type_name,
            method,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    if array_access {
        // A bare factory-call base such as `makeItems()` in
        // `makeItems()[0].helper(...)` resolves the leading call through the
        // same factory rules as a property initializer and dispatches the
        // final member on the factory return array's element component type;
        // unknown factories, primitive or multi-dimensional return arrays, and
        // other unbound element-access bases fail closed.
        if let Some(function_name) = kotlin_array_factory_call_root_spelling(receiver_name) {
            return resolve_kotlin_factory_array_element_member_call(
                source_symbol,
                &function_name,
                method,
                call_arity,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            );
        }
        // An unbound element-access base may also be a property of the
        // enclosing type (an implicit `this` receiver), including an inherited
        // array property such as `items` in `items[0].helper(...)` inside a
        // subclass; the element-access hop resolves the inherited array
        // property's element component type the same way as an explicit
        // `this.`-rooted chain. Unknown bases and unresolvable components fail
        // closed.
        if let Some(target) = resolve_kotlin_implicit_this_member_chain(
            source_symbol,
            &format!("{receiver}.{method}"),
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some(target));
        }
        // An unbound element-access base fails closed instead of falling
        // through to a same-named object or type.
        return Ok(None);
    }
    // An unbound receiver can still be a named object declaration such as
    // `Config.helper(...)`. Object names resolve from the same package or an
    // explicit import; conflicts and unknown names fail closed.
    if let Some(object_path) = resolve_kotlin_object_receiver_path(
        source_symbol,
        receiver,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        return resolve_kotlin_member_or_extension(
            source_symbol,
            &object_path,
            receiver,
            method,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    // A class name such as `Config.helper(...)` can also dispatch to a member
    // of the class's companion object. The receiver resolves as a type path and
    // only members indexed under `Type::Companion::` are eligible; instance
    // members and extensions fail closed because they need an instance receiver.
    if let Some(type_path) = resolve_kotlin_receiver_type_path(
        source_symbol,
        receiver,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? && let Some(target) = resolve_kotlin_companion_member(
        &type_path,
        method,
        call_arity,
        raw_symbols,
        semantic_path_index,
    ) {
        return Ok(Some(target));
    }
    // An unbound receiver may also be a property of the enclosing type (an
    // implicit `this` receiver), including an inherited property such as
    // `holder.helper(...)` inside a subclass; the member resolves through the
    // same direct and inherited rules as an explicit `this.`-rooted chain.
    // Unknown properties and members fail closed.
    if let Some(target) = resolve_kotlin_implicit_this_member_chain(
        source_symbol,
        &format!("{receiver}.{method}"),
        call_arity,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        return Ok(Some(target));
    }
    Ok(None)
}

/// Finds the unique function declaration symbol path under `scope_path` whose
/// base name is `function_name` and which declares a return type, mirroring the
/// uniqueness discipline of `resolve_kotlin_property_initializer_function_path`.
/// Unknown, ambiguous, and return-type-less functions fail closed.
fn kotlin_scope_function_path(
    scope_path: &str,
    function_name: &str,
    raw_symbols: &[IndexedSymbol],
) -> Option<String> {
    let candidates = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.node_kind == "function_declaration"
                && candidate.base_name == function_name
                && candidate.scope_path.as_deref() == Some(scope_path)
                && candidate.return_type.is_some()
        })
        .map(|candidate| candidate.symbol_id.clone())
        .collect::<Vec<_>>();
    (candidates.len() == 1).then(|| candidates[0].clone())
}

/// Resolves the symbol path of a member function called through a `this`- or
/// `super`-rooted call initializer such as `this.ownMake` in
/// `val items = this.ownMake()` or `super.inheritedMake` in
/// `val items = super.inheritedMake()`. A `this` root dispatches on the
/// enclosing type path (or its companion scope inside a companion member),
/// and a `super` root on the direct superclass path; the member function
/// resolves through the same direct, inherited, and extension rules as
/// method-call hops. Callers outside a type, unresolvable superclass paths,
/// and unknown or ambiguous member functions fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_this_super_rooted_member_function_path(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((root, function_name)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    if function_name.is_empty() || !matches!(root, "this" | "super") {
        return Ok(None);
    }
    let root_path = if root == "super" {
        // A `super`-rooted callee starts on the direct superclass path; a
        // class without a resolvable superclass fails closed.
        let Some(superclass_path) = resolve_kotlin_superclass_path(
            source_symbol,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        superclass_path
    } else {
        // A `this`-rooted callee starts on the enclosing type path: a
        // declared type scope, or the companion scope of a declared type for
        // `this` inside a companion member. Callers outside a type fail
        // closed because `this` has no enclosing type to dispatch on.
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        if kotlin_path_is_type_declaration(scope_path, raw_symbols) {
            scope_path.to_string()
        } else if let Some((parent, _)) = scope_path.rsplit_once("::")
            && kotlin_path_is_type_declaration(parent, raw_symbols)
        {
            scope_path.to_string()
        } else {
            return Ok(None);
        }
    };
    resolve_kotlin_member_function_path(
        &root_path,
        function_name,
        source_symbol,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Resolves a `val` local's qualified factory-call initializer callee such as
/// `Util.makeItems` in `val items = Util.makeItems()` to a unique function
/// symbol path. Object-declaration roots such as `Factory.makeItems` dispatch
/// to the object's members, class or interface roots such as `Util.makeItems`
/// dispatch to the companion object's members, explicit companion hops such as
/// `Util.Companion.makeItems` or `Util.Factory.makeItems` dispatch through the
/// canonical companion scope, and bound-receiver chains such as
/// `group.makeItems` resolve the receiver's declared type path before looking
/// up the member function. Unknown or ambiguous receivers, members, and
/// callees fail closed so qualified initializer resolution stays conservative.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_qualified_initializer_function_path(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((receiver_ref, function_name)) = reference_name.rsplit_once('.') else {
        return Ok(None);
    };
    if receiver_ref.is_empty() || function_name.is_empty() {
        return Ok(None);
    }
    // A `this`- or `super`-rooted callee such as `this.ownMake` or
    // `super.inheritedMake` dispatches the member function on the enclosing
    // type (or its companion scope) or the direct superclass path.
    if matches!(receiver_ref, "this" | "super") {
        return resolve_kotlin_this_super_rooted_member_function_path(
            source_symbol,
            reference_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    // A bound receiver shadows any same-named type; resolve the receiver's
    // declared type path and look up the member function on it.
    let bindings = kotlin_receiver_type_bindings_for_function(
        &source_symbol.file_path,
        source_symbol.byte_range,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    if bindings
        .as_ref()
        .is_some_and(|bindings| bindings.contains(receiver_ref))
    {
        let Some(type_name) = bindings
            .as_ref()
            .and_then(|bindings| bindings.type_for(receiver_ref))
        else {
            return Ok(None);
        };
        let Some(type_path) = resolve_kotlin_receiver_type_path(
            source_symbol,
            &type_name,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return Ok(kotlin_scope_function_path(
            &type_path,
            function_name,
            raw_symbols,
        ));
    }
    // An explicit companion hop such as `Util.Companion` or `Util.Factory`
    // maps to the class's canonical `Type::Companion` scope.
    if let Some((owner_ref, companion_name)) = receiver_ref.rsplit_once('.')
        && let Some(class_path) = resolve_kotlin_receiver_type_path(
            source_symbol,
            owner_ref,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        && let Some(companion_scope) = resolve_kotlin_explicit_companion_scope(
            &class_path,
            companion_name,
            raw_symbols,
            semantic_path_index,
        )
    {
        return Ok(kotlin_scope_function_path(
            &companion_scope,
            function_name,
            raw_symbols,
        ));
    }
    // An object-declaration root such as `Factory.makeItems` dispatches to the
    // object's members.
    if let Some(object_path) = resolve_kotlin_object_receiver_path(
        source_symbol,
        receiver_ref,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        return Ok(kotlin_scope_function_path(
            &object_path,
            function_name,
            raw_symbols,
        ));
    }
    // A class or interface root such as `Util.makeItems` dispatches to the
    // companion object's members; instance members fail closed because a class
    // name cannot be an instance receiver.
    if let Some(type_path) = resolve_kotlin_receiver_type_path(
        source_symbol,
        receiver_ref,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        let companion_scope = format!("{type_path}::Companion");
        return Ok(kotlin_scope_function_path(
            &companion_scope,
            function_name,
            raw_symbols,
        ));
    }
    // A multi-hop receiver prefix such as `h.maker.make` or
    // `Registry.factory.make` walks the prefix as a property chain on the
    // bound receiver, object, or class scope before looking up the member
    // function on the terminal type; unknown or unresolvable hops fail
    // closed.
    if receiver_ref.contains('.')
        && let Some(type_path) = resolve_kotlin_receiver_chain_type_path(
            source_symbol,
            receiver_ref,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    {
        return Ok(kotlin_scope_function_path(
            &type_path,
            function_name,
            raw_symbols,
        ));
    }
    Ok(None)
}

/// Resolves the element component type of a factory call whose declared
/// return type is a single-level generic array, such as `makeItems` in
/// `makeItems()[0].helper(...)` or `Util.makeGroups` in
/// `Util.makeGroups()[0].inner().helper(...)`. A bare callee resolves through
/// the property initializer rules (a unique same-file, same-package, or
/// explicitly imported top-level function with a declared return type); a
/// qualified callee resolves through the qualified initializer rules (object,
/// companion, or bound-receiver member). The component path resolves in the
/// factory's own file and enclosing scope. Unknown or ambiguous factories,
/// missing return types, primitive or multi-dimensional return arrays, and
/// unresolved component types return `None` so callers fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_factory_array_element_component_type(
    source_symbol: &IndexedSymbol,
    function_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, String)>> {
    // A bare callee such as `makeItems` resolves through the property
    // initializer rules (a unique same-file, same-package, or explicitly
    // imported top-level function with a declared return type); a qualified
    // callee such as `Util.makeItems` resolves through the qualified
    // initializer rules (object, companion, or bound-receiver member).
    let factory_path = if function_name.contains('.') {
        resolve_kotlin_qualified_initializer_function_path(
            source_symbol,
            function_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    } else {
        resolve_kotlin_property_initializer_function_path(
            source_symbol,
            function_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    };
    let Some(factory_path) = factory_path else {
        return Ok(None);
    };
    let Some(factory) = raw_symbols
        .iter()
        .find(|candidate| candidate.symbol_id == factory_path)
    else {
        return Ok(None);
    };
    let Some(return_type) = factory.return_type.as_deref() else {
        return Ok(None);
    };
    let Some(component_name) = kotlin_array_type_component_name(return_type) else {
        return Ok(None);
    };
    let Some(component_path) = resolve_kotlin_receiver_type_path(
        factory,
        &component_name,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    Ok(Some((component_path, component_name.to_string())))
}

/// Resolves the terminal member of a bare or qualified factory-call
/// element-access receiver such as `makeItems()[0].helper(...)`: the leading
/// call resolves through the same factory rules as a property initializer (a
/// unique same-file, same-package, or explicitly imported top-level function
/// with a declared return type, or a qualified companion, object, or
/// bound-receiver member for dotted callees), the declared return type must be
/// a single-level generic array, and the final member dispatches on the
/// array's element component type. Unknown or ambiguous factories, missing
/// return types, primitive or multi-dimensional return arrays, and unresolved
/// component types fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_factory_array_element_member_call(
    source_symbol: &IndexedSymbol,
    function_name: &str,
    method: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((component_path, component_name)) =
        resolve_kotlin_factory_array_element_component_type(
            source_symbol,
            function_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    else {
        return Ok(None);
    };
    let type_name = component_path
        .rsplit("::")
        .next()
        .unwrap_or(&component_name)
        .to_string();
    resolve_kotlin_member_or_extension(
        source_symbol,
        &component_path,
        &type_name,
        method,
        call_arity,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Resolves the element component type path of a factory-call element-access
/// hop such as `makeGroups()[0]` in `group.makeGroups()[0].inner().helper(...)`
/// or `Util.makeGroups()[0].item.helper(...)`. The factory is a uniquely
/// declared member or companion function on `owner_type_path` whose declared
/// return type is a single-level generic array; the component path resolves in
/// the factory's own file and enclosing scope. Unknown or ambiguous factories,
/// missing return types, primitive or multi-dimensional return arrays, and
/// unresolved component types return `None` so callers fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_owner_factory_array_element_component_type(
    owner_type_path: &str,
    function_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if function_name.is_empty() {
        return Ok(None);
    }
    let mut factory_candidates = Vec::new();
    // The owner scope covers bound-receiver and object factories; the
    // canonical companion scope covers class and interface roots whose
    // factories live on the companion. When neither scope declares the
    // factory, the walk continues on each direct superclass scope (and its
    // companion) so a factory inherited from a base class resolves through
    // `this`/`super`-rooted and bound receivers the same way an inherited
    // method-call hop does. A factory declared in more than one reachable
    // scope is ambiguous and fails closed.
    let mut current_type_path = owner_type_path.to_string();
    let mut visited_type_paths = BTreeSet::new();
    loop {
        if let Some(deadline) = deadline {
            deadline.check("resolving Kotlin owner factory array element")?;
        }
        if !visited_type_paths.insert(current_type_path.clone()) {
            break;
        }
        for scope in [
            current_type_path.clone(),
            format!("{current_type_path}::Companion"),
        ] {
            let candidates = semantic_path_index
                .get(&format!("{scope}::{function_name}"))
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    let candidate = &raw_symbols[*index];
                    candidate.node_kind == "function_declaration"
                        && candidate.scope_path.as_deref() == Some(scope.as_str())
                        && candidate.return_type.is_some()
                })
                .collect::<Vec<_>>();
            factory_candidates.extend(candidates);
        }
        if !factory_candidates.is_empty() {
            break;
        }
        let class_candidates = semantic_path_index
            .get(&current_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.node_kind == "class_declaration" && !kotlin_type_is_interface(candidate)
            })
            .collect::<Vec<_>>();
        let [class_index] = class_candidates.as_slice() else {
            break;
        };
        let Some(superclass_path) = kotlin_superclass_path_for_class(
            &raw_symbols[*class_index],
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            break;
        };
        current_type_path = superclass_path;
    }
    if factory_candidates.len() != 1 {
        return Ok(None);
    }
    let factory = &raw_symbols[factory_candidates[0]];
    let Some(return_type) = factory.return_type.as_deref() else {
        return Ok(None);
    };
    let Some(component_name) = kotlin_array_type_component_name(return_type) else {
        return Ok(None);
    };
    resolve_kotlin_receiver_type_path(
        factory,
        &component_name,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Resolves an intermediate hop in a chained Kotlin receiver call. A
/// factory-call element-access hop such as `makeGroups()[0]` resolves the
/// leading call as a uniquely declared factory on the current receiver type
/// and continues on the factory return array's element component type; all
/// other hops fall through to the shared property and method-call hop rules.
/// `this`/`super`-rooted chains resolve through
/// `resolve_kotlin_type_rooted_member_chain` with this wrapper, so factory
/// element-access hops declared on the enclosing type or its superclass chain
/// resolve there too.
#[allow(clippy::too_many_arguments)]
fn kotlin_chained_receiver_hop_type_path(
    owner_type_path: &str,
    hop: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if let Some((factory_hop, _)) = kotlin_array_access_spelling(hop)
        && let Some(function_name) = kotlin_array_factory_call_root_spelling(factory_hop)
    {
        return resolve_kotlin_owner_factory_array_element_component_type(
            owner_type_path,
            &function_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    kotlin_chain_hop_type_path(
        owner_type_path,
        hop,
        source_symbol,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Resolves the direct superclass type path of the class enclosing
/// `source_symbol`, such as `Base` in `class Caller : Base()`, by locating the
/// enclosing class declaration's first delegation specifier. The superclass
/// must be a pure dotted type spelling with no type arguments, nullable, or
/// delegation (`by`) modifiers, and must resolve through the same type-path
/// rules as any receiver type. Classes without a resolvable superclass fail
/// closed.
fn resolve_kotlin_superclass_path(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
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
            deadline.check("locating Kotlin superclass")?;
        }
        if node.kind() == "function_declaration"
            && (node.start_byte(), node.end_byte()) == source_symbol.byte_range
        {
            let mut ancestor = node.parent();
            while let Some(candidate) = ancestor {
                if candidate.kind() == "class_declaration" {
                    let mut cursor = candidate.walk();
                    let Some(specifiers) = candidate
                        .named_children(&mut cursor)
                        .find(|child| child.kind() == "delegation_specifiers")
                    else {
                        return Ok(None);
                    };
                    let mut specifier_cursor = specifiers.walk();
                    let Some(specifier) = specifiers.named_children(&mut specifier_cursor).next()
                    else {
                        return Ok(None);
                    };
                    superclass_reference =
                        kotlin_delegation_specifier_type_name(specifier, &source)?;
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
    resolve_kotlin_receiver_type_path(
        source_symbol,
        &superclass_reference,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Extracts the pure dotted type spelling of a delegation specifier such as
/// `Base` in `class Caller : Base()`, `Base` in `class Caller : Base`, or
/// `Base` in `interface Derived : Base`. Constructor invocations unwrap to
/// their `type` child, and bare `type`/`user_type` spellings pass through;
/// delegation (`by`) specifiers and other shapes fail closed.
fn kotlin_delegation_specifier_type_name(
    specifier: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    // A `delegation_specifier` such as `Base()` or `Base` wraps the concrete
    // type node; unwrap it before extracting the pure dotted spelling.
    let specifier = match specifier.kind() {
        "delegation_specifier" => {
            let mut cursor = specifier.walk();
            specifier.named_children(&mut cursor).next()
        }
        _ => Some(specifier),
    };
    let Some(specifier) = specifier else {
        return Ok(None);
    };
    let type_node = match specifier.kind() {
        "constructor_invocation" => {
            let mut cursor = specifier.walk();
            specifier.named_children(&mut cursor).next()
        }
        "type" | "user_type" => Some(specifier),
        _ => None,
    };
    let Some(type_node) = type_node else {
        return Ok(None);
    };
    let text = node_text(type_node, source)?.trim();
    Ok(kotlin_dotted_type_name(text))
}

/// Dispatches the terminal member of a receiver name bound from a qualified
/// element-access initializer such as `val x = group.holder.fieldItems[0]`,
/// a `super`-rooted initializer such as `val x = super.inheritedItems[0]`, a
/// companion-object initializer such as `val x = Util.fieldItems[0]`, or a
/// factory-call initializer such as `val x = makeItems()[0]` on the resolved
/// element component type of the initializer's terminal array.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_qualified_element_access_receiver_call(
    source_symbol: &IndexedSymbol,
    receiver_name: &str,
    method: &str,
    call_arity: usize,
    bindings: Option<&KotlinReceiverTypeBindings>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(base) = bindings.and_then(|bindings| bindings.element_access_base_for(receiver_name))
    else {
        return Ok(None);
    };
    let Some(component_path) = resolve_kotlin_element_access_base_component_type_path(
        source_symbol,
        &base,
        bindings,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let type_name = component_path
        .rsplit("::")
        .next()
        .unwrap_or(method)
        .to_string();
    resolve_kotlin_member_or_extension(
        source_symbol,
        &component_path,
        &type_name,
        method,
        call_arity,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Resolves the element component type path of a name bound from a qualified
/// element-access initializer such as `val x = group.holder.fieldItems[0]`,
/// a `this`-rooted initializer such as `val x = this.groups[0]`, a
/// `super`-rooted initializer such as `val x = super.inheritedItems[0]`, a
/// companion-object initializer such as `val x = Util.fieldItems[0]`, a
/// factory-call initializer such as `val x = makeItems()[0]`, or a qualified
/// factory-call initializer such as `val x = Util.makeItems()[0]`.
/// A factory-call base records the callee with a trailing `()` marker and
/// resolves through the same factory rules as a direct factory-call
/// element-access receiver. Otherwise the base's first hop must be `this`
/// (the enclosing type path), `super` (the direct superclass path), a bound
/// receiver with a usable declared type, a named object whose terminal
/// property lives on the object itself, or an unbound type whose terminal
/// property lives on its companion object; intermediate hops walk the same
/// property-type rules as chained receivers, and the terminal hop must be a
/// uniquely declared single-level array property (declared on the owner or
/// inherited through its class or interface chain) whose element component
/// type is returned. Unbound or non-array first hops, unknown or non-array
/// terminal properties, and unresolvable intermediate hops return `None` so
/// callers fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_element_access_base_component_type_path(
    source_symbol: &IndexedSymbol,
    base: &str,
    bindings: Option<&KotlinReceiverTypeBindings>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    // A factory-call base such as `val x = makeItems()[0]` records the callee
    // with a trailing `()` marker; resolve the factory's declared return array
    // through the same rules as a direct factory-call element-access receiver
    // and return the array's element component type.
    if let Some(function_name) = base.strip_suffix("()") {
        if function_name.is_empty() {
            return Ok(None);
        }
        let Some((component_path, _)) = resolve_kotlin_factory_array_element_component_type(
            source_symbol,
            function_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return Ok(Some(component_path));
    }
    // A bare bound array name such as `items` in `val first = items[0]` where
    // `items` is a parameter, local, enclosing-class property, or implicit
    // companion property with a single-level array type dispatches directly on
    // the bound element component type.
    if !base.contains('.')
        && let Some(component_type) =
            bindings.and_then(|bindings| bindings.array_component_for(base))
        && let Some(component_path) = resolve_kotlin_initializer_type_path(
            source_symbol,
            &component_type,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    {
        return Ok(Some(component_path));
    }
    // A plain bare base that is not locally bound may be a same-package or
    // explicitly imported top-level array property such as `itemGroup` in
    // `val first = itemGroup[0]` with `val itemGroup: Array<Holder>` at
    // package scope, whose element component type resolves in the property's
    // own file scope; a locally bound or member name shadows the top-level
    // property, so element access on a bound non-array base fails closed.
    if !base.contains('.') {
        if bindings.is_some_and(|bindings| bindings.contains(base)) {
            return Ok(None);
        }
        if let Some(component_path) = resolve_kotlin_top_level_property_array_component_path(
            source_symbol,
            base,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some(component_path));
        }
    }
    let Some((first_hop, chain)) = base.split_once('.') else {
        return Ok(None);
    };
    if first_hop.is_empty() || chain.is_empty() {
        return Ok(None);
    }
    let hops = chain.split('.').collect::<Vec<_>>();
    let (mut current_path, skip) = if first_hop == "super" {
        // A `super`-rooted base such as `val x = super.inheritedItems[0]`
        // starts on the direct superclass path; a class without a resolvable
        // superclass fails closed.
        let Some(superclass_path) = resolve_kotlin_superclass_path(
            source_symbol,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        (superclass_path, 0)
    } else if first_hop == "this" {
        // A `this`-rooted base such as `val x = this.groups[0]` starts on
        // the enclosing type path: a declared type scope, or the companion
        // scope of a declared type for `this` inside a companion member.
        // Callers outside a type (top-level functions, extension functions)
        // fail closed because `this` has no enclosing type to dispatch on.
        let Some(scope_path) = source_symbol.scope_path.as_deref() else {
            return Ok(None);
        };
        let this_root = if kotlin_path_is_type_declaration(scope_path, raw_symbols) {
            scope_path
        } else if let Some((parent, _)) = scope_path.rsplit_once("::")
            && kotlin_path_is_type_declaration(parent, raw_symbols)
        {
            scope_path
        } else {
            return Ok(None);
        };
        (this_root.to_string(), 0)
    } else if first_hop == "Companion" {
        // A bare `Companion` root such as `val x = Companion.items[0]` inside
        // a type starts on the enclosing type's canonical companion scope, the
        // same target as `this` inside a companion member; callers outside a
        // type and types without a companion object fail closed.
        let Some(companion_scope) = resolve_kotlin_enclosing_companion_scope(
            source_symbol,
            raw_symbols,
            semantic_path_index,
        ) else {
            return Ok(None);
        };
        (companion_scope, 0)
    } else if let Some(first_type_name) = bindings.and_then(|bindings| bindings.type_for(first_hop))
    {
        let Some(type_path) = resolve_kotlin_initializer_type_path(
            source_symbol,
            &first_type_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        (type_path, 0)
    } else if let Some(object_path) = resolve_kotlin_object_receiver_path(
        source_symbol,
        first_hop,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        // An unbound first hop that names a declared object such as
        // `Factory.fieldItems` in `val x = Factory.fieldItems[0]` starts on
        // the object's own scope, so the terminal array property must be
        // declared on the object itself. The object name resolves from the
        // same package or an explicit import; unknown or ambiguous objects
        // fail closed.
        (object_path, 0)
    } else {
        // An unbound first hop names a type whose terminal array property
        // lives on its companion object, the Kotlin analog of a Java static
        // field such as `Util.fieldItems` in `val x = Util.fieldItems[0]`.
        // A named or nested companion hop such as `Config.Factory.groups` or
        // `Outer.Inner.Factory.groups` consumes the leading companion hops
        // onto the canonical companion scope, and the class must otherwise
        // declare a companion object; anonymous companions are discovered
        // through their `Type::Companion::` member scope while named
        // companions surface as an indexed `companion_object` symbol.
        // Missing, ambiguous, or non-class roots fail closed.
        let mut base_hops = Vec::with_capacity(hops.len() + 1);
        base_hops.push(first_hop);
        base_hops.extend(hops.iter().copied());
        if let Some((companion_root, consumed)) = kotlin_companion_chain_root(
            source_symbol,
            &base_hops,
            bindings,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            // The companion root must leave at least the terminal array
            // property hop, so an element access directly on the companion
            // object itself such as `Config.Factory[0]` still fails closed.
            if consumed >= base_hops.len() {
                return Ok(None);
            }
            (format!("{companion_root}::Companion"), consumed - 1)
        } else {
            let Some(type_path) = resolve_kotlin_receiver_type_path(
                source_symbol,
                first_hop,
                raw_symbols,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            let companion_scope_prefix = format!("{type_path}::Companion::");
            let companion_exists = semantic_path_index.iter().any(|(path, indexes)| {
                path.starts_with(&companion_scope_prefix)
                    || indexes.iter().copied().any(|index| {
                        let candidate = &raw_symbols[index];
                        candidate.node_kind == "companion_object"
                            && candidate.scope_path.as_deref() == Some(type_path.as_str())
                    })
            });
            if !companion_exists {
                return Ok(None);
            }
            (format!("{type_path}::Companion"), 0)
        }
    };
    for (index, hop) in hops.iter().enumerate().skip(skip) {
        let is_terminal = index + 1 == hops.len();
        let next_path = if is_terminal {
            kotlin_array_property_component_type_path(
                &current_path,
                hop,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
        } else {
            kotlin_property_type_path(
                &current_path,
                hop,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
        };
        let Some(next_path) = next_path else {
            return Ok(None);
        };
        current_path = next_path;
    }
    Ok(Some(current_path))
}

/// Resolves the element component type path of a uniquely declared array
/// property such as `fieldItems` on `owner_type_path` in
/// `val x = group.holder.fieldItems[0]`. The property may be declared under
/// the owner type or inherited through its class or interface chain, and must
/// carry a single-level generic array type whose component resolves in the
/// property's own file and enclosing scope. Unknown, ambiguous, non-array,
/// or multi-dimensional property types fail closed.
#[allow(clippy::too_many_arguments)]
fn kotlin_array_property_component_type_path(
    owner_type_path: &str,
    property_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if property_name.is_empty() {
        return Ok(None);
    }
    let candidates = semantic_path_index
        .get(&format!("{owner_type_path}::{property_name}"))
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "property_declaration"
                && candidate.scope_path.as_deref() == Some(owner_type_path)
        })
        .collect::<Vec<_>>();
    let property_index = if candidates.len() == 1 {
        Some(candidates[0])
    } else if candidates.is_empty() {
        // An inherited terminal array property such as
        // `this.inheritedGroups[0]` where `inheritedGroups` is declared on a
        // parent class or interface walks the same inherited-member rules as
        // property hops; ambiguous or blocked chains fail closed.
        resolve_kotlin_inherited_property_index(
            owner_type_path,
            property_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    } else {
        None
    };
    let Some(property_index) = property_index else {
        return Ok(None);
    };
    let Some(return_type) = raw_symbols[property_index].return_type.as_deref() else {
        return Ok(None);
    };
    let Some(component_name) = kotlin_array_type_component_name(return_type) else {
        return Ok(None);
    };
    resolve_kotlin_receiver_type_path(
        &raw_symbols[property_index],
        &component_name,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Maps the second hop of a companion chain such as `Config.Factory.member(...)`
/// or `Config.Companion.member(...)` to the class's canonical companion scope
/// `{class_path}::Companion`. The literal `Companion` is always available; any
/// other name resolves only when exactly one companion object is declared under
/// that name. Object declarations cannot host companion objects, so an
/// object-resolved class path fails closed.
fn resolve_kotlin_explicit_companion_scope(
    class_path: &str,
    companion_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Option<String> {
    if kotlin_path_object_count(class_path, raw_symbols) != 0 {
        return None;
    }
    if companion_name == "Companion" {
        return Some(format!("{class_path}::Companion"));
    }
    let companion_indexes = semantic_path_index
        .get(&format!("{class_path}::{companion_name}"))
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| raw_symbols[*index].node_kind == "companion_object")
        .collect::<Vec<_>>();
    (companion_indexes.len() == 1).then(|| format!("{class_path}::Companion"))
}

/// Resolves `method` on the companion object of `type_path`. Only a unique
/// member indexed under `Type::Companion::method` with a matching arity
/// resolves; an ambiguous overload set fails closed.
fn resolve_kotlin_companion_member(
    type_path: &str,
    method: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Option<String> {
    let companion_scope = format!("{type_path}::Companion");
    let candidates = semantic_path_index
        .get(&format!("{companion_scope}::{method}"))
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "function_declaration"
                && candidate.scope_path.as_deref() == Some(companion_scope.as_str())
                && candidate.parameters.len() == call_arity
        })
        .collect::<Vec<_>>();
    (candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone())
}

/// Resolves a companion-chain receiver root such as `Config.Factory`,
/// `Config.Companion`, `Outer.Inner`, or `Outer.Inner.Companion` to the
/// canonical dispatch scope and the number of leading hops consumed. The
/// first hop must resolve to a uniquely declared type and not be shadowed by
/// a local binding; a second hop naming a companion object (its declared name
/// or the literal `Companion`) consumes two hops onto the canonical
/// `{type}::Companion` scope, while a second hop naming a nested class or
/// interface consumes two hops onto the nested type (or three when a third
/// hop names its companion). Object declarations cannot host companion
/// objects. Unknown or ambiguous roots return `None` so callers fail closed.
#[allow(clippy::too_many_arguments)]
fn kotlin_companion_chain_root(
    source_symbol: &IndexedSymbol,
    hops: &[&str],
    bindings: Option<&KotlinReceiverTypeBindings>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, usize)>> {
    if hops.len() < 2
        || bindings
            .as_ref()
            .is_some_and(|bindings| bindings.contains(hops[0]))
    {
        return Ok(None);
    }
    let Some(class_path) = resolve_kotlin_receiver_type_path(
        source_symbol,
        hops[0],
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let direct_scope = resolve_kotlin_explicit_companion_scope(
        &class_path,
        hops[1],
        raw_symbols,
        semantic_path_index,
    );
    let nested_path = kotlin_path_nested_class_path(&class_path, hops[1], raw_symbols);
    match (direct_scope.is_some(), nested_path) {
        (true, _) => Ok(Some((class_path, 2))),
        (false, Some(nested_path)) => {
            let nested_companion = hops.len() >= 4
                && resolve_kotlin_explicit_companion_scope(
                    &nested_path,
                    hops[2],
                    raw_symbols,
                    semantic_path_index,
                )
                .is_some();
            Ok(Some((nested_path, if nested_companion { 3 } else { 2 })))
        }
        (false, None) => Ok(None),
    }
}

/// Resolves an anonymous-companion chain root such as `Util` in
/// `Util.items[0].helper(...)` or `Util.groups[0].inner().helper(...)` to the
/// canonical `{type}::Companion` scope and the number of leading hops
/// consumed. A class-name root that is not shadowed by a local binding and
/// whose anonymous companion object exists (discovered through its
/// `Type::Companion::` member scope or an indexed companion object) consumes
/// one hop so the remaining hops resolve on the companion scope; classes
/// without a companion object, unknown names, object roots, and shadowed
/// roots fail closed. Callers must try `kotlin_companion_chain_root` first so
/// explicit and nested companions keep their longer consumed counts.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_anonymous_companion_chain_root(
    source_symbol: &IndexedSymbol,
    hops: &[&str],
    bindings: Option<&KotlinReceiverTypeBindings>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, usize)>> {
    if hops.len() < 2
        || bindings
            .as_ref()
            .is_some_and(|bindings| bindings.contains(hops[0]))
    {
        return Ok(None);
    }
    let Some(type_path) = resolve_kotlin_receiver_type_path(
        source_symbol,
        hops[0],
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let companion_scope_prefix = format!("{type_path}::Companion::");
    let companion_exists = semantic_path_index.iter().any(|(path, indexes)| {
        path.starts_with(&companion_scope_prefix)
            || indexes.iter().copied().any(|index| {
                let candidate = &raw_symbols[index];
                candidate.node_kind == "companion_object"
                    && candidate.scope_path.as_deref() == Some(type_path.as_str())
            })
    });
    if !companion_exists {
        return Ok(None);
    }
    Ok(Some((format!("{type_path}::Companion"), 1)))
}

/// Resolves the canonical companion scope of the type enclosing
/// `source_symbol`, such as `com::example::Util::Companion` for a member of
/// class `Util` or a member of its companion object, so a bare `Companion`
/// root dispatches on the same scope as `this` inside a companion member.
/// Callers outside a type (top-level and extension functions) and types
/// without a companion object fail closed.
fn resolve_kotlin_enclosing_companion_scope(
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Option<String> {
    let scope_path = source_symbol.scope_path.as_deref()?;
    let companion_root = if kotlin_path_is_type_declaration(scope_path, raw_symbols) {
        scope_path
    } else if let Some((parent, _)) = scope_path.rsplit_once("::")
        && kotlin_path_is_type_declaration(parent, raw_symbols)
    {
        parent
    } else {
        return None;
    };
    let companion_scope = format!("{companion_root}::Companion");
    let companion_scope_prefix = format!("{companion_scope}::");
    let companion_exists = semantic_path_index.iter().any(|(path, indexes)| {
        path.starts_with(&companion_scope_prefix)
            || indexes.iter().copied().any(|index| {
                let candidate = &raw_symbols[index];
                candidate.node_kind == "companion_object"
                    && candidate.scope_path.as_deref() == Some(companion_root)
            })
    });
    companion_exists.then_some(companion_scope)
}

/// Returns the enclosing type path an implicit or explicit `this` receiver
/// dispatches on: a declared type scope, or the companion scope of a declared
/// type for callers inside a companion member. Package-level and
/// extension-function scopes return `None` because `this` has no enclosing
/// type to dispatch on.
fn kotlin_enclosing_this_root<'a>(
    source_symbol: &'a IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
) -> Option<&'a str> {
    let scope_path = source_symbol.scope_path.as_deref()?;
    if kotlin_path_is_type_declaration(scope_path, raw_symbols) {
        Some(scope_path)
    } else if let Some((parent, _)) = scope_path.rsplit_once("::")
        && kotlin_path_is_type_declaration(parent, raw_symbols)
    {
        Some(scope_path)
    } else {
        None
    }
}

/// Resolves the terminal type path of a name bound from a property-chain
/// initializer such as `val first = holder.item`, `val first =
/// this.holder.item`, or `val first = super.baseItem`. A `this`-rooted chain
/// starts on the enclosing type path, a `super`-rooted chain on the direct
/// superclass path, and a bare chain on the enclosing type path as an
/// implicit `this` receiver (so inherited first hops resolve the same way as
/// an explicit `this.`-rooted chain); each following hop walks the same
/// property, array-element, and method-call hop rules as chained receivers.
/// Callers outside a type, unresolvable roots, and unknown or ambiguous hops
/// return `None` so callers fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_property_chain_initializer_type_path(
    source_symbol: &IndexedSymbol,
    chain: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let hops = chain.split('.').collect::<Vec<_>>();
    if hops.is_empty() || hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    let Some((mut type_path, skip)) = kotlin_property_chain_initializer_root(
        source_symbol,
        &hops,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    for hop in hops.iter().skip(skip) {
        let Some(next_path) = kotlin_chained_receiver_hop_type_path(
            &type_path,
            hop,
            source_symbol,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        type_path = next_path;
    }
    Ok(Some(type_path))
}

/// Resolves the starting type path for a property-chain initializer chain: a
/// `super`-rooted chain starts on the direct superclass path, a `this`-rooted
/// chain on the enclosing type path, a bare property chain on the enclosing
/// type path as an implicit `this` receiver (so inherited first hops resolve
/// the same way as an explicit `this.`-rooted chain), and a leading
/// method-call hop such as `make()` in `val first = make().item` on the
/// declared return type of a unique same-file, same-package, or explicitly
/// imported top-level function, falling back to an enclosing-type member or
/// companion member function through the same rules as an unqualified
/// initializer callee; when no such function exists, a plain constructor-call
/// hop such as `Holder()` in `val first = Holder().item` starts the chain on
/// the constructed type. A locally bound bare first hop (such as `x` in
/// `val first = x.item` after `val x = Holder()`) starts the chain on the
/// bound value's declared type, its recorded property-chain terminal type,
/// or its element-access component type; a bare first hop that names a named
/// object, a class with an explicit or named companion chain
/// (`Config.Factory` or `Config.Companion`), or a class with an anonymous
/// companion starts the chain on that object or companion scope, and a bare
/// first hop that names a same-package or explicitly imported top-level
/// property starts the chain on the property's declared type while an own or
/// inherited property of the enclosing type shadows a same-named top-level
/// property, before all of these fall back to the enclosing type's own
/// property. The returned skip count tells callers how many leading hops the
/// root consumed. Bare property and `this`-rooted chains outside a type
/// return `None` so chains fail closed.
#[allow(clippy::too_many_arguments)]
fn kotlin_property_chain_initializer_root(
    source_symbol: &IndexedSymbol,
    hops: &[&str],
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, usize)>> {
    let first_hop = hops[0];
    if first_hop == "super" {
        let Some(superclass_path) = resolve_kotlin_superclass_path(
            source_symbol,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        Ok(Some((superclass_path, 1)))
    } else if first_hop == "this" {
        let Some(this_root) = kotlin_enclosing_this_root(source_symbol, raw_symbols) else {
            return Ok(None);
        };
        Ok(Some((this_root.to_string(), 1)))
    } else if let Some(hop_name) = kotlin_method_call_hop_spelling(first_hop) {
        // A leading method-call hop such as `make()` in `val first =
        // make().item` resolves as a unique initializer callee (same-file,
        // same-package, or explicitly imported top-level function, then an
        // enclosing-type member or companion member function) whose declared
        // return type starts the chain; when no such function exists, the hop
        // may instead be a plain constructor call such as `Holder()` in
        // `val first = Holder().item`, which starts the chain on the
        // constructed type. Unknown or ambiguous callees fail closed.
        if let Some(function_path) = resolve_kotlin_property_initializer_function_path(
            source_symbol,
            &hop_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            let Some(function) = raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == function_path)
            else {
                return Ok(None);
            };
            let Some(return_type) = function
                .return_type
                .as_deref()
                .and_then(kotlin_dotted_type_name)
            else {
                return Ok(None);
            };
            let Some(type_path) = resolve_kotlin_receiver_type_path(
                function,
                &return_type,
                raw_symbols,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            return Ok(Some((type_path, 1)));
        }
        let Some(constructor_name) = kotlin_dotted_type_name(&hop_name) else {
            return Ok(None);
        };
        if let Some(type_path) = resolve_kotlin_receiver_type_path(
            source_symbol,
            &constructor_name,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some((type_path, 1)));
        }
        Ok(None)
    } else {
        let bindings = kotlin_receiver_type_bindings_for_function(
            &source_symbol.file_path,
            source_symbol.byte_range,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?;
        // A locally bound first hop dispatches through the bound value
        // instead of a same-named object, companion, or enclosing-type
        // member: `val x = Holder()` then `val first = x.item` starts the
        // chain on `Holder`, `val x = group.holder` on the terminal type of
        // that chain, and `val x = makeItems()[0]` on the element component
        // type. Bound names whose declared type or base cannot resolve fail
        // closed instead of guessing a same-named root.
        if bindings
            .as_ref()
            .is_some_and(|bindings| bindings.contains(first_hop))
        {
            // A name bound from an `if`/`when` expression initializer such as
            // `val group = if (flag) h.make() else Holder().make()` resolves
            // all branch spellings to a common declared type path before any
            // other bound-first-hop inference; divergent or unresolvable
            // branches fail closed.
            if let Some(bindings) = bindings.as_ref()
                && let Some(path) = resolve_kotlin_branch_initializer_type_path(
                    source_symbol,
                    first_hop,
                    bindings,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                )?
            {
                return Ok(Some((path, 1)));
            }
            if let Some(type_name) = bindings
                .as_ref()
                .and_then(|bindings| bindings.type_for(first_hop))
            {
                let Some(type_path) = resolve_kotlin_initializer_type_path(
                    source_symbol,
                    &type_name,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                )?
                else {
                    return Ok(None);
                };
                return Ok(Some((type_path, 1)));
            }
            if let Some(base) = bindings
                .as_ref()
                .and_then(|bindings| bindings.element_access_base_for(first_hop))
                && let Some(component_path) =
                    resolve_kotlin_element_access_base_component_type_path(
                        source_symbol,
                        &base,
                        bindings.as_ref(),
                        raw_symbols,
                        semantic_path_index,
                        file_overrides,
                        kotlin_import_contexts_by_file,
                        deadline,
                    )?
            {
                return Ok(Some((component_path, 1)));
            }
            if let Some(chain) = bindings
                .as_ref()
                .and_then(|bindings| bindings.property_chain_base_for(first_hop))
                && let Some(chain_path) = resolve_kotlin_property_chain_initializer_type_path(
                    source_symbol,
                    &chain,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                )?
            {
                return Ok(Some((chain_path, 1)));
            }
            return Ok(None);
        }
        // A leading element-access hop such as `itemGroup[0]` in
        // `val x = itemGroup[0].make()` may index an own or inherited member
        // array of the enclosing type (the implicit `this` receiver) or a
        // same-package or explicitly imported top-level array property; the
        // member array shadows the top-level property (Kotlin scope: local
        // binding, then member, then package), so when the enclosing type
        // declares the base name at all the member wins and the hop walk
        // resolves it (failing closed for non-array members), and only an
        // undeclared base falls back to the top-level array property's
        // element component. A leading factory element-access hop such as
        // `makeGroups()[0]` in `val first = makeGroups()[0].item` resolves
        // the factory's declared return array element component type through
        // the same rules as a direct factory-call element-access receiver (a
        // unique same-file, same-package, or explicitly imported top-level
        // function, then an enclosing-type member or companion member
        // function); unknown or non-array-returning factories fail closed.
        // Unknown bases and unresolvable components fail closed.
        if let Some((base_name, _)) = kotlin_array_access_spelling(first_hop)
            && let Some(function_name) = kotlin_array_factory_call_root_spelling(base_name)
        {
            if let Some((component_path, _)) = resolve_kotlin_factory_array_element_component_type(
                source_symbol,
                &function_name,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )? {
                return Ok(Some((component_path, 1)));
            }
            let Some(this_root) = kotlin_enclosing_this_root(source_symbol, raw_symbols) else {
                return Ok(None);
            };
            return Ok(Some((this_root.to_string(), 0)));
        }
        // A leading element-access hop whose base is a locally bound name,
        // such as `group[0]` in `val first = group[0].item` after
        // `val group = makeGroups()`, dispatches through the bound name's
        // array component type: a direct array component binding (a
        // parameter, local, or member with a single-level array type), or a
        // factory-call binding whose declared return type is a single-level
        // array resolved through the same factory rules as a direct
        // factory-call element-access receiver. Bound non-array names,
        // unknown factories, and unresolvable components fail closed because
        // the local binding shadows member and top-level arrays.
        if let Some((base_name, _)) = kotlin_array_access_spelling(first_hop)
            && !base_name.contains('(')
            && bindings
                .as_ref()
                .is_some_and(|bindings| bindings.contains(base_name))
        {
            if let Some(component_type) = bindings
                .as_ref()
                .and_then(|bindings| bindings.array_component_for(base_name))
            {
                let Some(component_path) = resolve_kotlin_initializer_type_path(
                    source_symbol,
                    &component_type,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                )?
                else {
                    return Ok(None);
                };
                return Ok(Some((component_path, 1)));
            }
            if let Some(initializer_name) = bindings
                .as_ref()
                .and_then(|bindings| bindings.type_for(base_name))
                && !initializer_name.is_empty()
                && resolve_kotlin_receiver_type_path(
                    source_symbol,
                    &initializer_name,
                    raw_symbols,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                )?
                .is_none()
                && let Some((component_path, _)) =
                    resolve_kotlin_factory_array_element_component_type(
                        source_symbol,
                        &initializer_name,
                        raw_symbols,
                        semantic_path_index,
                        file_overrides,
                        kotlin_import_contexts_by_file,
                        deadline,
                    )?
            {
                return Ok(Some((component_path, 1)));
            }
            return Ok(None);
        }
        if let Some((base_name, _)) = kotlin_array_access_spelling(first_hop)
            && !base_name.contains('(')
        {
            let this_root = kotlin_enclosing_this_root(source_symbol, raw_symbols);
            let member_shadows = if let Some(root) = this_root {
                kotlin_type_declares_property(
                    root,
                    base_name,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                )?
            } else {
                false
            };
            if member_shadows {
                let Some(this_root) = this_root else {
                    return Ok(None);
                };
                return Ok(Some((this_root.to_string(), 0)));
            }
            if let Some(component_path) = resolve_kotlin_top_level_property_array_component_path(
                source_symbol,
                base_name,
                raw_symbols,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )? {
                return Ok(Some((component_path, 1)));
            }
            let Some(this_root) = this_root else {
                return Ok(None);
            };
            return Ok(Some((this_root.to_string(), 0)));
        }
        if let Some(object_path) = resolve_kotlin_object_receiver_path(
            source_symbol,
            first_hop,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some((object_path, 1)));
        }
        // A nested constructor-call chain root such as `Outer.Nested()` in
        // `val group = Outer.Nested().make().items[0]` or
        // `Outer.Nested.Inner()` in `val group = Outer.Nested.Inner().make()`
        // resolves the first hop as a type path, walks each following
        // non-`()` hop through uniquely declared nested types, and constructs
        // the first `()`-marked hop as a uniquely constructible nested class,
        // consuming all hops up to and including the constructor hop so the
        // remaining chain dispatches on the constructed nested type. Unknown,
        // ambiguous, and non-constructible nested classes and unresolvable
        // intermediate hops return `None` so the companion-chain roots below
        // still resolve `Outer.Nested.make()` and `Outer.Nested` chains.
        if let Some((nested_path, consumed)) = resolve_kotlin_nested_constructor_chain_root(
            source_symbol,
            hops,
            bindings.as_ref(),
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some((nested_path, consumed)));
        }
        if let Some((companion_root, consumed)) = kotlin_companion_chain_root(
            source_symbol,
            hops,
            bindings.as_ref(),
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some((format!("{companion_root}::Companion"), consumed)));
        }
        if let Some((companion_root, _)) = resolve_kotlin_anonymous_companion_chain_root(
            source_symbol,
            hops,
            bindings.as_ref(),
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some((companion_root, 1)));
        }
        // A bare first hop that names a same-package or explicitly imported
        // top-level property (`val holder: Holder = Holder()` at package
        // scope) starts the chain on the property's declared type, but an own
        // or inherited property of the enclosing type shadows a same-named
        // top-level property (Kotlin scope: local binding, then member, then
        // package), so the implicit `this` receiver dispatches first when the
        // enclosing type declares the name. Unknown, ambiguous, primitive, and
        // untyped top-level properties fail closed.
        if let Some(property_path) = resolve_kotlin_top_level_property_receiver_path(
            source_symbol,
            first_hop,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            if let Some(this_root) = kotlin_enclosing_this_root(source_symbol, raw_symbols)
                && kotlin_type_declares_property(
                    this_root,
                    first_hop,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                )?
            {
                return Ok(Some((this_root.to_string(), 0)));
            }
            return Ok(Some((property_path, 1)));
        }
        let Some(this_root) = kotlin_enclosing_this_root(source_symbol, raw_symbols) else {
            return Ok(None);
        };
        Ok(Some((this_root.to_string(), 0)))
    }
}

/// Resolves the terminal element component type path of a name bound from a
/// property-chain initializer whose terminal property is a single-level
/// array, such as `val first = holder.items` with `val items: Array<Item>`,
/// including `this`- and `super`-rooted chains, inherited terminal array
/// properties, a single-hop chain that names a bound value declared as or
/// derived from a single-level array (an own or inherited member property, a
/// parameter or explicitly typed local, a name re-bound from another chain or
/// element-access base such as `val group = itemGroup` then `val first =
/// group`), a single-hop chain that names a same-package or explicitly
/// imported top-level array property such as `val first = itemGroup` with
/// `val itemGroup: Array<Holder>` at package scope, and a chain whose
/// terminal is a zero-argument method-call hop returning a single-level
/// array, such as `val x = h.items[0].make()` with
/// `fun make(): Array<Item>` (so a trailing element access such as
/// `x[0].helper(...)` dispatches on the return array's element component
/// type). Intermediate hops walk the same property, array-element, and
/// method-call hop rules as chained receivers, and the terminal hop resolves
/// its element component type so a trailing element access such as
/// `first[0].helper(...)` dispatches on the component type. Non-array
/// terminals, unresolvable roots, bound or member names that shadow the
/// top-level property, bound non-array values, and unknown or ambiguous hops
/// return `None` so callers fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_property_chain_array_component_type_path(
    source_symbol: &IndexedSymbol,
    chain: &str,
    bindings: Option<&KotlinReceiverTypeBindings>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let hops = chain.split('.').collect::<Vec<_>>();
    if hops.is_empty() || hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    // A single-hop chain resolves directly from the bound value when the hop
    // names a locally bound or enclosing-member value declared as a
    // single-level generic array (`val items: Array<Item>`), a name bound
    // from another property chain (`val x = itemGroup` then `val first = x`)
    // or element-access base, or an ambiguous binding; a bound non-array
    // value fails closed because element access on it is invalid. An unbound
    // single hop may instead name a same-package or explicitly imported
    // top-level array property such as `itemGroup` in `val first = itemGroup`
    // with `val itemGroup: Array<Holder>` at package scope.
    if hops.len() == 1 {
        if bindings.is_some_and(|bindings| bindings.contains(hops[0])) {
            if let Some(component_type) = bindings
                .as_ref()
                .and_then(|bindings| bindings.array_component_for(hops[0]))
            {
                return resolve_kotlin_initializer_type_path(
                    source_symbol,
                    &component_type,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                );
            }
            if let Some(type_name) = bindings
                .as_ref()
                .and_then(|bindings| bindings.type_for(hops[0]))
            {
                if let Some(component_name) = kotlin_array_type_component_name(&type_name) {
                    return resolve_kotlin_initializer_type_path(
                        source_symbol,
                        &component_name,
                        raw_symbols,
                        semantic_path_index,
                        file_overrides,
                        kotlin_import_contexts_by_file,
                        deadline,
                    );
                }
                // A name bound from a factory or method-call initializer such
                // as `val x = makeItems()` or `val x = holder.make()` records
                // the callee spelling as its type; when the spelling does not
                // resolve as a type, the element access dispatches through the
                // factory's declared return array's element component type.
                if !type_name.is_empty()
                    && resolve_kotlin_receiver_type_path(
                        source_symbol,
                        &type_name,
                        raw_symbols,
                        file_overrides,
                        kotlin_import_contexts_by_file,
                        deadline,
                    )?
                    .is_none()
                    && let Some((component_path, _)) =
                        resolve_kotlin_factory_array_element_component_type(
                            source_symbol,
                            &type_name,
                            raw_symbols,
                            semantic_path_index,
                            file_overrides,
                            kotlin_import_contexts_by_file,
                            deadline,
                        )?
                {
                    return Ok(Some(component_path));
                }
                return Ok(None);
            }
            if let Some(base) = bindings
                .as_ref()
                .and_then(|bindings| bindings.element_access_base_for(hops[0]))
            {
                return resolve_kotlin_element_access_base_component_type_path(
                    source_symbol,
                    &base,
                    bindings,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                );
            }
            if let Some(chain) = bindings
                .as_ref()
                .and_then(|bindings| bindings.property_chain_base_for(hops[0]))
            {
                return resolve_kotlin_property_chain_array_component_type_path(
                    source_symbol,
                    &chain,
                    bindings,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                );
            }
            return Ok(None);
        }
        if let Some(component_path) = resolve_kotlin_top_level_property_array_component_path(
            source_symbol,
            hops[0],
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some(component_path));
        }
    }
    let Some((mut type_path, skip)) = kotlin_property_chain_initializer_root(
        source_symbol,
        &hops,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let terminal = hops[hops.len() - 1];
    let intermediate_count = hops.len().saturating_sub(skip).saturating_sub(1);
    for hop in hops.iter().skip(skip).take(intermediate_count) {
        let Some(next_path) = kotlin_chained_receiver_hop_type_path(
            &type_path,
            hop,
            source_symbol,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        type_path = next_path;
    }
    // A terminal method-call hop such as `make()` in
    // `val x = h.items[0].make()` resolves the member function's declared
    // single-level array return type through the same factory rules as a
    // factory-call element-access hop, so a trailing element access such as
    // `x[0].helper(...)` dispatches on the return array's element component
    // type; unknown factories, missing return types, and non-array return
    // types fail closed.
    if let Some(method_name) = kotlin_method_call_hop_spelling(terminal) {
        return resolve_kotlin_owner_factory_array_element_component_type(
            &type_path,
            &method_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    kotlin_array_property_component_type_path(
        &type_path,
        terminal,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Resolves a bare member chain inside a member function (an implicit `this`
/// receiver) such as `holder.item.helper(...)` or `holder.helper(...)` where
/// `holder` is a property of the enclosing type or one of its superclasses.
/// The chain dispatches through the same direct, inherited, and extension
/// rules as an explicit `this.`-rooted chain. Callers outside a type and
/// unknown or unresolvable hops and members fail closed because an implicit
/// `this` receiver has no enclosing type to dispatch on.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_implicit_this_member_chain(
    source_symbol: &IndexedSymbol,
    chain: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(this_root) = kotlin_enclosing_this_root(source_symbol, raw_symbols) else {
        return Ok(None);
    };
    resolve_kotlin_type_rooted_member_chain(
        source_symbol,
        this_root,
        chain,
        call_arity,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_chained_receiver_call(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let hops = reference_name.split('.').collect::<Vec<_>>();
    if hops.len() < 3 {
        return Ok(None);
    }
    let bindings = kotlin_receiver_type_bindings_for_function(
        &source_symbol.file_path,
        source_symbol.byte_range,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    // A companion chain such as `Config.Companion.member(...)`,
    // `Config.Factory.member(...)`, or `Config.Companion.holder.run(...)`
    // resolves the class name as a type path and dispatches only within the
    // companion scope; instance members and extensions fail closed because a
    // class name cannot be an instance receiver. The companion may also be
    // hosted by a nested class or interface, so `Outer.Inner.helper(...)`,
    // `Outer.Inner.Companion.helper(...)`, and
    // `Outer.Inner.Factory.holder.run(...)` dispatch through the nested
    // type's canonical companion scope. A local binding of the same name
    // shadows the class receiver.
    let companion_chain = kotlin_companion_chain_root(
        source_symbol,
        &hops,
        bindings.as_ref(),
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    if let Some((companion_root, consumed)) = companion_chain {
        let companion_scope = format!("{companion_root}::Companion");
        let mut receiver_path = companion_scope.clone();
        for hop in hops.iter().skip(consumed).take(hops.len() - consumed - 1) {
            let Some(next_path) = kotlin_chained_receiver_hop_type_path(
                &receiver_path,
                hop,
                source_symbol,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            receiver_path = next_path;
        }
        let method = hops[hops.len() - 1];
        if receiver_path == companion_scope {
            // A direct companion member call never falls through to extensions.
            return Ok(resolve_kotlin_companion_member(
                &companion_root,
                method,
                call_arity,
                raw_symbols,
                semantic_path_index,
            ));
        }
        let type_name = receiver_path
            .rsplit("::")
            .next()
            .unwrap_or(method)
            .to_string();
        return resolve_kotlin_member_or_extension(
            source_symbol,
            &receiver_path,
            &type_name,
            method,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    // A nested object receiver such as `Outer.Inner.helper(...)` or
    // `Outer.Inner.holder.run(...)` resolves the first hop as a type path and
    // requires the second hop to name exactly one nested object declaration.
    // A local binding of the first-hop name shadows the class receiver, and a
    // nested object that shares its name with a nested class or interface
    // fails closed instead of guessing a target.
    if hops.len() >= 3
        && !bindings
            .as_ref()
            .is_some_and(|bindings| bindings.contains(hops[0]))
        && let Some(class_path) = resolve_kotlin_receiver_type_path(
            source_symbol,
            hops[0],
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        && let Some(nested_object_path) =
            kotlin_path_nested_object(&class_path, hops[1], raw_symbols)
    {
        let mut receiver_path = nested_object_path;
        for hop in hops.iter().skip(2).take(hops.len() - 3) {
            let Some(next_path) = kotlin_chained_receiver_hop_type_path(
                &receiver_path,
                hop,
                source_symbol,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            receiver_path = next_path;
        }
        let method = hops[hops.len() - 1];
        let type_name = receiver_path
            .rsplit("::")
            .next()
            .unwrap_or(method)
            .to_string();
        return resolve_kotlin_member_or_extension(
            source_symbol,
            &receiver_path,
            &type_name,
            method,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    // The first hop is either a locally bound receiver (including a `val`
    // bound from a qualified element-access initializer whose terminal array
    // element component type starts the chain), an element-access receiver
    // whose base is bound with a single-level array component type or is a
    // factory call whose declared return type is a single-level array, or a
    // named object declaration such as `Config` in `Config.holder.run()`. An
    // ambiguous local binding fails closed instead of falling through to a
    // same-named object.
    let mut type_path = if bindings
        .as_ref()
        .is_some_and(|bindings| bindings.contains(hops[0]))
    {
        if let Some(type_name) = bindings
            .as_ref()
            .and_then(|bindings| bindings.type_for(hops[0]))
        {
            let Some(path) = resolve_kotlin_initializer_type_path(
                source_symbol,
                &type_name,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            path
        } else if let Some(base) = bindings
            .as_ref()
            .and_then(|bindings| bindings.element_access_base_for(hops[0]))
            && let Some(component_path) = resolve_kotlin_element_access_base_component_type_path(
                source_symbol,
                &base,
                bindings.as_ref(),
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
        {
            // A `val` bound from a qualified element-access initializer such
            // as `val first = makeItems()[0]` or
            // `val first = group.fieldItems[0]` has no usable type until the
            // base's terminal array element component type is resolved; start
            // the chain on that component type so trailing hops dispatch
            // through the same chain rules.
            component_path
        } else if let Some(chain) = bindings
            .as_ref()
            .and_then(|bindings| bindings.property_chain_base_for(hops[0]))
            && let Some(path) = resolve_kotlin_property_chain_initializer_type_path(
                source_symbol,
                &chain,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
        {
            // A `val` local bound from a property-chain initializer such as
            // `val first = holder.item` starts the chain on the initializer's
            // terminal property type.
            path
        } else {
            return Ok(None);
        }
    } else if let Some((base_name, _)) = kotlin_array_access_spelling(hops[0])
        && let Some(component_type) = bindings
            .as_ref()
            .and_then(|bindings| bindings.array_component_for(base_name))
        && let Some(path) = resolve_kotlin_initializer_type_path(
            source_symbol,
            &component_type,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    {
        path
    } else if let Some((base_name, _)) = kotlin_array_access_spelling(hops[0])
        && let Some(chain) = bindings
            .as_ref()
            .and_then(|bindings| bindings.property_chain_base_for(base_name))
        && let Some(component_path) = resolve_kotlin_property_chain_array_component_type_path(
            source_symbol,
            &chain,
            bindings.as_ref(),
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    {
        // A `val` local bound from a property-chain initializer whose
        // terminal property is a single-level array such as
        // `val first = holder.items` starts the chain on the terminal array's
        // element component type so trailing element-access and member hops
        // dispatch through the same chain rules; unknown chains, non-array
        // terminals, and unresolvable hops fail closed.
        component_path
    } else if let Some((base_name, _)) = kotlin_array_access_spelling(hops[0])
        && let Some(initializer_name) = bindings
            .as_ref()
            .and_then(|bindings| bindings.type_for(base_name))
        && !initializer_name.is_empty()
        && resolve_kotlin_receiver_type_path(
            source_symbol,
            &initializer_name,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        .is_none()
        && let Some((component_path, _)) = resolve_kotlin_factory_array_element_component_type(
            source_symbol,
            &initializer_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    {
        component_path
    } else if let Some((factory_base, _)) = kotlin_array_access_spelling(hops[0])
        && let Some(function_name) = kotlin_array_factory_call_root_spelling(factory_base)
        && let Some((component_path, _)) = resolve_kotlin_factory_array_element_component_type(
            source_symbol,
            &function_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    {
        component_path
    } else if let Some(object_path) = resolve_kotlin_object_receiver_path(
        source_symbol,
        hops[0],
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        object_path
    } else if let Some((companion_root, _)) = resolve_kotlin_anonymous_companion_chain_root(
        source_symbol,
        &hops,
        bindings.as_ref(),
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        // An unbound first hop names a class whose terminal array property
        // lives on its anonymous companion, the Kotlin analog of a static
        // field such as `Util.items` in `Util.items[0].helper(...)`; start the
        // chain on the canonical companion scope so the element-access hop
        // resolves the component type.
        companion_root
    } else if hops.len() >= 3
        && let Some((factory_hop, _)) = kotlin_array_access_spelling(hops[1])
        && kotlin_array_factory_call_root_spelling(factory_hop).is_some()
        && let Some(class_path) = resolve_kotlin_receiver_type_path(
            source_symbol,
            hops[0],
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    {
        class_path
    } else if let Some(target) = resolve_kotlin_implicit_this_member_chain(
        source_symbol,
        reference_name,
        call_arity,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        // An unbound first hop may also be a property of the enclosing type
        // (an implicit `this` receiver), including an inherited property such
        // as `holder` in `holder.item.helper(...)` inside a subclass; the
        // whole chain dispatches through the same direct and inherited rules
        // as an explicit `this.`-rooted chain. Unknown properties and members
        // fail closed.
        return Ok(Some(target));
    } else {
        return Ok(None);
    };
    // Each intermediate hop must resolve to a uniquely declared property whose
    // explicit type pins the next receiver, to a method-call hop such as
    // `inner()` whose declared return type continues the chain, or to a
    // factory-call element-access hop such as `makeGroups()[0]` whose declared
    // single-level array return type pins the element component type; a bare
    // constructor initializer such as `val member = Other()` also pins the
    // type, while generic, nullable, function-call-inferred, ambiguous, and
    // missing hops fail closed.
    for hop in hops.iter().skip(1).take(hops.len() - 2) {
        let Some(next_path) = kotlin_chained_receiver_hop_type_path(
            &type_path,
            hop,
            source_symbol,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        type_path = next_path;
    }
    let method = hops[hops.len() - 1];
    let type_name = type_path.rsplit("::").next().unwrap_or(method).to_string();
    resolve_kotlin_member_or_extension(
        source_symbol,
        &type_path,
        &type_name,
        method,
        call_arity,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Walks `chain` as a member chain rooted at `root_type_path`, resolving each
/// intermediate hop as a uniquely declared property whose declared type pins
/// the next receiver, or as a method-call hop such as `inner()` whose declared
/// return type continues the chain, and dispatching the final member or
/// extension on the terminal type. A chain with no intermediate hops
/// dispatches the final member directly on the root type. Unknown or
/// unresolvable hops and missing members fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_type_rooted_member_chain(
    source_symbol: &IndexedSymbol,
    root_type_path: &str,
    chain: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let hops = chain.split('.').collect::<Vec<_>>();
    if hops.is_empty() || hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    let mut receiver_path = root_type_path.to_string();
    for hop in hops.iter().take(hops.len() - 1) {
        let Some(next_path) = kotlin_chained_receiver_hop_type_path(
            &receiver_path,
            hop,
            source_symbol,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        receiver_path = next_path;
    }
    let method = hops[hops.len() - 1];
    let type_name = receiver_path
        .rsplit("::")
        .next()
        .unwrap_or(method)
        .to_string();
    resolve_kotlin_member_or_extension(
        source_symbol,
        &receiver_path,
        &type_name,
        method,
        call_arity,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Parses a method-call hop spelling such as `inner()` into the method name.
/// Property hops, element-access hops, and malformed spellings return `None`
/// so they fall through to property resolution and fail closed when no such
/// property exists.
fn kotlin_method_call_hop_spelling(hop: &str) -> Option<String> {
    let open = hop.find('(')?;
    let (method_name, rest) = hop.split_at(open);
    if method_name.is_empty() || method_name.contains('.') || rest != "()" {
        return None;
    }
    Some(method_name.to_string())
}

/// Resolves the symbol path of a uniquely declared member or extension
/// function named `method_name` on `owner_type_path` with a declared return
/// type. Direct members win; an interface-typed owner falls back to a parent
/// interface in its extends chain, a class-typed owner to a parent class in
/// its direct superclass chain or an implemented interface, and finally to a
/// unique top-level extension function for the receiver type resolved in the
/// caller's file, package, or explicit imports. Member functions shadow
/// extensions, and an ambiguous overload or extension set, blocked chains,
/// and unknown members fail closed as `None`.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_member_function_path(
    owner_type_path: &str,
    method_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if method_name.is_empty() {
        return Ok(None);
    }
    let target_path = format!("{owner_type_path}::{method_name}");
    let member_candidates = semantic_path_index
        .get(&target_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "function_declaration"
                && candidate.scope_path.as_deref() == Some(owner_type_path)
                && candidate.return_type.is_some()
        })
        .collect::<Vec<_>>();
    if member_candidates.len() == 1 {
        return Ok(Some(raw_symbols[member_candidates[0]].symbol_id.clone()));
    }
    if !member_candidates.is_empty() {
        return Ok(None);
    }
    // An interface-typed owner dispatches a member function declared on a
    // parent interface in its extends chain before falling back to
    // extensions; inherited interface members shadow extensions, and blocked
    // chains fail closed instead of guessing an extension target.
    match resolve_kotlin_inherited_member_index(
        owner_type_path,
        method_name,
        "function_declaration",
        None,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        KotlinInheritedMemberResolution::Resolved(index) => {
            return Ok(Some(raw_symbols[index].symbol_id.clone()));
        }
        KotlinInheritedMemberResolution::Blocked => return Ok(None),
        KotlinInheritedMemberResolution::NoMember => {}
    }
    // A class-typed owner dispatches a member function declared on a parent
    // class in its direct superclass chain, or on an implemented interface
    // when the class hierarchy does not declare it; inherited class and
    // interface members shadow extensions, and ambiguous chains fail closed.
    let inherited_member = resolve_kotlin_superclass_chain_member(
        owner_type_path,
        method_name,
        "function_declaration",
        None,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    let inherited_member = match inherited_member {
        Some(index) => Some(index),
        None => resolve_kotlin_class_receiver_interface_member(
            owner_type_path,
            method_name,
            "function_declaration",
            None,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?,
    };
    if let Some(index) = inherited_member {
        return Ok(Some(raw_symbols[index].symbol_id.clone()));
    }
    // An extension fallback requires a unique top-level extension function for
    // the receiver type with a declared return type, resolved in the caller's
    // file, package, or explicit imports like any other extension; ambiguous
    // extension sets fail closed.
    let owner_type_name = owner_type_path
        .rsplit("::")
        .next()
        .unwrap_or(owner_type_path);
    let package_scope = kotlin_package_scope(source_symbol, raw_symbols);
    let imported_binding = resolve_kotlin_import_binding_for_reference(
        &source_symbol.file_path,
        method_name,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    let extension_candidates = raw_symbols
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            candidate.node_kind == "function_declaration"
                && candidate.extension_receiver.as_deref() == Some(owner_type_name)
                && candidate.base_name == method_name
                && candidate.return_type.is_some()
                && kotlin_symbol_is_top_level(candidate, raw_symbols)
                && (candidate.file_path == source_symbol.file_path
                    || package_scope
                        .is_some_and(|scope| candidate.scope_path.as_deref() == Some(scope))
                    || imported_binding
                        .as_ref()
                        .is_some_and(|binding| binding.semantic_path == candidate.semantic_path))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    Ok((extension_candidates.len() == 1)
        .then(|| raw_symbols[extension_candidates[0]].symbol_id.clone()))
}

/// Resolves the declared return type path of a method-call hop such as
/// `inner()` in `group.inner().entry.helper(...)`. The hop dispatches as a
/// unique member or extension function on the receiver type with a declared
/// return type (member functions shadow extensions, and an ambiguous overload
/// or extension set fails closed); the declared return type then resolves in
/// the method's own file and enclosing scope. Unknown, ambiguous, or
/// undeclared-return hops fail closed.
#[allow(clippy::too_many_arguments)]
fn kotlin_method_call_hop_type_path(
    owner_type_path: &str,
    method_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if method_name.is_empty() {
        return Ok(None);
    }
    let Some(method_path) = resolve_kotlin_member_function_path(
        owner_type_path,
        method_name,
        source_symbol,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(method) = raw_symbols
        .iter()
        .find(|candidate| candidate.symbol_id == method_path)
    else {
        return Ok(None);
    };
    let Some(return_type) = method
        .return_type
        .as_deref()
        .and_then(kotlin_dotted_type_name)
    else {
        return Ok(None);
    };
    resolve_kotlin_receiver_type_path(
        method,
        &return_type,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Resolves the next receiver type path for one chain hop. A method-call hop
/// such as `inner()` dispatches on the declared return type of a unique member
/// or extension function; an array-property element-access hop such as
/// `groups[0]` resolves the base name as a uniquely declared single-level
/// array property and continues on the element component type; a hop naming a
/// uniquely declared nested type such as `Nested` in `Outer.Nested.make()`
/// continues on the nested type's own scope; any other hop must resolve as a
/// uniquely declared property whose declared type or bare constructor
/// initializer pins the next receiver. Factory-call element-access hops such
/// as `makeGroups()[0]` are resolved by the chained-hop wrapper before this
/// fallback and remain unsupported in the receiver-chain and
/// constructor-receiver contexts. Unknown or ambiguous hops fail closed.
#[allow(clippy::too_many_arguments)]
fn kotlin_chain_hop_type_path(
    owner_type_path: &str,
    hop: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if let Some(method_name) = kotlin_method_call_hop_spelling(hop) {
        if let Some(path) = kotlin_method_call_hop_type_path(
            owner_type_path,
            &method_name,
            source_symbol,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some(path));
        }
        // A `()`-marked hop that is not a member function may construct a
        // uniquely declared nested class, such as `Nested()` in
        // `Outer.Nested().make()` or `Inner()` in
        // `Outer.Nested.Inner().make()`; the constructed nested class path
        // continues the chain. Unknown, ambiguous, and non-constructible
        // nested classes fail closed.
        if let Some(nested_path) = kotlin_constructor_hop_path(
            owner_type_path,
            &method_name,
            raw_symbols,
            semantic_path_index,
        ) {
            return Ok(Some(nested_path));
        }
        return Ok(None);
    }
    if let Some((base_name, _)) = kotlin_array_access_spelling(hop)
        && !base_name.contains('(')
    {
        return kotlin_array_property_component_type_path(
            owner_type_path,
            base_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    if let Ok(Some(path)) = kotlin_property_type_path(
        owner_type_path,
        hop,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    ) {
        return Ok(Some(path));
    }
    // A hop naming a uniquely declared nested type such as `Nested` in
    // `Outer.Nested.make()` or `DeepOuter.Mid.Inner.make()` continues on the
    // nested type's own scope the same way dotted type paths walk nested
    // declarations, so nested objects, classes, and interfaces resolve as
    // receiver-chain hops; ambiguous and missing nested types fail closed.
    if let Some(nested_path) = kotlin_nested_type_hop_path(owner_type_path, hop, raw_symbols) {
        return Ok(Some(nested_path));
    }
    Ok(None)
}

/// Resolves a dotted receiver prefix such as `group`, `group.holder`,
/// `items[0]`, or `Config.Factory.groups[0]` to the terminal receiver type
/// path. The leading receiver resolves through the local bindings (parameter,
/// local property, enclosing-class property, or element-access base), a
/// companion-chain root, or a named object declaration; each following hop
/// must resolve as a property, array-element, or method-call hop. Unknown or
/// ambiguous receivers and hops fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_receiver_chain_type_path(
    source_symbol: &IndexedSymbol,
    chain: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let hops = chain.split('.').collect::<Vec<_>>();
    if hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    let bindings = kotlin_receiver_type_bindings_for_function(
        &source_symbol.file_path,
        source_symbol.byte_range,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    let Some((mut type_path, skip)) = resolve_kotlin_receiver_chain_first_path(
        source_symbol,
        &hops,
        bindings.as_ref(),
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    for hop in hops.iter().skip(skip) {
        let Some(next_path) = kotlin_chain_hop_type_path(
            &type_path,
            hop,
            source_symbol,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        type_path = next_path;
    }
    Ok(Some(type_path))
}

/// Resolves the leading receiver of a dotted receiver prefix to a type path
/// and the number of leading hops that path already covers. A bound receiver
/// resolves through its declared or element-access-inferred type; a
/// companion-chain root such as `Config.Factory` or `Outer.Inner.Companion`
/// resolves to the canonical `{type}::Companion` scope and consumes its
/// leading hops; otherwise the first hop must be a named object declaration
/// or a locally bound array component. Unknown or ambiguous receivers fail
/// closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_receiver_chain_first_path(
    source_symbol: &IndexedSymbol,
    hops: &[&str],
    bindings: Option<&KotlinReceiverTypeBindings>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, usize)>> {
    if bindings
        .as_ref()
        .is_some_and(|bindings| bindings.contains(hops[0]))
    {
        // A name bound from an `if`/`when` expression initializer such as
        // `val group = if (flag) h.make() else Holder().make()` resolves all
        // branch spellings to a common declared type path before any other
        // bound-receiver inference; divergent or unresolvable branches fail
        // closed.
        if let Some(bindings) = bindings.as_ref()
            && let Some(path) = resolve_kotlin_branch_initializer_type_path(
                source_symbol,
                hops[0],
                bindings,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
        {
            return Ok(Some((path, 1)));
        }
        if let Some(type_name) = bindings
            .as_ref()
            .and_then(|bindings| bindings.type_for(hops[0]))
        {
            let Some(path) = resolve_kotlin_initializer_type_path(
                source_symbol,
                &type_name,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
            else {
                return Ok(None);
            };
            return Ok(Some((path, 1)));
        }
        if let Some(base) = bindings
            .as_ref()
            .and_then(|bindings| bindings.element_access_base_for(hops[0]))
            && let Some(component_path) = resolve_kotlin_element_access_base_component_type_path(
                source_symbol,
                &base,
                bindings,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
        {
            return Ok(Some((component_path, 1)));
        }
        if let Some(chain) = bindings
            .as_ref()
            .and_then(|bindings| bindings.property_chain_base_for(hops[0]))
            && let Some(path) = resolve_kotlin_property_chain_initializer_type_path(
                source_symbol,
                &chain,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?
        {
            return Ok(Some((path, 1)));
        }
        return Ok(None);
    }
    if let Some((base_name, _)) = kotlin_array_access_spelling(hops[0])
        && let Some(component_type) = bindings
            .as_ref()
            .and_then(|bindings| bindings.array_component_for(base_name))
        && let Some(path) = resolve_kotlin_initializer_type_path(
            source_symbol,
            &component_type,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    {
        return Ok(Some((path, 1)));
    }
    if let Some((base_name, _)) = kotlin_array_access_spelling(hops[0])
        && let Some(chain) = bindings
            .as_ref()
            .and_then(|bindings| bindings.property_chain_base_for(base_name))
        && let Some(component_path) = resolve_kotlin_property_chain_array_component_type_path(
            source_symbol,
            &chain,
            bindings,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    {
        // A `val` local bound from a property-chain initializer whose
        // terminal property is a single-level array such as
        // `val first = holder.items` starts the chain on the terminal array's
        // element component type so a trailing element access such as
        // `first[0].inner().helper(...)` dispatches through the same chain
        // rules; unknown chains, non-array terminals, and unresolvable hops
        // fail closed.
        return Ok(Some((component_path, 1)));
    }
    // A `val` local initialized from a factory call whose declared return
    // type is a single-level array, such as `val items = this.ownMake()` or
    // `val items = Util.makeItems()`, dispatches an element access on the
    // array's element component type through the same factory rules as a
    // direct factory-call element-access receiver; unknown factories and
    // non-array return types fail closed.
    if let Some((base_name, _)) = kotlin_array_access_spelling(hops[0])
        && let Some(initializer_name) = bindings
            .as_ref()
            .and_then(|bindings| bindings.type_for(base_name))
        && !initializer_name.is_empty()
        && resolve_kotlin_receiver_type_path(
            source_symbol,
            &initializer_name,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        .is_none()
        && let Some((component_path, _)) = resolve_kotlin_factory_array_element_component_type(
            source_symbol,
            &initializer_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    {
        return Ok(Some((component_path, 1)));
    }
    // A leading constructor-call hop such as `Holder()` in
    // `val group = Holder().make()` resolves to the uniquely constructible
    // class path and consumes one hop so the terminal method-call hop
    // dispatches on the constructed type; unknown, ambiguous, and
    // non-constructible names fail closed.
    if let Some(constructor_name) = kotlin_method_call_hop_spelling(hops[0])
        && let Some(type_path) = resolve_kotlin_receiver_type_path(
            source_symbol,
            &constructor_name,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        && kotlin_constructible_class_indexes(&type_path, raw_symbols, semantic_path_index).len()
            == 1
    {
        return Ok(Some((type_path, 1)));
    }
    // A nested constructor-call chain root such as `Outer.Nested()` in
    // `val group = Outer.Nested().make()` or `Outer.Nested.Inner()` in
    // `val group = Outer.Nested.Inner().make()` resolves the first hop as a
    // type path, walks each following non-`()` hop through uniquely declared
    // nested types, and constructs the first `()`-marked hop as a uniquely
    // constructible nested class, consuming all hops up to and including the
    // constructor hop so the remaining chain dispatches on the constructed
    // nested type. Unknown, ambiguous, and non-constructible nested classes
    // and unresolvable intermediate hops return `None` so the companion-chain
    // and object roots below still resolve `Outer.Nested.make()` and
    // `Outer.Nested` chains.
    if let Some((nested_path, consumed)) = resolve_kotlin_nested_constructor_chain_root(
        source_symbol,
        hops,
        bindings,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        return Ok(Some((nested_path, consumed)));
    }
    if let Some((companion_root, consumed)) = kotlin_companion_chain_root(
        source_symbol,
        hops,
        bindings,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        return Ok(Some((format!("{companion_root}::Companion"), consumed)));
    }
    // A nested object chain root such as `Outer.Nested` in
    // `Outer.Nested.make()` resolves the first hop as a type path and the
    // second hop as exactly one nested object declaration (nested objects are
    // not companion chains, so the companion-chain root above does not cover
    // them); the nested object scope consumes two hops so the remaining chain
    // walks from there. A local binding of the first-hop name shadows the type,
    // and a nested object that shares its name with a nested class or
    // interface fails closed instead of guessing a target.
    if hops.len() >= 2
        && !bindings
            .as_ref()
            .is_some_and(|bindings| bindings.contains(hops[0]))
        && let Some(class_path) = resolve_kotlin_receiver_type_path(
            source_symbol,
            hops[0],
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        && let Some(nested_object_path) =
            kotlin_path_nested_object(&class_path, hops[1], raw_symbols)
    {
        return Ok(Some((nested_object_path, 2)));
    }
    if let Some(object_path) = resolve_kotlin_object_receiver_path(
        source_symbol,
        hops[0],
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        return Ok(Some((object_path, 1)));
    }
    if let Some((companion_root, consumed)) = resolve_kotlin_anonymous_companion_chain_root(
        source_symbol,
        hops,
        bindings,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        return Ok(Some((companion_root, consumed)));
    }
    Ok(None)
}

/// Resolves the index of a `property_declaration` named `property_name` that
/// an `owner_type_path` instance inherits: a parent-interface member in the
/// interface extends chain, a parent-class member in the direct superclass
/// chain, or an implemented-interface member when the class hierarchy does not
/// declare it. Ambiguous, blocked, cyclic, or unresolvable chains fail closed
/// as `None` so property and array-property hops never guess an inherited
/// target.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_inherited_property_index(
    owner_type_path: &str,
    property_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<usize>> {
    match resolve_kotlin_inherited_member_index(
        owner_type_path,
        property_name,
        "property_declaration",
        None,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        KotlinInheritedMemberResolution::Resolved(index) => Ok(Some(index)),
        KotlinInheritedMemberResolution::Blocked => Ok(None),
        KotlinInheritedMemberResolution::NoMember => {
            let inherited_member = resolve_kotlin_superclass_chain_member(
                owner_type_path,
                property_name,
                "property_declaration",
                None,
                raw_symbols,
                semantic_path_index,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?;
            let inherited_member = match inherited_member {
                Some(index) => Some(index),
                None => resolve_kotlin_class_receiver_interface_member(
                    owner_type_path,
                    property_name,
                    "property_declaration",
                    None,
                    raw_symbols,
                    semantic_path_index,
                    file_overrides,
                    kotlin_import_contexts_by_file,
                    deadline,
                )?,
            };
            Ok(inherited_member)
        }
    }
}

/// Resolves the declared type path of `property_name` on `owner_type_path`. A
/// unique property resolves when it carries an explicit simple, non-nullable
/// type or a bare constructor initializer; the declared type resolves in the
/// property's own file and enclosing scope. Generic, nullable, function-call
/// inferred, and ambiguous property types fail closed.
#[allow(clippy::too_many_arguments)]
fn kotlin_property_type_path(
    owner_type_path: &str,
    property_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if property_name.is_empty() {
        return Ok(None);
    }
    let candidates = semantic_path_index
        .get(&format!("{owner_type_path}::{property_name}"))
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "property_declaration"
                && candidate.scope_path.as_deref() == Some(owner_type_path)
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        let Some(index) = resolve_kotlin_inherited_property_index(
            owner_type_path,
            property_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let Some(return_type) = raw_symbols[index].return_type.as_deref() else {
            return Ok(None);
        };
        let Some(type_name) = kotlin_dotted_type_name(return_type) else {
            return Ok(None);
        };
        return resolve_kotlin_initializer_type_path(
            &raw_symbols[index],
            &type_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        );
    }
    if candidates.len() != 1 {
        return Ok(None);
    }
    let Some(return_type) = raw_symbols[candidates[0]].return_type.as_deref() else {
        return Ok(None);
    };
    let Some(type_name) = kotlin_dotted_type_name(return_type) else {
        return Ok(None);
    };
    resolve_kotlin_initializer_type_path(
        &raw_symbols[candidates[0]],
        &type_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Resolves an inferred receiver/initializer name such as `Other` in
/// `val member = Other()` or `makeOther` in `val member = makeOther()` to a
/// type path. A directly declared type wins; otherwise a uniquely resolved
/// top-level function's declared return type pins the receiver, resolved in the
/// factory's own file and package scope. A dotted return type such as
/// `Outer.Inner` pins the receiver through the same dotted type-path rules as
/// a directly declared nested type. Unknown, ambiguous, and undeclared-return
/// factories fail closed so receivers never guess a target.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_initializer_type_path(
    source_symbol: &IndexedSymbol,
    initializer_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if let Some(path) = resolve_kotlin_receiver_type_path(
        source_symbol,
        initializer_name,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        return Ok(Some(path));
    }
    // A `this`- or `super`-rooted factory-call initializer such as
    // `val value = this.make()` or `val value = super.make()` resolves the
    // callee as a member function on the enclosing type or the direct
    // superclass and pins the receiver to its declared return type, resolved
    // in the method's own file and enclosing scope; unknown or ambiguous
    // member functions fail closed. Chained call receivers such as
    // `val group = this.make().make()` or `val group = super.make().make()`
    // fall back to the property-chain initializer rules, which start on the
    // enclosing type or direct superclass and walk each intermediate
    // method-call hop before the terminal callee; unknown or unresolvable
    // chains fail closed there as well.
    if initializer_name.starts_with("this.") || initializer_name.starts_with("super.") {
        if let Some(function_path) = resolve_kotlin_this_super_rooted_member_function_path(
            source_symbol,
            initializer_name,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            let Some(function) = raw_symbols
                .iter()
                .find(|candidate| candidate.symbol_id == function_path)
            else {
                return Ok(None);
            };
            let Some(function_return_type) = function
                .return_type
                .as_deref()
                .and_then(kotlin_dotted_type_name)
            else {
                return Ok(None);
            };
            return resolve_kotlin_receiver_type_path(
                function,
                &function_return_type,
                raw_symbols,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            );
        }
        // A chained `this`- or `super`-rooted call receiver keeps its trailing
        // callee without a `()` marker in the callee spelling, so the terminal
        // hop is marked as a call here before the property-chain walk.
        let chain = format!("{initializer_name}()");
        if let Some(path) = resolve_kotlin_property_chain_initializer_type_path(
            source_symbol,
            &chain,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some(path));
        }
        return Ok(None);
    }
    // A dotted factory-call initializer such as `val group = h.make()`,
    // `val group = Factory.make()`, or `val group = Holder.make()`, including
    // chained method-call receivers such as `val group = h.make().make()`,
    // resolves the leading receiver through the same rules as a receiver
    // chain (a locally bound value, a named object, a type with a companion
    // object, or a nested receiver chain) and dispatches the terminal callee
    // as a method-call hop on that receiver, pinning the binding to the
    // method's declared return type resolved in the method's own file and
    // enclosing scope. Unknown receivers, ambiguous or unknown member
    // functions, and functions without a declared return type fail closed.
    if let Some((receiver, method)) = initializer_name.rsplit_once('.')
        && !receiver.is_empty()
        && !method.is_empty()
        && !method.contains('(')
        && !method.contains('[')
    {
        let callee_chain = format!("{receiver}.{method}()");
        if let Some(receiver_path) = resolve_kotlin_receiver_chain_type_path(
            source_symbol,
            &callee_chain,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            return Ok(Some(receiver_path));
        }
    }
    let Some(function_path) = resolve_kotlin_property_initializer_function_path(
        source_symbol,
        initializer_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(factory) = raw_symbols
        .iter()
        .find(|candidate| candidate.symbol_id == function_path)
    else {
        return Ok(None);
    };
    let Some(function_return_type) = factory
        .return_type
        .as_deref()
        .and_then(kotlin_dotted_type_name)
    else {
        return Ok(None);
    };
    resolve_kotlin_receiver_type_path(
        factory,
        &function_return_type,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Resolves the common declared type path of a name bound from an `if`/`when`
/// expression initializer such as `val group = if (flag) h.make() else
/// Holder().make()`. Every branch spelling resolves through the same rules as
/// a bound initializer (a directly declared type or dotted factory call
/// first, then a property-chain initializer walk), and all branches must
/// resolve to the same type path; a branch that cannot resolve or branches
/// that disagree fail closed so callers never guess a target from a
/// divergent `if`/`when` expression.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_branch_initializer_type_path(
    source_symbol: &IndexedSymbol,
    name: &str,
    bindings: &KotlinReceiverTypeBindings,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(branches) = bindings.branch_initializers_for(name) else {
        return Ok(None);
    };
    let mut common_path = None;
    for spelling in branches {
        let Some(path) = resolve_kotlin_initializer_type_path(
            source_symbol,
            &spelling,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        .or(resolve_kotlin_property_chain_initializer_type_path(
            source_symbol,
            &spelling,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?) else {
            return Ok(None);
        };
        if let Some(existing) = common_path.as_ref()
            && existing != &path
        {
            return Ok(None);
        }
        common_path = Some(path);
    }
    Ok(common_path)
}

/// Resolves an inferred property initializer callee such as `makeOther` in
/// `val derived = makeOther()` to a unique function whose declared return
/// type can pin the property receiver. Same-file, same-package, and explicitly
/// imported top-level functions are eligible; when none matches, a member
/// function of the enclosing type such as `make` in `val derived = make()`
/// inside a class that declares `fun make()` resolves as an implicit `this`
/// receiver through the same rules as a `this`-rooted call, and then a
/// companion member function of the enclosing type such as `make` in a class
/// whose companion declares `fun make()` resolves through the enclosing
/// type's canonical companion scope the same way an unqualified companion
/// property does. Unknown names, ambiguous candidates, and functions without
/// a declared return type fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_property_initializer_function_path(
    source_symbol: &IndexedSymbol,
    function_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let package_scope = kotlin_package_scope(source_symbol, raw_symbols);
    let imported_binding = resolve_kotlin_import_binding_for_reference(
        &source_symbol.file_path,
        function_name,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    let candidates = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.node_kind == "function_declaration"
                && candidate.base_name == function_name
                && candidate.return_type.is_some()
                && kotlin_symbol_is_top_level(candidate, raw_symbols)
                && (candidate.file_path == source_symbol.file_path
                    || package_scope
                        .is_some_and(|scope| candidate.scope_path.as_deref() == Some(scope))
                    || imported_binding
                        .as_ref()
                        .is_some_and(|binding| binding.semantic_path == candidate.semantic_path))
        })
        .map(|candidate| candidate.symbol_id.clone())
        .collect::<Vec<_>>();
    if candidates.len() == 1 {
        return Ok(Some(candidates[0].clone()));
    }
    if !candidates.is_empty() {
        return Ok(None);
    }
    // A bare callee may also be a member function of the enclosing type,
    // visible unqualified inside member functions as an implicit `this`
    // receiver; resolve it through the same rules as a `this`-rooted call and
    // prefer it over a same-named companion member. Callers outside a type and
    // unknown or ambiguous member functions fail closed.
    if !function_name.contains('.')
        && let Some(member_path) = resolve_kotlin_this_super_rooted_member_function_path(
            source_symbol,
            &format!("this.{function_name}"),
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
    {
        return Ok(Some(member_path));
    }
    // A bare callee may also be an implicit companion member function of the
    // enclosing type, visible unqualified inside member functions; resolve it
    // through the enclosing type's canonical companion scope when no
    // top-level, imported, or enclosing-type member function matches. Callers
    // outside a type, types without a companion object, and ambiguous
    // companion members fail closed.
    if let Some(companion_scope) =
        resolve_kotlin_enclosing_companion_scope(source_symbol, raw_symbols, semantic_path_index)
    {
        let companion_candidates = semantic_path_index
            .get(&format!("{companion_scope}::{function_name}"))
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.node_kind == "function_declaration"
                    && candidate.scope_path.as_deref() == Some(companion_scope.as_str())
                    && candidate.return_type.is_some()
            })
            .collect::<Vec<_>>();
        if let [candidate_index] = companion_candidates.as_slice() {
            return Ok(Some(raw_symbols[*candidate_index].symbol_id.clone()));
        }
    }
    Ok(None)
}

/// Returns whether a Kotlin type declaration is an interface by inspecting its
/// stored signature keyword (`interface`, including `fun interface`). Classes,
/// objects, type aliases, and other declarations return `false`.
fn kotlin_type_is_interface(symbol: &IndexedSymbol) -> bool {
    if symbol.node_kind != "class_declaration" {
        return false;
    }
    let Some(signature) = symbol.signature.as_deref() else {
        return false;
    };
    let tokens = signature.split_whitespace().collect::<Vec<_>>();
    let Some(keyword_position) = tokens
        .iter()
        .position(|token| *token == "class" || *token == "interface")
    else {
        return false;
    };
    tokens[keyword_position] == "interface"
}

/// Returns the directly extended super-interface type spellings of a Kotlin
/// interface declaration by locating its `delegation_specifiers` clause and
/// extracting each specifier's pure dotted type spelling. Interfaces without a
/// supertype clause return `Some(empty)`; non-interface declarations and any
/// specifier without a usable dotted spelling (delegation `by`, annotations,
/// or otherwise complex shapes) fail closed as `None`.
fn kotlin_direct_interface_parent_spellings(
    source_interface: &IndexedSymbol,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<Vec<String>>> {
    if !kotlin_type_is_interface(source_interface) {
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
            deadline.check("locating Kotlin parent interface")?;
        }
        if node.kind() == "class_declaration"
            && (node.start_byte(), node.end_byte()) == source_interface.byte_range
        {
            let mut cursor = node.walk();
            let Some(specifiers) = node
                .named_children(&mut cursor)
                .find(|child| child.kind() == "delegation_specifiers")
            else {
                return Ok(Some(Vec::new()));
            };
            let mut specifier_cursor = specifiers.walk();
            let mut spellings = Vec::new();
            for specifier in specifiers.named_children(&mut specifier_cursor) {
                let Some(spelling) = kotlin_delegation_specifier_type_name(specifier, &source)?
                else {
                    return Ok(None);
                };
                spellings.push(spelling);
            }
            return Ok(Some(spellings));
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    Ok(None)
}

enum KotlinInheritedMemberResolution {
    Resolved(usize),
    NoMember,
    Blocked,
}

/// Walks an interface's direct super-interface branches recursively to resolve
/// `member_name` of the given node kind with an optional exact arity match.
/// Exactly one branch must provide a uniquely matching declaration; the same
/// declaration reached identically through multiple branches still resolves
/// once. Competing, ambiguous, cyclic, and unresolvable branches fail closed as
/// `Blocked`.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_interface_chain_member(
    interface_path: &str,
    member_name: &str,
    member_kind: &str,
    call_arity: Option<usize>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
    visited_interface_paths: &mut BTreeSet<String>,
) -> Result<KotlinInheritedMemberResolution> {
    if let Some(deadline) = deadline {
        deadline.check("resolving Kotlin interface chain member")?;
    }
    if !visited_interface_paths.insert(interface_path.to_string()) {
        return Ok(KotlinInheritedMemberResolution::Blocked);
    }
    let target_path = format!("{interface_path}::{member_name}");
    let declared_candidates = semantic_path_index
        .get(&target_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == member_kind
                && candidate.scope_path.as_deref() == Some(interface_path)
                && call_arity.is_none_or(|arity| candidate.parameters.len() == arity)
        })
        .collect::<Vec<_>>();
    if !declared_candidates.is_empty() {
        let resolution = match declared_candidates.as_slice() {
            [candidate_index] => KotlinInheritedMemberResolution::Resolved(*candidate_index),
            _ => KotlinInheritedMemberResolution::Blocked,
        };
        visited_interface_paths.remove(interface_path);
        return Ok(resolution);
    }
    let interface_candidates = semantic_path_index
        .get(interface_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "class_declaration" && kotlin_type_is_interface(candidate)
        })
        .collect::<Vec<_>>();
    let [interface_index] = interface_candidates.as_slice() else {
        visited_interface_paths.remove(interface_path);
        return Ok(KotlinInheritedMemberResolution::Blocked);
    };
    let source_interface = &raw_symbols[*interface_index];
    let Some(parent_spellings) =
        kotlin_direct_interface_parent_spellings(source_interface, file_overrides, deadline)?
    else {
        visited_interface_paths.remove(interface_path);
        return Ok(KotlinInheritedMemberResolution::Blocked);
    };
    let mut resolved_index = None;
    for parent_spelling in parent_spellings {
        let Some(parent_interface_path) = resolve_kotlin_receiver_type_path(
            source_interface,
            &parent_spelling,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            visited_interface_paths.remove(interface_path);
            return Ok(KotlinInheritedMemberResolution::Blocked);
        };
        match resolve_kotlin_interface_chain_member(
            &parent_interface_path,
            member_name,
            member_kind,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
            visited_interface_paths,
        )? {
            KotlinInheritedMemberResolution::Resolved(index) => {
                if resolved_index
                    .as_ref()
                    .is_some_and(|resolved| *resolved != index)
                {
                    visited_interface_paths.remove(interface_path);
                    return Ok(KotlinInheritedMemberResolution::Blocked);
                }
                resolved_index.get_or_insert(index);
            }
            KotlinInheritedMemberResolution::Blocked => {
                visited_interface_paths.remove(interface_path);
                return Ok(KotlinInheritedMemberResolution::Blocked);
            }
            KotlinInheritedMemberResolution::NoMember => {}
        }
    }
    visited_interface_paths.remove(interface_path);
    Ok(resolved_index
        .map(KotlinInheritedMemberResolution::Resolved)
        .unwrap_or(KotlinInheritedMemberResolution::NoMember))
}

/// Resolves a member declared directly on `receiver_type_path` or on a parent
/// interface in its extends chain, when the receiver type is a uniquely
/// declared interface. Non-interface receiver types resolve as `NoMember` so
/// callers can fall back to extension resolution; blocked chains (ambiguous or
/// unresolvable branches) resolve as `Blocked` and must fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_inherited_member_index(
    receiver_type_path: &str,
    member_name: &str,
    member_kind: &str,
    call_arity: Option<usize>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<KotlinInheritedMemberResolution> {
    let receiver_is_interface = semantic_path_index
        .get(receiver_type_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "class_declaration" && kotlin_type_is_interface(candidate)
        })
        .count()
        == 1;
    if !receiver_is_interface {
        return Ok(KotlinInheritedMemberResolution::NoMember);
    }
    let mut visited_interface_paths = BTreeSet::new();
    resolve_kotlin_interface_chain_member(
        receiver_type_path,
        member_name,
        member_kind,
        call_arity,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
        &mut visited_interface_paths,
    )
}

/// Extracts the delegation specifier spellings of a Kotlin class declaration:
/// the class supertype (a `constructor_invocation` specifier) and the direct
/// implemented interface spellings (every other specifier). Classes without a
/// supertype clause return `None` for the supertype, and classes without
/// implemented interfaces return an empty interface list; non-class
/// declarations and any specifier without a usable dotted spelling fail closed
/// as `None`.
fn kotlin_class_delegation_spellings(
    class_symbol: &IndexedSymbol,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(Option<String>, Vec<String>)>> {
    if class_symbol.node_kind != "class_declaration" || kotlin_type_is_interface(class_symbol) {
        return Ok(None);
    }
    let path = Path::new(&class_symbol.file_path);
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
            deadline.check("locating Kotlin class delegation specifiers")?;
        }
        if node.kind() == "class_declaration"
            && (node.start_byte(), node.end_byte()) == class_symbol.byte_range
        {
            let mut cursor = node.walk();
            let Some(specifiers) = node
                .named_children(&mut cursor)
                .find(|child| child.kind() == "delegation_specifiers")
            else {
                return Ok(Some((None, Vec::new())));
            };
            let mut specifier_cursor = specifiers.walk();
            let mut supertype = None;
            let mut interfaces = Vec::new();
            for specifier in specifiers.named_children(&mut specifier_cursor) {
                let specifier = match specifier.kind() {
                    "delegation_specifier" => {
                        let mut unwrap_cursor = specifier.walk();
                        specifier.named_children(&mut unwrap_cursor).next()
                    }
                    _ => Some(specifier),
                };
                let Some(specifier) = specifier else {
                    return Ok(None);
                };
                if specifier.kind() == "constructor_invocation" {
                    if supertype.is_some() {
                        return Ok(None);
                    }
                    supertype = kotlin_delegation_specifier_type_name(specifier, &source)?;
                } else {
                    let Some(spelling) = kotlin_delegation_specifier_type_name(specifier, &source)?
                    else {
                        return Ok(None);
                    };
                    interfaces.push(spelling);
                }
            }
            return Ok(Some((supertype, interfaces)));
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    Ok(None)
}

/// Resolves the direct superclass type path of a Kotlin class declaration.
/// The superclass is the class's `constructor_invocation` delegation
/// specifier; a class without one (including classes whose only delegation
/// specifiers are interfaces) or whose supertype spelling does not resolve
/// fails closed as `None` so a hierarchy walk can stop without guessing.
fn kotlin_superclass_path_for_class(
    class_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some((superclass_reference, _)) =
        kotlin_class_delegation_spellings(class_symbol, file_overrides, deadline)?
    else {
        return Ok(None);
    };
    let Some(superclass_reference) = superclass_reference else {
        return Ok(None);
    };
    resolve_kotlin_receiver_type_path(
        class_symbol,
        &superclass_reference,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Returns whether `member_name` of `member_kind` is declared anywhere in the
/// class hierarchy rooted at `initial_type_path` (the class itself and each
/// resolvable direct superclass). An ambiguous class, a cyclic hierarchy, or
/// an unresolvable superclass chain fails closed as `None`; a chain that
/// terminates without declaring the member returns `Some(false)`.
#[allow(clippy::too_many_arguments)]
fn kotlin_class_hierarchy_declares_member_from_type_path(
    initial_type_path: &str,
    member_name: &str,
    member_kind: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<bool>> {
    let mut visited_type_paths = BTreeSet::new();
    let mut current_type_path = initial_type_path.to_string();
    loop {
        if let Some(deadline) = deadline {
            deadline.check("checking Kotlin superclass members")?;
        }
        if !visited_type_paths.insert(current_type_path.clone()) {
            return Ok(None);
        }
        let target_path = format!("{current_type_path}::{member_name}");
        if semantic_path_index
            .get(&target_path)
            .into_iter()
            .flatten()
            .copied()
            .any(|index| {
                let candidate = &raw_symbols[index];
                candidate.node_kind == member_kind
                    && candidate.scope_path.as_deref() == Some(current_type_path.as_str())
            })
        {
            return Ok(Some(true));
        }
        let class_candidates = semantic_path_index
            .get(&current_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.node_kind == "class_declaration" && !kotlin_type_is_interface(candidate)
            })
            .collect::<Vec<_>>();
        let [class_index] = class_candidates.as_slice() else {
            return Ok(None);
        };
        let Some(superclass_path) = kotlin_superclass_path_for_class(
            &raw_symbols[*class_index],
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(Some(false));
        };
        current_type_path = superclass_path;
    }
}

/// Resolves a member of `member_kind` declared on a parent class in the direct
/// superclass chain of `initial_type_path` with an optional exact arity match.
/// The receiver class itself is excluded because direct members are resolved
/// before this fallback. A uniquely declared member returns its index; an
/// ambiguous class, a cyclic or unresolvable superclass chain, or a competing
/// overload set fails closed as `None`.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_superclass_chain_member(
    initial_type_path: &str,
    member_name: &str,
    member_kind: &str,
    call_arity: Option<usize>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<usize>> {
    let mut visited_type_paths = BTreeSet::new();
    let mut current_type_path = initial_type_path.to_string();
    loop {
        if let Some(deadline) = deadline {
            deadline.check("resolving Kotlin superclass chain member")?;
        }
        if !visited_type_paths.insert(current_type_path.clone()) {
            return Ok(None);
        }
        let class_candidates = semantic_path_index
            .get(&current_type_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.node_kind == "class_declaration" && !kotlin_type_is_interface(candidate)
            })
            .collect::<Vec<_>>();
        let [class_index] = class_candidates.as_slice() else {
            return Ok(None);
        };
        let Some(superclass_path) = kotlin_superclass_path_for_class(
            &raw_symbols[*class_index],
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let target_path = format!("{superclass_path}::{member_name}");
        let candidates = semantic_path_index
            .get(&target_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.node_kind == member_kind
                    && candidate.scope_path.as_deref() == Some(superclass_path.as_str())
                    && call_arity.is_none_or(|arity| candidate.parameters.len() == arity)
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [candidate_index] => return Ok(Some(*candidate_index)),
            [] => {}
            _ => return Ok(None),
        }
        current_type_path = superclass_path;
    }
}

/// Returns the direct implemented interface type spellings of a Kotlin class
/// declaration, mirroring `kotlin_class_delegation_spellings`; non-class
/// declarations and malformed specifiers fail closed as `None`.
fn kotlin_direct_interface_spellings_for_class(
    class_symbol: &IndexedSymbol,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<Vec<String>>> {
    let Some((_, interface_spellings)) =
        kotlin_class_delegation_spellings(class_symbol, file_overrides, deadline)?
    else {
        return Ok(None);
    };
    Ok(Some(interface_spellings))
}

/// Dispatches a member of `member_kind` declared on an implemented interface
/// for a class-typed instance receiver whose class and direct superclass chain
/// do not declare it. The receiver's direct interfaces resolve in its own file
/// and enclosing scope; exactly one direct-interface chain must provide a
/// uniquely arity-matched member and every other chain must prove it has no
/// declaration. Any same-name member declared in the receiver class hierarchy,
/// competing or unresolved interface chains, and ambiguous chains fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_class_receiver_interface_member(
    receiver_class_path: &str,
    member_name: &str,
    member_kind: &str,
    call_arity: Option<usize>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<usize>> {
    let class_candidates = semantic_path_index
        .get(receiver_class_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "class_declaration" && !kotlin_type_is_interface(candidate)
        })
        .collect::<Vec<_>>();
    let [class_index] = class_candidates.as_slice() else {
        return Ok(None);
    };
    let receiver_class = &raw_symbols[*class_index];
    if kotlin_class_hierarchy_declares_member_from_type_path(
        receiver_class_path,
        member_name,
        member_kind,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? != Some(false)
    {
        return Ok(None);
    }
    let Some(interface_spellings) =
        kotlin_direct_interface_spellings_for_class(receiver_class, file_overrides, deadline)?
    else {
        return Ok(None);
    };
    let mut resolved_index = None;
    for interface_spelling in interface_spellings {
        let Some(interface_path) = resolve_kotlin_receiver_type_path(
            receiver_class,
            &interface_spelling,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let interface_is_unique = semantic_path_index
            .get(&interface_path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| {
                let candidate = &raw_symbols[*index];
                candidate.node_kind == "class_declaration" && kotlin_type_is_interface(candidate)
            })
            .count()
            == 1;
        if !interface_is_unique {
            return Ok(None);
        }
        match resolve_kotlin_inherited_member_index(
            &interface_path,
            member_name,
            member_kind,
            call_arity,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )? {
            KotlinInheritedMemberResolution::Resolved(index) => {
                if resolved_index
                    .as_ref()
                    .is_some_and(|resolved| *resolved != index)
                {
                    return Ok(None);
                }
                resolved_index.get_or_insert(index);
            }
            KotlinInheritedMemberResolution::Blocked => return Ok(None),
            KotlinInheritedMemberResolution::NoMember => {}
        }
    }
    Ok(resolved_index)
}

/// Dispatches `method` on `type_path`: a unique member function shadows extensions,
/// an ambiguous member overload set fails closed instead of guessing an extension
/// target, and otherwise an unambiguous top-level extension resolves the call.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_member_or_extension(
    source_symbol: &IndexedSymbol,
    type_path: &str,
    type_name: &str,
    method: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let target_path = format!("{type_path}::{method}");
    let member_candidates = semantic_path_index
        .get(&target_path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "function_declaration"
                && candidate.scope_path.as_deref() == Some(type_path)
                && candidate.parameters.len() == call_arity
        })
        .collect::<Vec<_>>();
    if member_candidates.len() == 1 {
        return Ok(Some(raw_symbols[member_candidates[0]].symbol_id.clone()));
    }
    if !member_candidates.is_empty() {
        return Ok(None);
    }
    // An interface-typed receiver dispatches a member declared on a parent
    // interface in its extends chain before falling back to extensions;
    // inherited interface members shadow extensions like direct members, and
    // blocked chains fail closed instead of guessing an extension target.
    match resolve_kotlin_inherited_member_index(
        type_path,
        method,
        "function_declaration",
        Some(call_arity),
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        KotlinInheritedMemberResolution::Resolved(index) => {
            return Ok(Some(raw_symbols[index].symbol_id.clone()));
        }
        KotlinInheritedMemberResolution::Blocked => return Ok(None),
        KotlinInheritedMemberResolution::NoMember => {}
    }
    // A class-typed receiver dispatches a member declared on a parent class
    // in its direct superclass chain when neither the class nor any nearer
    // superclass declares it; inherited class members shadow extensions like
    // direct members, and ambiguous or unresolvable chains fail closed.
    if let Some(index) = resolve_kotlin_superclass_chain_member(
        type_path,
        method,
        "function_declaration",
        Some(call_arity),
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        return Ok(Some(raw_symbols[index].symbol_id.clone()));
    }
    // A class-typed receiver dispatches a member declared on an implemented
    // interface when its class hierarchy does not declare the method; exactly
    // one direct-interface chain must provide a uniquely arity-matched member,
    // and inherited interface members shadow extensions like direct members.
    if let Some(index) = resolve_kotlin_class_receiver_interface_member(
        type_path,
        method,
        "function_declaration",
        Some(call_arity),
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        return Ok(Some(raw_symbols[index].symbol_id.clone()));
    }
    resolve_kotlin_extension_call(
        source_symbol,
        type_name,
        method,
        call_arity,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_extension_call(
    source_symbol: &IndexedSymbol,
    receiver_type: &str,
    method: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let package_scope = kotlin_package_scope(source_symbol, raw_symbols);
    let imported_binding = resolve_kotlin_import_binding_for_reference(
        &source_symbol.file_path,
        method,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    let candidates = raw_symbols
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            candidate.node_kind == "function_declaration"
                && candidate.extension_receiver.as_deref() == Some(receiver_type)
                && candidate.base_name == method
                && candidate.parameters.len() == call_arity
                && kotlin_symbol_is_top_level(candidate, raw_symbols)
                && (candidate.file_path == source_symbol.file_path
                    || package_scope
                        .is_some_and(|scope| candidate.scope_path.as_deref() == Some(scope))
                    || imported_binding
                        .as_ref()
                        .is_some_and(|binding| binding.semantic_path == candidate.semantic_path))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    Ok((candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone()))
}

fn kotlin_symbol_is_top_level(candidate: &IndexedSymbol, raw_symbols: &[IndexedSymbol]) -> bool {
    match (
        candidate.scope_path.as_deref(),
        kotlin_package_scope(candidate, raw_symbols),
    ) {
        (None, None) => true,
        (Some(scope), Some(package)) => scope == package,
        _ => false,
    }
}

fn resolve_kotlin_receiver_type_path(
    source_symbol: &IndexedSymbol,
    type_name: &str,
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    // A receiver type may be a type alias such as `typealias Helper = Other`,
    // so resolution walks the alias chain until it reaches a concrete class or
    // interface declaration. Each hop applies the same scope/import rules, and
    // a visited set fails closed on cyclic aliases instead of looping forever.
    // A dotted name such as `Outer.Inner` resolves its first segment with the
    // same scope/import rules and then walks nested type declarations; a dotted
    // alias target such as `typealias Helper = Outer.Inner` expands into those
    // nested segments before the remaining path continues.
    let segments = type_name.split('.').collect::<Vec<_>>();
    if segments.is_empty()
        || segments.iter().any(|segment| {
            segment.is_empty()
                || !segment
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
    {
        return Ok(None);
    }
    let mut visited = BTreeSet::new();
    let mut pending = segments
        .iter()
        .map(|segment| segment.to_string())
        .collect::<VecDeque<_>>();
    let mut resolved_path = None;
    // The first segment resolves in the caller's file and package scope, but an
    // alias target must resolve in the alias's own file and package scope so an
    // imported alias whose target lives in its own package still pins the
    // receiver.
    let mut scope_symbol = source_symbol;
    while let Some(name) = pending.pop_front() {
        let candidate_path = if let Some(current_path) = resolved_path.as_deref() {
            // A later segment must name a concrete nested type declaration under
            // the resolved path; nested aliases and missing members fail closed.
            let nested_path = format!("{current_path}::{name}");
            if !kotlin_path_is_nested_type_declaration(&nested_path, raw_symbols) {
                return Ok(None);
            }
            Some(nested_path)
        } else {
            let same_package_path = kotlin_package_scope(scope_symbol, raw_symbols)
                .map(|scope| format!("{scope}::{name}"));
            let same_package_is_type = same_package_path
                .as_deref()
                .is_some_and(|path| kotlin_path_is_type_declaration(path, raw_symbols));
            let imported_binding = resolve_kotlin_import_binding_for_reference(
                &scope_symbol.file_path,
                &name,
                file_overrides,
                kotlin_import_contexts_by_file,
                deadline,
            )?;
            let imported_path = imported_binding
                .map(|binding| binding.semantic_path)
                .filter(|path| kotlin_path_is_type_declaration(path, raw_symbols));
            match (same_package_is_type, imported_path) {
                // A same-package declaration and an explicit import of the same name conflict.
                (true, Some(_)) => return Ok(None),
                (true, None) => same_package_path,
                (false, Some(path)) => Some(path),
                (false, None) => return Ok(None),
            }
        };
        let Some(candidate_path) = candidate_path else {
            return Ok(None);
        };
        if !visited.insert(candidate_path.clone()) {
            return Ok(None);
        }
        if resolved_path.is_none()
            && let Some(alias_target) = kotlin_type_alias_target(&candidate_path, raw_symbols)
        {
            if let Some(alias_symbol) = raw_symbols.iter().find(|candidate| {
                candidate.semantic_path == candidate_path && candidate.node_kind == "type_alias"
            }) {
                scope_symbol = alias_symbol;
            }
            let target_segments = alias_target.split('.').collect::<Vec<_>>();
            if target_segments.is_empty()
                || target_segments.iter().any(|segment| {
                    segment.is_empty()
                        || !segment
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || character == '_')
                })
            {
                return Ok(None);
            }
            for segment in target_segments.iter().rev() {
                pending.push_front(segment.to_string());
            }
            continue;
        }
        resolved_path = Some(candidate_path);
    }
    Ok(resolved_path)
}

/// Returns the target type name of a uniquely declared type alias such as
/// `typealias Helper = Other`. Generic, ambiguous, and missing alias targets
/// fail closed because they cannot pin a receiver; a dotted target such as
/// `typealias Helper = Outer.Inner` resolves through the same dotted type-path
/// rules as a directly declared nested type.
fn kotlin_type_alias_target(path: &str, raw_symbols: &[IndexedSymbol]) -> Option<String> {
    let aliases = raw_symbols
        .iter()
        .filter(|candidate| candidate.semantic_path == path && candidate.node_kind == "type_alias")
        .collect::<Vec<_>>();
    if aliases.len() != 1 {
        return None;
    }
    kotlin_dotted_type_name(aliases[0].return_type.as_deref()?)
}

fn kotlin_path_is_type_declaration(path: &str, raw_symbols: &[IndexedSymbol]) -> bool {
    raw_symbols
        .iter()
        .any(|candidate| candidate.semantic_path == path && is_kotlin_type_declaration(candidate))
}

/// A nested segment of a dotted receiver path must be a concrete class,
/// interface, or object declaration; nested type aliases are not walked so they
/// fail closed instead of returning a non-terminal path.
fn kotlin_path_is_nested_type_declaration(path: &str, raw_symbols: &[IndexedSymbol]) -> bool {
    raw_symbols.iter().any(|candidate| {
        candidate.semantic_path == path
            && matches!(
                candidate.node_kind.as_str(),
                "class_declaration" | "interface_declaration" | "object_declaration"
            )
    })
}

/// Resolves an unbound receiver name to a named object declaration such as
/// `Config` in `Config.helper(...)`. The object must be uniquely declared in the
/// caller's package or bound by an explicit import; a same-package object that
/// conflicts with an imported binding, an unknown name, or a name that matches
/// multiple declarations fails closed.
fn resolve_kotlin_object_receiver_path(
    source_symbol: &IndexedSymbol,
    receiver: &str,
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if receiver.is_empty()
        || !receiver
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Ok(None);
    }
    let same_package_path = kotlin_package_scope(source_symbol, raw_symbols)
        .map(|scope| format!("{scope}::{receiver}"));
    let same_package_object_count = same_package_path
        .as_deref()
        .map(|path| kotlin_path_object_count(path, raw_symbols))
        .unwrap_or(0);
    let imported_binding = resolve_kotlin_import_binding_for_reference(
        &source_symbol.file_path,
        receiver,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    let imported_path = imported_binding
        .map(|binding| binding.semantic_path)
        .filter(|path| kotlin_path_object_count(path, raw_symbols) == 1);
    match (same_package_object_count, imported_path) {
        // A same-package object and an explicit import of the same name conflict.
        (1, Some(_)) => Ok(None),
        (1, None) => Ok(same_package_path),
        (0, Some(path)) => Ok(Some(path)),
        _ => Ok(None),
    }
}

fn kotlin_path_object_count(path: &str, raw_symbols: &[IndexedSymbol]) -> usize {
    raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.semantic_path == path && candidate.node_kind == "object_declaration"
        })
        .count()
}

/// Returns the count of top-level property declarations at exactly
/// `semantic_path`, such as `com::example::holder` for a package-level
/// `val holder: Holder = Holder()`. Class members and companion members live
/// under longer paths so they never match, and a same-named class or object
/// declaration does not count.
fn kotlin_path_top_level_property_count(path: &str, raw_symbols: &[IndexedSymbol]) -> usize {
    raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.semantic_path == path && candidate.node_kind == "property_declaration"
        })
        .count()
}

/// Returns the uniquely resolved top-level property symbol for an unbound
/// receiver name such as `holder` in `val first = holder.item` with
/// `val holder: Holder = Holder()` at package scope. The property must be
/// uniquely declared in the caller's package or bound by an explicit import;
/// a same-package property that conflicts with an imported binding, an
/// unknown name, and a name that matches multiple declarations fail closed.
fn resolve_kotlin_top_level_property_symbol<'a>(
    source_symbol: &IndexedSymbol,
    receiver: &str,
    raw_symbols: &'a [IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<&'a IndexedSymbol>> {
    if receiver.is_empty()
        || !receiver
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Ok(None);
    }
    let same_package_path = kotlin_package_scope(source_symbol, raw_symbols)
        .map(|scope| format!("{scope}::{receiver}"));
    let same_package_property_count = same_package_path
        .as_deref()
        .map(|path| kotlin_path_top_level_property_count(path, raw_symbols))
        .unwrap_or(0);
    let imported_binding = resolve_kotlin_import_binding_for_reference(
        &source_symbol.file_path,
        receiver,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    let imported_property_path = imported_binding
        .map(|binding| binding.semantic_path)
        .filter(|path| kotlin_path_top_level_property_count(path, raw_symbols) == 1);
    let candidate_path = match (same_package_property_count, imported_property_path) {
        // A same-package property and an explicit import of the same name
        // conflict.
        (1, Some(_)) => return Ok(None),
        (1, None) => same_package_path,
        (0, Some(path)) => Some(path),
        _ => return Ok(None),
    };
    let Some(property_path) = candidate_path else {
        return Ok(None);
    };
    Ok(raw_symbols
        .iter()
        .find(|candidate| candidate.semantic_path == property_path))
}

/// Resolves an unbound receiver name to the declared type path of a
/// top-level property such as `holder` in `val first = holder.item` with
/// `val holder: Holder = Holder()` at package scope. The property's declared
/// type resolves in the property's own file and package scope (its imports
/// and package), not the caller's, so an imported property whose type lives
/// in its own package still pins the receiver; properties without a usable
/// declared type fail closed.
fn resolve_kotlin_top_level_property_receiver_path(
    source_symbol: &IndexedSymbol,
    receiver: &str,
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(property) = resolve_kotlin_top_level_property_symbol(
        source_symbol,
        receiver,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(return_type) = property
        .return_type
        .as_deref()
        .and_then(kotlin_dotted_type_name)
    else {
        return Ok(None);
    };
    resolve_kotlin_receiver_type_path(
        property,
        &return_type,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Resolves an unbound receiver name to the element component type path of a
/// top-level property whose declared type is a single-level generic array,
/// such as `itemGroup` in `val first = itemGroup[0]` with
/// `val itemGroup: Array<Holder>` at package scope. The component type
/// resolves in the property's own file and package scope; non-array,
/// primitive, and unresolvable top-level properties fail closed.
fn resolve_kotlin_top_level_property_array_component_path(
    source_symbol: &IndexedSymbol,
    receiver: &str,
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(property) = resolve_kotlin_top_level_property_symbol(
        source_symbol,
        receiver,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let Some(return_type) = property.return_type.as_deref() else {
        return Ok(None);
    };
    let Some(component_name) = kotlin_array_type_component_name(return_type) else {
        return Ok(None);
    };
    resolve_kotlin_receiver_type_path(
        property,
        &component_name,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Returns whether `owner_type_path` declares a property named
/// `property_name` directly or inherits one, without resolving the property's
/// declared type. This lets a bare first hop dispatch on the enclosing type's
/// own or inherited property (which shadows a same-named top-level property)
/// even when the property's type is not resolvable, so callers fail closed
/// instead of falling through to a top-level property.
fn kotlin_type_declares_property(
    owner_type_path: &str,
    property_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<bool> {
    let owns = semantic_path_index
        .get(&format!("{owner_type_path}::{property_name}"))
        .into_iter()
        .flatten()
        .any(|index| {
            let candidate = &raw_symbols[*index];
            candidate.node_kind == "property_declaration"
                && candidate.scope_path.as_deref() == Some(owner_type_path)
        });
    if owns {
        return Ok(true);
    }
    Ok(resolve_kotlin_inherited_property_index(
        owner_type_path,
        property_name,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    .is_some())
}

/// Returns the nested object path when `object_name` under `owner_type_path`
/// names exactly one object declaration and no same-named class or interface
/// declaration conflicts; unknown and ambiguous nested objects fail closed.
fn kotlin_path_nested_object(
    owner_type_path: &str,
    object_name: &str,
    raw_symbols: &[IndexedSymbol],
) -> Option<String> {
    let nested_path = format!("{owner_type_path}::{object_name}");
    let mut object_count = 0usize;
    let mut other_type_count = 0usize;
    for candidate in raw_symbols {
        if candidate.semantic_path != nested_path {
            continue;
        }
        match candidate.node_kind.as_str() {
            "object_declaration" => object_count += 1,
            "class_declaration" | "interface_declaration" => other_type_count += 1,
            _ => {}
        }
    }
    (object_count == 1 && other_type_count == 0).then_some(nested_path)
}

/// Returns the nested type path when `type_name` under `owner_type_path` names
/// exactly one class or interface declaration and no same-named object or type
/// alias declaration conflicts; unknown and ambiguous nested types fail closed.
/// Only classes and interfaces can host companion objects.
fn kotlin_path_nested_class_path(
    owner_type_path: &str,
    type_name: &str,
    raw_symbols: &[IndexedSymbol],
) -> Option<String> {
    let nested_path = format!("{owner_type_path}::{type_name}");
    let mut class_count = 0usize;
    let mut other_declaration_count = 0usize;
    for candidate in raw_symbols {
        if candidate.semantic_path != nested_path {
            continue;
        }
        match candidate.node_kind.as_str() {
            "class_declaration" | "interface_declaration" => class_count += 1,
            _ => other_declaration_count += 1,
        }
    }
    (class_count == 1 && other_declaration_count == 0).then_some(nested_path)
}

/// Returns the nested type path when `type_name` under `owner_type_path` names
/// exactly one nested class, interface, or object declaration and no
/// same-named non-type declaration conflicts; unknown and ambiguous nested
/// types fail closed. Receiver-chain hops use this to walk nested types such
/// as `Nested` in `Outer.Nested.make()` or `DeepOuter.Mid.Inner.make()` the
/// same way dotted type paths walk nested declarations.
fn kotlin_nested_type_hop_path(
    owner_type_path: &str,
    type_name: &str,
    raw_symbols: &[IndexedSymbol],
) -> Option<String> {
    let nested_path = format!("{owner_type_path}::{type_name}");
    let mut type_count = 0usize;
    for candidate in raw_symbols {
        if candidate.semantic_path != nested_path {
            continue;
        }
        match candidate.node_kind.as_str() {
            "class_declaration" | "interface_declaration" | "object_declaration" => type_count += 1,
            _ => return None,
        }
    }
    (type_count == 1).then_some(nested_path)
}

/// Returns the constructed type path when a `()`-marked chain hop names a
/// uniquely constructible nested class directly under `owner_type_path`, such
/// as `Nested()` in `Outer.Nested().make()` after the `Outer` root; unknown,
/// ambiguous, and non-constructible nested classes fail closed.
fn kotlin_constructor_hop_path(
    owner_type_path: &str,
    type_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Option<String> {
    let nested_path = format!("{owner_type_path}::{type_name}");
    (kotlin_constructible_class_indexes(&nested_path, raw_symbols, semantic_path_index).len() == 1)
        .then_some(nested_path)
}

/// Resolves a nested constructor-call chain root such as `Outer.Nested()` in
/// `val group = Outer.Nested().make()` or `Outer.Nested.Inner()` in
/// `val group = Outer.Nested.Inner().make()` to the constructed type path and
/// the number of leading hops consumed. The first hop must resolve as a type
/// path and must not be shadowed by a local binding; each following non-`()`
/// hop walks a uniquely declared nested type, and the first `()`-marked hop
/// must construct a uniquely declared nested class. The constructed path
/// consumes all hops up to and including the constructor hop so callers
/// dispatch the remaining chain on it. Unknown or ambiguous roots, shadowed
/// first hops, unresolvable intermediate nested types, and unknown,
/// ambiguous, or non-constructible nested classes return `None` so callers
/// fall through to companion, object, and other receiver roots instead of
/// guessing.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_nested_constructor_chain_root(
    source_symbol: &IndexedSymbol,
    hops: &[&str],
    bindings: Option<&KotlinReceiverTypeBindings>,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, usize)>> {
    if hops.len() < 2
        || kotlin_method_call_hop_spelling(hops[0]).is_some()
        || bindings
            .as_ref()
            .is_some_and(|bindings| bindings.contains(hops[0]))
    {
        return Ok(None);
    }
    let Some(mut owner_path) = resolve_kotlin_receiver_type_path(
        source_symbol,
        hops[0],
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let mut consumed = 1;
    while consumed < hops.len() {
        let hop = hops[consumed];
        if let Some(constructor_name) = kotlin_method_call_hop_spelling(hop) {
            return Ok(kotlin_constructor_hop_path(
                &owner_path,
                &constructor_name,
                raw_symbols,
                semantic_path_index,
            )
            .map(|nested_path| (nested_path, consumed + 1)));
        }
        let Some(nested_path) = kotlin_nested_type_hop_path(&owner_path, hop, raw_symbols) else {
            return Ok(None);
        };
        owner_path = nested_path;
        consumed += 1;
    }
    Ok(None)
}

/// Resolves a `()`-marked receiver chain such as
/// `Outer.Inner().helper(...)`, `Group().member.helper(...)`,
/// `makeGroup().entry.helper(...)`, or `group.inner().entry.helper(...)`. The
/// `()` marker names either a type path that must resolve to exactly one
/// constructible class declaration, a bare factory call whose declared return
/// type pins the receiver, or a receiver chain ending in a method-call hop
/// whose declared return type continues the chain; the remaining hops resolve
/// each intermediate property's declared type or method-call hop and then
/// dispatch the final member or extension exactly like an instance receiver.
/// Unknown names, non-constructible declarations (interfaces, enums, sealed,
/// abstract, annotation, and inner classes), unresolvable factory callees, and
/// unresolvable receiver chains and hops fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_constructor_receiver_call(
    source_symbol: &IndexedSymbol,
    reference_name: &str,
    call_arity: usize,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let Some(marker) = reference_name.find("()") else {
        return Ok(None);
    };
    if marker == 0 {
        return Ok(None);
    }
    let type_path_text = &reference_name[..marker];
    let chain_text = reference_name[marker + 2..]
        .strip_prefix('.')
        .unwrap_or_default();
    if type_path_text.is_empty() || chain_text.is_empty() {
        return Ok(None);
    }
    let hops = chain_text.split('.').collect::<Vec<_>>();
    if hops.iter().any(|hop| hop.is_empty()) {
        return Ok(None);
    }
    // The constructed type path must resolve to exactly one constructible
    // class; objects, aliases, unknown names, and non-constructible classes
    // fail closed. When the `()`-marked root is not a type name at all, it
    // may be a bare factory-call root such as `makeGroup()` in
    // `makeGroup().entry.helper(...)`; the leading call then resolves through
    // the same factory rules as a `var` initializer (a unique same-file,
    // same-package, or explicitly imported top-level function with a
    // declared return type) and the trailing member chain dispatches on the
    // factory's declared return type. Unknown factories and factories
    // without a declared return type fail closed.
    let type_path = if let Some(type_path) = resolve_kotlin_receiver_type_path(
        source_symbol,
        type_path_text,
        raw_symbols,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        let candidates =
            kotlin_constructible_class_indexes(&type_path, raw_symbols, semantic_path_index);
        if candidates.len() != 1 {
            return Ok(None);
        }
        type_path
    } else if let Some(factory_type_path) = resolve_kotlin_initializer_type_path(
        source_symbol,
        type_path_text,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )? {
        factory_type_path
    } else {
        // A `()`-marked prefix with a dotted receiver such as `group.inner`
        // may be a receiver chain ending in a method-call hop. The receiver
        // chain resolves through the same binding/object rules, the method-call
        // hop dispatches on its declared return type, and the trailing chain
        // continues from that type; a bare `inner().helper(...)` with no
        // receiver, unknown receiver chains, and unresolvable hops fail closed.
        if !type_path_text.contains('.') {
            return Ok(None);
        }
        let Some((receiver_chain, method_hop)) = type_path_text.rsplit_once('.') else {
            return Ok(None);
        };
        // The `()` marker was stripped with the type-path split, so rebuild the
        // hop spelling before parsing the method name.
        let hop_spelling = format!("{method_hop}()");
        let Some(method_name) = kotlin_method_call_hop_spelling(&hop_spelling) else {
            return Ok(None);
        };
        let Some(receiver_path) = resolve_kotlin_receiver_chain_type_path(
            source_symbol,
            receiver_chain,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let Some(hop_path) = kotlin_method_call_hop_type_path(
            &receiver_path,
            &method_name,
            source_symbol,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        hop_path
    };
    // Each intermediate hop must resolve to a uniquely declared property whose
    // explicit type or bare constructor initializer pins the next receiver, or
    // to a method-call hop whose declared return type continues the chain;
    // missing, ambiguous, and unresolvable hops fail closed.
    let mut receiver_path = type_path;
    for hop in hops.iter().take(hops.len() - 1) {
        let Some(next_path) = kotlin_chain_hop_type_path(
            &receiver_path,
            hop,
            source_symbol,
            raw_symbols,
            semantic_path_index,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        receiver_path = next_path;
    }
    let method = hops[hops.len() - 1];
    let type_name = receiver_path
        .rsplit("::")
        .next()
        .unwrap_or(method)
        .to_string();
    resolve_kotlin_member_or_extension(
        source_symbol,
        &receiver_path,
        &type_name,
        method,
        call_arity,
        raw_symbols,
        semantic_path_index,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )
}

/// Resolves a bare call to a class name such as `Other(...)` to the class
/// declaration that the call constructs. A nested class inside the caller's
/// enclosing type shadows package-level declarations; a same-package class that
/// conflicts with an explicit import of the same name, an unknown name, and
/// non-constructible declarations (interfaces, enums, sealed/abstract/annotation/
/// inner classes) fail closed.
#[allow(clippy::too_many_arguments)]
fn resolve_kotlin_constructor_call(
    source_symbol: &IndexedSymbol,
    type_name: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
    file_overrides: Option<&BTreeMap<String, String>>,
    kotlin_import_contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if type_name.is_empty() || type_name.contains("::") {
        return Ok(None);
    }
    // A qualified constructor call such as `Outer.Inner(...)` resolves the
    // dotted type path first; the terminal path must name exactly one
    // constructible class declaration.
    if type_name.contains('.') {
        let Some(type_path) = resolve_kotlin_receiver_type_path(
            source_symbol,
            type_name,
            raw_symbols,
            file_overrides,
            kotlin_import_contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        let candidates =
            kotlin_constructible_class_indexes(&type_path, raw_symbols, semantic_path_index);
        return Ok((candidates.len() == 1).then(|| raw_symbols[candidates[0]].symbol_id.clone()));
    }
    // A nested class inside the caller's enclosing type shadows package-level and
    // imported declarations of the same name. The scope is only a type scope when
    // it differs from the caller's package scope; a top-level caller's scope is
    // its package, so the conflict checks below still apply.
    let package_scope = kotlin_package_scope(source_symbol, raw_symbols);
    if let Some(scope_path) = source_symbol.scope_path.as_deref()
        && package_scope != Some(scope_path)
    {
        let nested_candidates = kotlin_constructible_class_indexes(
            &format!("{scope_path}::{type_name}"),
            raw_symbols,
            semantic_path_index,
        );
        if nested_candidates.len() == 1 {
            return Ok(Some(raw_symbols[nested_candidates[0]].symbol_id.clone()));
        }
        if nested_candidates.len() > 1 {
            return Ok(None);
        }
    }
    let package_path = package_scope.map(|scope| format!("{scope}::{type_name}"));
    let package_candidates = package_path
        .as_deref()
        .map(|path| kotlin_constructible_class_indexes(path, raw_symbols, semantic_path_index))
        .unwrap_or_default();
    let imported_binding = resolve_kotlin_import_binding_for_reference(
        &source_symbol.file_path,
        type_name,
        file_overrides,
        kotlin_import_contexts_by_file,
        deadline,
    )?;
    let imported_candidates = imported_binding
        .map(|binding| binding.semantic_path)
        .map(|path| kotlin_constructible_class_indexes(&path, raw_symbols, semantic_path_index))
        .unwrap_or_default();
    // A same-package class and an explicit import of the same name conflict.
    if !package_candidates.is_empty() && !imported_candidates.is_empty() {
        return Ok(None);
    }
    if package_candidates.len() == 1 {
        return Ok(Some(raw_symbols[package_candidates[0]].symbol_id.clone()));
    }
    if imported_candidates.len() == 1 {
        return Ok(Some(raw_symbols[imported_candidates[0]].symbol_id.clone()));
    }
    Ok(None)
}

fn kotlin_constructible_class_indexes(
    path: &str,
    raw_symbols: &[IndexedSymbol],
    semantic_path_index: &BTreeMap<String, Vec<usize>>,
) -> Vec<usize> {
    semantic_path_index
        .get(path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| kotlin_class_is_constructible(&raw_symbols[*index]))
        .collect()
}

/// A Kotlin class is constructible through `Name(...)` only when it is a plain
/// class declaration whose keyword is `class` (not `interface`) and whose
/// modifiers do not forbid direct construction. Interfaces, enums, sealed,
/// abstract, annotation, and inner classes fail closed.
fn kotlin_class_is_constructible(symbol: &IndexedSymbol) -> bool {
    if symbol.node_kind != "class_declaration" {
        return false;
    }
    let Some(signature) = symbol.signature.as_deref() else {
        return false;
    };
    let tokens = signature.split_whitespace().collect::<Vec<_>>();
    let Some(keyword_position) = tokens
        .iter()
        .position(|token| *token == "class" || *token == "interface")
    else {
        return false;
    };
    if tokens[keyword_position] == "interface" {
        return false;
    }
    !tokens[..keyword_position].iter().any(|token| {
        matches!(
            *token,
            "enum" | "sealed" | "abstract" | "annotation" | "inner"
        )
    })
}

fn kotlin_package_scope<'a>(
    source_symbol: &'a IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
) -> Option<&'a str> {
    let mut scope = source_symbol.scope_path.as_deref()?;
    while let Some((parent, _)) = scope.rsplit_once("::") {
        if raw_symbols.iter().any(|candidate| {
            candidate.semantic_path == scope && is_kotlin_type_declaration(candidate)
        }) || raw_symbols.iter().any(|candidate| {
            // Companion scopes such as `Type::Companion` are not indexed as
            // type declarations themselves, but their parent type is.
            candidate.semantic_path == parent && is_kotlin_type_declaration(candidate)
        }) {
            scope = parent;
        } else {
            break;
        }
    }
    Some(scope)
}

fn is_kotlin_type_declaration(symbol: &IndexedSymbol) -> bool {
    matches!(
        symbol.node_kind.as_str(),
        "class_declaration" | "interface_declaration" | "object_declaration" | "type_alias"
    )
}

fn javascript_default_import_candidate_indexes(
    file_overrides: Option<&BTreeMap<String, String>>,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    binding: &JavaScriptImportBinding,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<usize>> {
    if binding.unresolved || binding.module_paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut candidates = BTreeSet::new();
    for module_path in &binding.module_paths {
        if let Some(deadline) = deadline {
            deadline.check("resolving JavaScript/TypeScript default import")?;
        }
        let Some(default_name) =
            resolve_javascript_module_default_export_name(module_path, file_overrides, deadline)?
        else {
            continue;
        };
        collect_javascript_member_candidates_in_module(
            raw_symbols,
            name_index,
            &default_name,
            module_path,
            &mut candidates,
        );
    }
    Ok(candidates.into_iter().collect())
}

fn javascript_namespace_object_call_candidate_indexes(
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    module_paths: &BTreeSet<String>,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<usize>> {
    if module_paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut candidates = BTreeSet::new();
    for module_path in module_paths {
        if let Some(deadline) = deadline {
            deadline.check("resolving JavaScript/TypeScript namespace-object call")?;
        }
        // The namespace object is callable only when the bound module exports
        // a single CommonJS callable value; other modules fail closed.
        let Some(binding) = resolve_javascript_namespace_object_call_binding(
            module_path,
            file_overrides,
            deadline,
        )?
        else {
            continue;
        };
        for exporting_path in &binding.module_paths {
            collect_javascript_member_candidates_in_module(
                raw_symbols,
                name_index,
                &binding.imported_name,
                exporting_path,
                &mut candidates,
            );
        }
    }
    Ok(candidates.into_iter().collect())
}

fn javascript_module_member_candidate_indexes(
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    member_name: &str,
    module_paths: &BTreeSet<String>,
    file_overrides: Option<&BTreeMap<String, String>>,
    javascript_import_contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<usize>> {
    if module_paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut candidates = BTreeSet::new();
    for module_path in module_paths {
        if let Some(deadline) = deadline {
            deadline.check("resolving JavaScript/TypeScript namespace member")?;
        }
        // Namespace members are the bound module's exports: direct named
        // exports, named re-export chains, and star re-export chains. Broken,
        // ambiguous, cyclic, or non-exported members fail closed instead of
        // falling back to same-named workspace symbols.
        let Some(binding) = resolve_javascript_namespace_member_binding(
            module_path,
            member_name,
            file_overrides,
            javascript_import_contexts_by_file,
            deadline,
        )?
        else {
            continue;
        };
        for exporting_path in &binding.module_paths {
            collect_javascript_member_candidates_in_module(
                raw_symbols,
                name_index,
                &binding.imported_name,
                exporting_path,
                &mut candidates,
            );
        }
    }
    Ok(candidates.into_iter().collect())
}

fn collect_javascript_member_candidates_in_module(
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
    member_name: &str,
    module_path: &str,
    candidates: &mut BTreeSet<usize>,
) {
    if let Some(indexes) = name_index.get(member_name) {
        for index in indexes.iter().copied() {
            if raw_symbols[index].file_path == *module_path {
                candidates.insert(index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::{Duration, Instant};

    use super::resolve_dependencies_for_symbol_with_deadline;
    use crate::symbol_dependency::c::CIncludeTargetsCache;
    use crate::symbol_dependency::rust::RustOutOfLineModuleContext;
    use crate::symbol_index_model::IndexedSymbol;
    use crate::workspace_scan::WorkspaceScanDeadline;

    #[test]
    fn deadline_resolver_checks_each_symbol_reference() {
        let symbol = IndexedSymbol {
            extension_receiver: None,
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
            &mut CIncludeTargetsCache::new(),
            &mut std::collections::BTreeMap::new(),
            &mut std::collections::BTreeMap::new(),
            &RustOutOfLineModuleContext::default(),
            &mut std::collections::BTreeMap::new(),
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

    #[test]
    fn java_array_factory_call_root_spelling_parses_single_element_access() {
        use super::java_array_factory_call_root_spelling;
        assert_eq!(
            java_array_factory_call_root_spelling("makeItems()[0]"),
            Some(("makeItems".to_string(), 0))
        );
        assert_eq!(
            java_array_factory_call_root_spelling("makeItems(1)[2]"),
            Some(("makeItems".to_string(), 1))
        );
        assert_eq!(
            java_array_factory_call_root_spelling("makeItems()[]"),
            Some(("makeItems".to_string(), 0))
        );
        // Multi-dimensional element access, dotted roots, and malformed
        // spellings fail closed.
        assert_eq!(
            java_array_factory_call_root_spelling("makeItems()[0][0]"),
            None
        );
        assert_eq!(
            java_array_factory_call_root_spelling("Util.makeItems()[0]"),
            None
        );
        assert_eq!(java_array_factory_call_root_spelling("makeItems()"), None);
        assert_eq!(java_array_factory_call_root_spelling("makeItems[0]"), None);
        assert_eq!(java_array_factory_call_root_spelling("[0]"), None);
        assert_eq!(java_array_factory_call_root_spelling("()"), None);
        assert_eq!(
            java_array_factory_call_root_spelling("makeItems()[0]x"),
            None
        );
    }

    #[test]
    fn kotlin_array_factory_call_root_spelling_parses_single_element_access() {
        use super::kotlin_array_factory_call_root_spelling;
        assert_eq!(
            kotlin_array_factory_call_root_spelling("makeItems()"),
            Some("makeItems".to_string())
        );
        assert_eq!(
            kotlin_array_factory_call_root_spelling("makeItems(1)"),
            Some("makeItems".to_string())
        );
        assert_eq!(
            kotlin_array_factory_call_root_spelling("Util.makeItems()"),
            Some("Util.makeItems".to_string())
        );
        assert_eq!(
            kotlin_array_factory_call_root_spelling("group.makeItems(1)"),
            Some("group.makeItems".to_string())
        );
        // Non-call bases, `this`/`super` roots, empty names, and malformed
        // spellings fail closed.
        assert_eq!(kotlin_array_factory_call_root_spelling("makeItems"), None);
        assert_eq!(kotlin_array_factory_call_root_spelling("()"), None);
        assert_eq!(
            kotlin_array_factory_call_root_spelling("makeItems()x"),
            None
        );
        assert_eq!(
            kotlin_array_factory_call_root_spelling("this.makeItems()"),
            None
        );
        assert_eq!(
            kotlin_array_factory_call_root_spelling("super.makeItems()"),
            None
        );
    }

    #[test]
    fn csharp_array_factory_call_root_spelling_parses_single_element_access() {
        use super::csharp_array_factory_call_root_spelling;
        assert_eq!(
            csharp_array_factory_call_root_spelling("makeItems()[0]"),
            Some(("makeItems".to_string(), 0, 1))
        );
        assert_eq!(
            csharp_array_factory_call_root_spelling("makeItems(1)[2]"),
            Some(("makeItems".to_string(), 1, 1))
        );
        assert_eq!(
            csharp_array_factory_call_root_spelling("makeItems()[]"),
            Some(("makeItems".to_string(), 0, 1))
        );
        // Jagged element access records the depth: two bracket groups strip
        // two component layers, and a multi-dimensional group counts once.
        assert_eq!(
            csharp_array_factory_call_root_spelling("makeMatrix()[0][0]"),
            Some(("makeMatrix".to_string(), 0, 2))
        );
        assert_eq!(
            csharp_array_factory_call_root_spelling("makeCube()[0][0][0]"),
            Some(("makeCube".to_string(), 0, 3))
        );
        assert_eq!(
            csharp_array_factory_call_root_spelling("makeGrid()[0, 0]"),
            Some(("makeGrid".to_string(), 0, 1))
        );
        // Dotted roots and malformed spellings fail closed.
        assert_eq!(
            csharp_array_factory_call_root_spelling("Util.makeItems()[0]"),
            None
        );
        assert_eq!(csharp_array_factory_call_root_spelling("makeItems()"), None);
        assert_eq!(
            csharp_array_factory_call_root_spelling("makeItems[0]"),
            None
        );
        assert_eq!(csharp_array_factory_call_root_spelling("[0]"), None);
        assert_eq!(csharp_array_factory_call_root_spelling("()"), None);
        assert_eq!(
            csharp_array_factory_call_root_spelling("makeItems()[0]x"),
            None
        );
    }
}
