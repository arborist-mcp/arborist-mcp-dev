use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::c::CIncludeContext;
use super::csharp::{
    CSharpImportContext, csharp_global_import_context_for_files_with_overrides_and_deadline,
};
use super::go::GoImportContext;
use super::java::JavaImportContext;
use super::kotlin::KotlinImportContext;
use super::resolution::{
    build_name_index, build_semantic_path_index, cpp_template_base_path, indexed_symbol_rank,
    raw_symbol_indexes_by_id, resolve_dependencies_for_symbol_with_deadline,
};
use super::rust::rust_out_of_line_module_context_for_files_with_overrides_and_deadline;
use crate::model::{LanguageId, SymbolMeta, SymbolMetaInit};
use crate::symbol_index_model::IndexedSymbol;
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) struct RefreshResolutionInputs<'a> {
    pub(crate) source_file_paths: &'a [PathBuf],
    pub(crate) file_overrides: Option<&'a BTreeMap<String, String>>,
    pub(crate) deadline: Option<&'a WorkspaceScanDeadline>,
}

pub(crate) fn refresh_resolved_symbol_subgraph(
    raw_symbols: &[IndexedSymbol],
    old_resolved_map: &BTreeMap<String, SymbolMeta>,
    old_changed_symbols: &[IndexedSymbol],
    new_changed_symbols: &[IndexedSymbol],
    changed_file_paths: &BTreeSet<String>,
    inputs: RefreshResolutionInputs<'_>,
) -> Result<(BTreeMap<String, SymbolMeta>, BTreeSet<String>)> {
    let RefreshResolutionInputs {
        source_file_paths,
        file_overrides,
        deadline,
    } = inputs;
    let name_index = build_name_index(raw_symbols);
    if let Some(deadline) = deadline {
        deadline.check("building refresh symbol indexes")?;
    }
    let semantic_path_index = build_semantic_path_index(raw_symbols);
    if let Some(deadline) = deadline {
        deadline.check("building refresh symbol indexes")?;
    }
    let raw_symbol_indexes = raw_symbol_indexes_by_id(raw_symbols);
    if let Some(deadline) = deadline {
        deadline.check("building refresh symbol indexes")?;
    }
    let representative_raw_symbols = raw_symbol_map(raw_symbols);
    let refresh_csharp_symbols = changed_file_paths.iter().any(|path| path_is_csharp(path));
    let impacted_ids = impacted_symbol_ids(
        raw_symbols,
        old_changed_symbols,
        new_changed_symbols,
        old_resolved_map,
        changed_file_paths,
        refresh_csharp_symbols,
    );
    let csharp_global_import_context = if impacted_ids.iter().any(|symbol_id| {
        representative_raw_symbols
            .get(symbol_id)
            .is_some_and(indexed_symbol_is_csharp)
    }) {
        Some(
            csharp_global_import_context_for_files_with_overrides_and_deadline(
                source_file_paths,
                file_overrides,
                deadline,
            )?,
        )
    } else {
        None
    };

    let rust_out_of_line_module_context =
        rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            source_file_paths,
            file_overrides,
            deadline,
        )?;

    let mut resolved_map = old_resolved_map.clone();
    let mut languages_by_file: HashMap<&str, Option<LanguageId>> = HashMap::new();
    let mut include_contexts_by_file: HashMap<&str, Option<CIncludeContext>> = HashMap::new();
    let mut javascript_import_contexts_by_file = BTreeMap::new();
    let mut go_import_contexts_by_file = BTreeMap::<String, GoImportContext>::new();
    let mut java_import_contexts_by_file = BTreeMap::<String, JavaImportContext>::new();
    let mut kotlin_import_contexts_by_file = BTreeMap::<String, KotlinImportContext>::new();
    let mut csharp_import_contexts_by_file = BTreeMap::<String, CSharpImportContext>::new();
    for symbol in old_changed_symbols {
        resolved_map.remove(&symbol.symbol_id);
    }

    for impacted_id in &impacted_ids {
        if let Some(deadline) = deadline {
            deadline.check("refreshing impacted symbols")?;
        }
        let Some(raw_symbol) = representative_raw_symbols.get(impacted_id) else {
            resolved_map.remove(impacted_id);
            continue;
        };

        let Some(indexes) = raw_symbol_indexes.get(impacted_id) else {
            continue;
        };

        let mut symbol = symbol_meta_from_indexed(raw_symbol);
        let mut dependencies = BTreeSet::new();
        for index in indexes {
            if let Some(deadline) = deadline {
                deadline.check("refreshing impacted symbols")?;
            }
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
                csharp_global_import_context.as_ref(),
                deadline,
            )?);
        }
        symbol.dependencies = dependencies.into_iter().collect();
        resolved_map.insert(impacted_id.clone(), symbol);
    }

    let reference_impacted_paths =
        reference_impacted_paths(old_resolved_map, &resolved_map, &impacted_ids);
    let mut persistence_impacted_paths = impacted_ids.clone();
    persistence_impacted_paths.extend(reference_impacted_paths.iter().cloned());

    for impacted_path in reference_impacted_paths {
        if let Some(deadline) = deadline {
            deadline.check("refreshing impacted references")?;
        }
        let mut callers = Vec::new();
        for (caller_path, symbol) in &resolved_map {
            if let Some(deadline) = deadline {
                deadline.check("refreshing impacted references")?;
            }
            if symbol
                .dependencies
                .iter()
                .any(|dependency| dependency == &impacted_path)
            {
                callers.push(caller_path.clone());
            }
        }

        if let Some(symbol) = resolved_map.get_mut(&impacted_path) {
            symbol.references = callers;
        }
    }

    Ok((resolved_map, persistence_impacted_paths))
}

