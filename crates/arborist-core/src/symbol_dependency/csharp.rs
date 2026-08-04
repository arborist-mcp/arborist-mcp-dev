use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{
    csharp_file_namespace_imports, csharp_file_static_type_imports, csharp_file_type_alias_imports,
    csharp_global_static_type_imports, detect_language, normalize_path, parse_document,
    parse_document_with_timeout, read_source,
};
use crate::model::LanguageId;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpTypeAliasBinding {
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpStaticTypeImportBinding {
    pub(crate) scope_path: Option<String>,
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpNamespaceImportBinding {
    pub(crate) scope_path: Option<String>,
    pub(crate) semantic_namespace_path: String,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct CSharpImportContext {
    type_alias_bindings: BTreeMap<(Option<String>, String), CSharpTypeAliasBinding>,
    ambiguous_type_alias_names: BTreeSet<(Option<String>, String)>,
    static_type_import_bindings: Vec<CSharpStaticTypeImportBinding>,
    namespace_import_bindings: Vec<CSharpNamespaceImportBinding>,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct CSharpGlobalImportContext {
    static_type_import_bindings: Vec<CSharpStaticTypeImportBinding>,
}

pub(in crate::symbol_dependency) fn csharp_global_import_context_for_files_with_overrides_and_deadline(
    source_file_paths: &[PathBuf],
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpGlobalImportContext> {
    let mut static_type_import_bindings = Vec::new();
    let mut visited_paths = BTreeSet::new();

    for source_file_path in source_file_paths {
        if let Some(deadline) = deadline {
            deadline.check("reading C# global import context")?;
        }
        let normalized_file_path = normalize_path(source_file_path);
        if !visited_paths.insert(normalized_file_path.clone()) {
            continue;
        }
        let path = Path::new(&normalized_file_path);
        if detect_language(path).ok() != Some(LanguageId::CSharp) {
            continue;
        }
        let source = file_overrides
            .and_then(|overrides| overrides.get(&normalized_file_path))
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| read_source(path))?;
        if let Some(deadline) = deadline {
            deadline.check("parsing C# global import context")?;
        }
        let document = if let Some(deadline) = deadline {
            parse_document_with_timeout(
                path,
                &source,
                deadline.remaining_timeout_micros("parsing C# global import context")?,
            )?
        } else {
            parse_document(path, &source)?
        };
        let root = document.tree.root_node();
        if root.has_error() {
            continue;
        }
        for semantic_type_path in csharp_global_static_type_imports(root, &source)? {
            if let Some(deadline) = deadline {
                deadline.check("extracting C# global static import bindings")?;
            }
            static_type_import_bindings.push(CSharpStaticTypeImportBinding {
                scope_path: None,
                semantic_type_path,
            });
        }
    }

    Ok(CSharpGlobalImportContext {
        static_type_import_bindings,
    })
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_static_type_imports_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Vec<CSharpStaticTypeImportBinding> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Vec::new();
    }
    context.static_type_import_bindings.clone()
}

fn csharp_import_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpImportContext> {
    let path = Path::new(file_path);
    if detect_language(path).ok() != Some(LanguageId::CSharp) {
        return Ok(CSharpImportContext::default());
    }

    if let Some(deadline) = deadline {
        deadline.check("reading C# import context")?;
    }
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    if let Some(deadline) = deadline {
        deadline.check("parsing C# import context")?;
    }
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing C# import context")?,
        )?
    } else {
        parse_document(path, &source)?
    };
    let root = document.tree.root_node();
    if root.has_error() {
        return Ok(CSharpImportContext::default());
    }

    let mut type_alias_bindings = BTreeMap::new();
    let mut ambiguous_alias_names = BTreeSet::new();
    for import in csharp_file_type_alias_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# type alias bindings")?;
        }
        insert_unique_csharp_type_alias_binding(
            &mut type_alias_bindings,
            &mut ambiguous_alias_names,
            import.scope_path,
            import.local_name,
            CSharpTypeAliasBinding {
                semantic_type_path: import.semantic_type_path,
            },
        );
    }
    let mut static_type_import_bindings = Vec::new();
    for import in csharp_file_static_type_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# static type import bindings")?;
        }
        static_type_import_bindings.push(CSharpStaticTypeImportBinding {
            scope_path: import.scope_path,
            semantic_type_path: import.semantic_type_path,
        });
    }
    let mut namespace_import_bindings = Vec::new();
    for import in csharp_file_namespace_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# namespace import bindings")?;
        }
        namespace_import_bindings.push(CSharpNamespaceImportBinding {
            scope_path: import.scope_path,
            semantic_namespace_path: import.semantic_namespace_path,
        });
    }
    Ok(CSharpImportContext {
        type_alias_bindings,
        ambiguous_type_alias_names: ambiguous_alias_names,
        static_type_import_bindings,
        namespace_import_bindings,
    })
}

fn insert_unique_csharp_type_alias_binding(
    bindings: &mut BTreeMap<(Option<String>, String), CSharpTypeAliasBinding>,
    ambiguous_names: &mut BTreeSet<(Option<String>, String)>,
    scope_path: Option<String>,
    local_name: String,
    binding: CSharpTypeAliasBinding,
) {
    let key = (scope_path, local_name);
    if ambiguous_names.contains(&key) {
        return;
    }
    if bindings.insert(key.clone(), binding).is_some() {
        bindings.remove(&key);
        ambiguous_names.insert(key);
    }
}

pub(in crate::symbol_dependency) fn resolve_csharp_type_alias_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, CSharpTypeAliasBinding)>> {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return Ok(None);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    for scope_path in csharp_import_scope_paths(source_namespace_path) {
        let key = (scope_path, local_type_name.to_string());
        if let Some(binding) = context.type_alias_bindings.get(&key) {
            return Ok(Some((method_name.to_string(), binding.clone())));
        }
    }
    Ok(None)
}

pub(in crate::symbol_dependency) fn csharp_type_alias_name_is_ambiguous_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<bool> {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return Ok(false);
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return Ok(false);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .map(|scope_path| (scope_path, local_type_name.to_string()))
        .any(|key| context.ambiguous_type_alias_names.contains(&key)))
}

fn csharp_import_scope_paths(source_namespace_path: Option<&str>) -> Vec<Option<String>> {
    let mut scope_paths = source_namespace_path
        .map(|source_namespace_path| vec![Some(source_namespace_path.to_string())])
        .unwrap_or_default();
    scope_paths.push(None);
    scope_paths
}

pub(in crate::symbol_dependency) fn resolve_csharp_static_type_imports_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<CSharpStaticTypeImportBinding>> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Ok(Vec::new());
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .flat_map(|scope_path| {
            context
                .static_type_import_bindings
                .iter()
                .filter(move |binding| binding.scope_path == scope_path)
                .cloned()
        })
        .collect())
}

pub(in crate::symbol_dependency) fn resolve_csharp_namespace_imports_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<CSharpNamespaceImportBinding>> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Ok(Vec::new());
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .flat_map(|scope_path| {
            context
                .namespace_import_bindings
                .iter()
                .filter(move |binding| binding.scope_path == scope_path)
                .cloned()
        })
        .collect())
}

fn csharp_import_context_from_cache(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpImportContext> {
    let normalized_file_path = normalize_path(Path::new(file_path));
    if let Some(context) = contexts_by_file.get(&normalized_file_path) {
        return Ok(context.clone());
    }

    let context = csharp_import_context_for_file_with_overrides_and_deadline(
        &normalized_file_path,
        file_overrides,
        deadline,
    )?;
    contexts_by_file.insert(normalized_file_path, context.clone());
    Ok(context)
}
