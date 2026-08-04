use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use crate::language::{
    detect_language, java_local_explicit_type_imports, normalize_path, parse_document,
    parse_document_with_timeout, read_source,
};
use crate::model::LanguageId;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct JavaImportBinding {
    pub(crate) semantic_path: String,
    pub(crate) source_path: String,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct JavaImportContext {
    bindings: BTreeMap<String, JavaImportBinding>,
}

fn java_import_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaImportContext> {
    let path = Path::new(file_path);
    if detect_language(path).ok() != Some(LanguageId::Java) {
        return Ok(JavaImportContext::default());
    }

    if let Some(deadline) = deadline {
        deadline.check("reading Java import context")?;
    }
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    if let Some(deadline) = deadline {
        deadline.check("parsing Java import context")?;
    }
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing Java import context")?,
        )?
    } else {
        parse_document(path, &source)?
    };
    let root = document.tree.root_node();
    if root.has_error() {
        return Ok(JavaImportContext::default());
    }

    let mut bindings = BTreeMap::new();
    let mut ambiguous_names = BTreeSet::new();
    for import in java_local_explicit_type_imports(path, root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting Java import bindings")?;
        }
        if ambiguous_names.contains(&import.local_name) {
            continue;
        }
        let binding = JavaImportBinding {
            semantic_path: import.semantic_path,
            source_path: normalize_path(&import.source_path),
        };
        if bindings
            .insert(import.local_name.clone(), binding)
            .is_some()
        {
            bindings.remove(&import.local_name);
            ambiguous_names.insert(import.local_name);
        }
    }
    Ok(JavaImportContext { bindings })
}

pub(in crate::symbol_dependency) fn resolve_java_import_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, JavaImportBinding)>> {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return Ok(None);
    }

    let context = java_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    let Some(binding) = context.bindings.get(local_type_name) else {
        return Ok(None);
    };
    Ok(Some((method_name.to_string(), binding.clone())))
}

fn java_import_context_from_cache(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaImportContext> {
    let normalized_file_path = normalize_path(Path::new(file_path));
    if let Some(context) = contexts_by_file.get(&normalized_file_path) {
        return Ok(context.clone());
    }

    let context = java_import_context_for_file_with_overrides_and_deadline(
        &normalized_file_path,
        file_overrides,
        deadline,
    )?;
    contexts_by_file.insert(normalized_file_path, context.clone());
    Ok(context)
}
