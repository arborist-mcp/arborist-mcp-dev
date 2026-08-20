use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use crate::deadline::DeadlineCheck;
use crate::language::{
    detect_language, go_local_package_imports_with_deadline, go_source_package_name,
    normalize_path, parse_document, parse_document_with_timeout, read_source,
};
use crate::model::LanguageId;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct GoImportBinding {
    pub(crate) package_paths: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct GoImportContext {
    package_name: Option<String>,
    bindings: BTreeMap<String, GoImportBinding>,
}

fn go_import_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<GoImportContext> {
    let path = Path::new(file_path);
    if detect_language(path).ok() != Some(LanguageId::Go) {
        return Ok(GoImportContext::default());
    }

    if let Some(deadline) = deadline {
        deadline.check("reading Go import context")?;
    }
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    if let Some(deadline) = deadline {
        deadline.check("parsing Go import context")?;
    }
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing Go import context")?,
        )?
    } else {
        parse_document(path, &source)?
    };
    let root = document.tree.root_node();
    if root.has_error() {
        return Ok(GoImportContext::default());
    }

    let package_name = go_source_package_name(root, &source)?;
    let mut bindings = BTreeMap::new();
    let mut ambiguous_names = BTreeSet::new();
    for import in go_local_package_imports_with_deadline(
        path,
        root,
        &source,
        deadline.map(|deadline| deadline as &dyn DeadlineCheck),
    )? {
        if let Some(deadline) = deadline {
            deadline.check("extracting Go import bindings")?;
        }
        let package_paths = import
            .source_paths
            .into_iter()
            .filter(|path| is_production_go_source(path))
            .map(|path| normalize_path(&path))
            .collect::<BTreeSet<_>>();
        if package_paths.is_empty() {
            continue;
        }

        let local_name = match import.explicit_local_name {
            Some(name) => Some(name),
            None => go_package_name_for_paths(&package_paths, file_overrides, deadline)?,
        };
        let Some(local_name) = local_name else {
            continue;
        };
        if ambiguous_names.contains(&local_name) {
            continue;
        }

        let binding = GoImportBinding { package_paths };
        if bindings.insert(local_name.clone(), binding).is_some() {
            bindings.remove(&local_name);
            ambiguous_names.insert(local_name);
        }
    }

    Ok(GoImportContext {
        package_name,
        bindings,
    })
}

pub(in crate::symbol_dependency) fn go_package_name_for_source_file(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    Ok(
        go_import_context_from_cache(file_path, file_overrides, contexts_by_file, deadline)?
            .package_name,
    )
}

pub(in crate::symbol_dependency) fn resolve_go_import_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, GoImportBinding)>> {
    let Some((local_name, imported_name)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    if local_name.is_empty() || imported_name.is_empty() || imported_name.contains('.') {
        return Ok(None);
    }

    let context =
        go_import_context_from_cache(source_file_path, file_overrides, contexts_by_file, deadline)?;
    let Some(binding) = context.bindings.get(local_name) else {
        return Ok(None);
    };

    Ok(Some((
        imported_name.to_string(),
        GoImportBinding {
            package_paths: binding.package_paths.clone(),
        },
    )))
}

fn go_import_context_from_cache(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, GoImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<GoImportContext> {
    let normalized_file_path = normalize_path(Path::new(file_path));
    if let Some(context) = contexts_by_file.get(&normalized_file_path) {
        return Ok(context.clone());
    }

    let context = go_import_context_for_file_with_overrides_and_deadline(
        &normalized_file_path,
        file_overrides,
        deadline,
    )?;
    contexts_by_file.insert(normalized_file_path, context.clone());
    Ok(context)
}

fn go_package_name_for_paths(
    package_paths: &BTreeSet<String>,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let mut names = BTreeSet::new();
    for package_path in package_paths {
        if let Some(deadline) = deadline {
            deadline.check("reading imported Go package names")?;
        }
        let path = Path::new(package_path);
        let source = file_overrides
            .and_then(|overrides| overrides.get(package_path))
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| read_source(path))?;
        if let Some(deadline) = deadline {
            deadline.check("parsing imported Go package names")?;
        }
        let document = if let Some(deadline) = deadline {
            parse_document_with_timeout(
                path,
                &source,
                deadline.remaining_timeout_micros("parsing imported Go package names")?,
            )?
        } else {
            parse_document(path, &source)?
        };
        let root = document.tree.root_node();
        if root.has_error() {
            return Ok(None);
        }
        let Some(name) = go_source_package_name(root, &source)? else {
            return Ok(None);
        };
        names.insert(name);
    }

    Ok((names.len() == 1).then(|| names.pop_first().unwrap()))
}

fn is_production_go_source(path: &Path) -> bool {
    !path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.ends_with("_test"))
}