pub(crate) fn materialize_resolved_symbol_rows(
    raw_symbols: &[IndexedSymbol],
    resolved_map: &BTreeMap<String, SymbolMeta>,
) -> Vec<SymbolMeta> {
    raw_symbols
        .iter()
        .filter_map(|raw_symbol| {
            resolved_map
                .get(&raw_symbol.symbol_id)
                .map(|resolved_symbol| {
                    SymbolMeta::new(SymbolMetaInit {
                        symbol_id: raw_symbol.symbol_id.clone(),
                        semantic_path: raw_symbol.semantic_path.clone(),
                        scope_path: raw_symbol.scope_path.clone(),
                        file_path: raw_symbol.file_path.clone(),
                        node_kind: raw_symbol.node_kind.clone(),
                        origin_type: "workspace_symbol".to_string(),
                        byte_range: raw_symbol.byte_range,
                        signature: raw_symbol.signature.clone(),
                        parameters: raw_symbol.parameters.clone(),
                        return_type: raw_symbol.return_type.clone(),
                        docstring: raw_symbol.docstring.clone(),
                        dependencies: resolved_symbol.dependencies.clone(),
                        references: resolved_symbol.references.clone(),
                    })
                })
        })
        .collect()
}

fn raw_symbol_map(symbols: &[IndexedSymbol]) -> BTreeMap<String, IndexedSymbol> {
    let mut map = BTreeMap::new();
    for symbol in symbols {
        map.entry(symbol.symbol_id.clone())
            .and_modify(|existing| {
                if indexed_symbol_rank(symbol) > indexed_symbol_rank(existing) {
                    *existing = symbol.clone();
                }
            })
            .or_insert_with(|| symbol.clone());
    }
    map
}

fn symbol_meta_from_indexed(symbol: &IndexedSymbol) -> SymbolMeta {
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
        dependencies: Vec::new(),
        references: Vec::new(),
    })
}

