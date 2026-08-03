use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use crate::language::{
    detect_language, javascript_named_import_module_paths, normalize_path, parse_document,
    parse_document_with_timeout, read_source,
};
use crate::model::LanguageId;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JavaScriptImportBinding {
    pub(crate) imported_name: String,
    pub(crate) module_paths: BTreeSet<String>,
}

#[derive(Debug, Default)]
pub(crate) struct JavaScriptImportContext {
    pub(crate) named_import_bindings: BTreeMap<String, JavaScriptImportBinding>,
}

pub(crate) fn javascript_import_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaScriptImportContext> {
    let path = Path::new(file_path);
    if !matches!(
        detect_language(path).ok(),
        Some(LanguageId::JavaScript | LanguageId::TypeScript | LanguageId::Tsx)
    ) {
        return Ok(JavaScriptImportContext::default());
    }

    if let Some(deadline) = deadline {
        deadline.check("reading JavaScript/TypeScript import context")?;
    }
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    if let Some(deadline) = deadline {
        deadline.check("parsing JavaScript/TypeScript import context")?;
    }
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing JavaScript/TypeScript import context")?,
        )?
    } else {
        parse_document(path, &source)?
    };
    if let Some(deadline) = deadline {
        deadline.check("extracting JavaScript/TypeScript import bindings")?;
    }
    let named_import_bindings =
        javascript_named_import_module_paths(path, document.tree.root_node(), &source)?
            .into_iter()
            .map(|(local_name, binding)| {
                (
                    local_name,
                    JavaScriptImportBinding {
                        imported_name: binding.imported_name,
                        module_paths: binding
                            .module_paths
                            .into_iter()
                            .map(|path| normalize_path(&path))
                            .collect(),
                    },
                )
            })
            .collect();

    Ok(JavaScriptImportContext {
        named_import_bindings,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::javascript_import_context_for_file_with_overrides_and_deadline;
    use crate::language::normalize_path;

    #[test]
    fn import_context_reads_source_overrides() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-import-context-overrides-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.ts");
        fs::write(&caller, "export function caller() {}\n").unwrap();
        fs::write(&helper, "export function helper() {}\n").unwrap();
        let mut overrides = BTreeMap::new();
        overrides.insert(
            normalize_path(&caller),
            "import { helper } from \"./helper\";\nexport function caller() { return helper(); }\n"
                .to_owned(),
        );

        let context = javascript_import_context_for_file_with_overrides_and_deadline(
            &normalize_path(&caller),
            Some(&overrides),
            None,
        )
        .unwrap();
        assert!(
            context
                .named_import_bindings
                .get("helper")
                .is_some_and(|binding| binding.module_paths.contains(&normalize_path(&helper)))
        );
        let _ = fs::remove_dir_all(root);
    }
}
