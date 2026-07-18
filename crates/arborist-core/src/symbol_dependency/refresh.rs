use std::collections::{BTreeMap, BTreeSet};

use super::resolution::{
    build_name_index, build_semantic_path_index, cpp_template_base_path, indexed_symbol_rank,
    raw_symbol_indexes_by_id, resolve_dependencies_for_symbol,
};
use crate::model::{SymbolMeta, SymbolMetaInit};
use crate::symbol_index_model::IndexedSymbol;

pub(crate) fn refresh_resolved_symbol_subgraph(
    raw_symbols: &[IndexedSymbol],
    old_resolved_map: &BTreeMap<String, SymbolMeta>,
    old_changed_symbols: &[IndexedSymbol],
    new_changed_symbols: &[IndexedSymbol],
    changed_file_paths: &BTreeSet<String>,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> (BTreeMap<String, SymbolMeta>, BTreeSet<String>) {
    let name_index = build_name_index(raw_symbols);
    let semantic_path_index = build_semantic_path_index(raw_symbols);
    let raw_symbol_indexes = raw_symbol_indexes_by_id(raw_symbols);
    let representative_raw_symbols = raw_symbol_map(raw_symbols);
    let impacted_ids = impacted_symbol_ids(
        raw_symbols,
        old_changed_symbols,
        new_changed_symbols,
        old_resolved_map,
        changed_file_paths,
    );

    let mut resolved_map = old_resolved_map.clone();
    for symbol in old_changed_symbols {
        resolved_map.remove(&symbol.symbol_id);
    }

    for impacted_id in &impacted_ids {
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
            dependencies.extend(resolve_dependencies_for_symbol(
                &raw_symbols[*index],
                raw_symbols,
                &name_index,
                &semantic_path_index,
                file_overrides,
            ));
        }
        symbol.dependencies = dependencies.into_iter().collect();
        resolved_map.insert(impacted_id.clone(), symbol);
    }

    let reference_impacted_paths =
        reference_impacted_paths(old_resolved_map, &resolved_map, &impacted_ids);

    for impacted_path in reference_impacted_paths {
        let callers = resolved_map
            .iter()
            .filter_map(|(caller_path, symbol)| {
                symbol
                    .dependencies
                    .iter()
                    .any(|dependency| dependency == &impacted_path)
                    .then_some(caller_path.clone())
            })
            .collect::<Vec<_>>();

        if let Some(symbol) = resolved_map.get_mut(&impacted_path) {
            symbol.references = callers;
        }
    }

    (resolved_map, impacted_ids)
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