fn impacted_symbol_ids(
    raw_symbols: &[IndexedSymbol],
    old_changed_symbols: &[IndexedSymbol],
    new_changed_symbols: &[IndexedSymbol],
    old_resolved_map: &BTreeMap<String, SymbolMeta>,
    changed_file_paths: &BTreeSet<String>,
    refresh_csharp_symbols: bool,
) -> BTreeSet<String> {
    let impacted_names: BTreeSet<_> = old_changed_symbols
        .iter()
        .chain(new_changed_symbols.iter())
        .map(|symbol| symbol.base_name.clone())
        .collect();
    let changed_reference_names: BTreeSet<_> = old_changed_symbols
        .iter()
        .chain(new_changed_symbols.iter())
        .flat_map(|symbol| {
            symbol
                .references_by_name
                .iter()
                .map(|reference| reference_base_name(reference))
                .collect::<Vec<_>>()
        })
        .collect();

    let mut impacted_ids: BTreeSet<_> = old_changed_symbols
        .iter()
        .chain(new_changed_symbols.iter())
        .map(|symbol| symbol.symbol_id.clone())
        .collect();
    if refresh_csharp_symbols {
        impacted_ids.extend(
            raw_symbols
                .iter()
                .filter(|symbol| indexed_symbol_is_csharp(symbol))
                .map(|symbol| symbol.symbol_id.clone()),
        );
    }

    for symbol in raw_symbols {
        if changed_file_paths.contains(&symbol.file_path) {
            continue;
        }
        if symbol.base_name.is_empty() {
            continue;
        }
        if symbol
            .references_by_name
            .iter()
            .any(|reference_name| impacted_names.contains(&reference_base_name(reference_name)))
            || changed_reference_names.contains(&symbol.base_name)
        {
            impacted_ids.insert(symbol.symbol_id.clone());
        }
    }

    let seed_ids: Vec<_> = impacted_ids.iter().cloned().collect();
    for symbol_id in seed_ids {
        if let Some(symbol) = old_resolved_map.get(&symbol_id) {
            impacted_ids.extend(symbol.dependencies.iter().cloned());
            impacted_ids.extend(symbol.references.iter().cloned());
        }
    }

    impacted_ids
}

fn path_is_csharp(path: &str) -> bool {
    matches!(
        crate::language::detect_language(Path::new(path)),
        Ok(LanguageId::CSharp)
    )
}

fn indexed_symbol_is_csharp(symbol: &IndexedSymbol) -> bool {
    path_is_csharp(&symbol.file_path)
}

fn reference_impacted_paths(
    old_resolved_map: &BTreeMap<String, SymbolMeta>,
    new_resolved_map: &BTreeMap<String, SymbolMeta>,
    impacted_paths: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut reference_paths = impacted_paths.clone();

    for impacted_path in impacted_paths {
        if let Some(symbol) = old_resolved_map.get(impacted_path) {
            reference_paths.extend(symbol.dependencies.iter().cloned());
            reference_paths.extend(symbol.references.iter().cloned());
        }
        if let Some(symbol) = new_resolved_map.get(impacted_path) {
            reference_paths.extend(symbol.dependencies.iter().cloned());
            reference_paths.extend(symbol.references.iter().cloned());
        }
    }

    reference_paths
}

fn reference_base_name(reference_name: &str) -> String {
    let template_base_path = cpp_template_base_path(reference_name);
    let reference_name = template_base_path.as_deref().unwrap_or(reference_name);
    let reference_name = reference_name
        .rsplit_once("::")
        .map(|(_, name)| name)
        .unwrap_or(reference_name);
    reference_name
        .rsplit('.')
        .next()
        .unwrap_or(reference_name)
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::{Duration, Instant};

    use crate::workspace_scan::WorkspaceScanDeadline;

    use super::{RefreshResolutionInputs, refresh_resolved_symbol_subgraph};

    #[test]
    fn refresh_subgraph_rejects_expired_deadline_before_building_indexes() {
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = refresh_resolved_symbol_subgraph(
            &[],
            &BTreeMap::new(),
            &[],
            &[],
            &BTreeSet::new(),
            RefreshResolutionInputs {
                source_file_paths: &[],
                file_overrides: None,
                deadline: Some(&deadline),
            },
        )
        .expect_err("expired deadline should stop subgraph refresh");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }
}
