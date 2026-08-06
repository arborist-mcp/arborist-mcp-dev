use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use anyhow::Result;

use crate::model::{SymbolMeta, SymbolMetaInit};
use crate::symbol_index_model::IndexedSymbol;
use crate::workspace_scan::WorkspaceScanDeadline;

use super::super::csharp::csharp_global_import_context_for_files_with_overrides_and_deadline;
use super::super::rust::rust_out_of_line_module_context_for_files_with_overrides_and_deadline;
use super::indexes::{build_name_index, build_semantic_path_index, raw_symbol_indexes_by_id};
use super::{resolve_dependencies_for_symbol, resolve_dependencies_for_symbol_with_deadline};

pub(crate) fn resolve_symbol_dependencies(
    raw_symbols: &[IndexedSymbol],
    source_file_paths: &[PathBuf],
) -> Vec<SymbolMeta> {
    resolve_symbol_dependencies_with_overrides(raw_symbols, source_file_paths, None)
}

pub(crate) fn resolve_symbol_dependencies_with_overrides(
    raw_symbols: &[IndexedSymbol],
    source_file_paths: &[PathBuf],
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Vec<SymbolMeta> {
    let name_index = build_name_index(raw_symbols);
    let semantic_path_index = build_semantic_path_index(raw_symbols);
    let symbol_indexes = raw_symbol_indexes_by_id(raw_symbols);
    let mut dependency_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut languages_by_file = HashMap::new();
    let mut include_contexts_by_file = HashMap::new();
    let mut javascript_import_contexts_by_file = BTreeMap::new();
    let mut go_import_contexts_by_file = BTreeMap::new();
    let mut java_import_contexts_by_file = BTreeMap::new();
    let mut kotlin_import_contexts_by_file = BTreeMap::new();
    let mut csharp_import_contexts_by_file = BTreeMap::new();
    let csharp_global_import_context =
        csharp_global_import_context_for_files_with_overrides_and_deadline(
            source_file_paths,
            file_overrides,
            None,
        )
        .expect("dependency resolution without a deadline cannot fail");

    let rust_out_of_line_module_context =
        rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            source_file_paths,
            file_overrides,
            None,
        )
        .expect("dependency resolution without a deadline cannot fail");

    for (symbol_id, indexes) in &symbol_indexes {
        let dependencies = dependency_map.entry(symbol_id.clone()).or_default();
        for index in indexes {
            dependencies.extend(resolve_dependencies_for_symbol(
                &raw_symbols[*index],
                raw_symbols,
                &name_index,
                &semantic_path_index,
                file_overrides,
                &mut languages_by_file,
                &mut include_contexts_by_file,
                &mut javascript_import_contexts_by_file,
                &mut go_import_contexts_by_file,
                &rust_out_of_line_module_context,
                &mut java_import_contexts_by_file,
                &mut kotlin_import_contexts_by_file,
                &mut csharp_import_contexts_by_file,
                Some(&csharp_global_import_context),
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
    source_file_paths: &[PathBuf],
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: &WorkspaceScanDeadline,
) -> Result<Vec<SymbolMeta>> {
    deadline.check("resolving symbol dependencies")?;
    let name_index = build_name_index(raw_symbols);
    let semantic_path_index = build_semantic_path_index(raw_symbols);
    let symbol_indexes = raw_symbol_indexes_by_id(raw_symbols);
    let mut dependency_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut languages_by_file = HashMap::new();
    let mut include_contexts_by_file = HashMap::new();
    let mut javascript_import_contexts_by_file = BTreeMap::new();
    let mut go_import_contexts_by_file = BTreeMap::new();
    let mut java_import_contexts_by_file = BTreeMap::new();
    let mut kotlin_import_contexts_by_file = BTreeMap::new();
    let mut csharp_import_contexts_by_file = BTreeMap::new();
    let csharp_global_import_context =
        csharp_global_import_context_for_files_with_overrides_and_deadline(
            source_file_paths,
            file_overrides,
            Some(deadline),
        )?;

    let rust_out_of_line_module_context =
        rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            source_file_paths,
            file_overrides,
            Some(deadline),
        )?;

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
                &mut languages_by_file,
                &mut include_contexts_by_file,
                &mut javascript_import_contexts_by_file,
                &mut go_import_contexts_by_file,
                &rust_out_of_line_module_context,
                &mut java_import_contexts_by_file,
                &mut kotlin_import_contexts_by_file,
                &mut csharp_import_contexts_by_file,
                Some(&csharp_global_import_context),
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::resolve_symbol_dependencies_with_overrides_with_deadline;
    use crate::workspace_scan::WorkspaceScanDeadline;

    #[test]
    fn deadline_resolver_rejects_expired_empty_input() {
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error =
            resolve_symbol_dependencies_with_overrides_with_deadline(&[], &[], None, &deadline)
                .expect_err("expired dependency resolution should fail before indexing");
        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }
}
