use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use crate::language::{
    c_companion_source_path, c_include_targets, detect_language, is_c_header_path, normalize_path,
    parse_document, read_source, resolve_local_c_include,
};
use crate::model::{LanguageId, SymbolMeta, SymbolMetaInit, SymbolSummary};
use crate::patching::{resolve_local_python_imported_symbol, resolve_local_python_module_path};
use crate::symbol_index_model::{IndexedSymbol, symbol_kind_rank};

#[derive(Debug, Default)]
pub(crate) struct CIncludeContext {
    pub(crate) include_paths: BTreeSet<String>,
    pub(crate) companion_source_paths: BTreeSet<String>,
}

pub(crate) fn c_include_context_for_file(file_path: &str) -> Result<CIncludeContext> {
    let path = Path::new(file_path);
    if detect_language(path).ok() != Some(LanguageId::C) {
        return Ok(CIncludeContext::default());
    }

    let mut include_paths = BTreeSet::new();
    let mut visited = BTreeSet::new();
    collect_c_include_closure(path, &mut include_paths, &mut visited)?;

    let companion_source_paths = include_paths
        .iter()
        .filter_map(|include_path| {
            c_companion_source_path(Path::new(include_path))
                .map(|candidate| normalize_path(&candidate))
        })
        .collect();

    Ok(CIncludeContext {
        include_paths,
        companion_source_paths,
    })
}

pub(crate) fn assign_symbol_ids(raw_symbols: &mut [IndexedSymbol]) -> Result<()> {
    let symbol_ids = (0..raw_symbols.len())
        .map(|index| symbol_id_for_index(index, raw_symbols))
        .collect::<Result<Vec<_>>>()?;

    for (symbol, symbol_id) in raw_symbols.iter_mut().zip(symbol_ids) {
        symbol.symbol_id = symbol_id;
    }

    Ok(())
}

pub(crate) fn resolve_symbol_dependencies(raw_symbols: &[IndexedSymbol]) -> Vec<SymbolMeta> {
    let name_index = build_name_index(raw_symbols);
    let symbol_indexes = raw_symbol_indexes_by_id(raw_symbols);
    let mut dependency_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for (symbol_id, indexes) in &symbol_indexes {
        let dependencies = dependency_map.entry(symbol_id.clone()).or_default();
        for index in indexes {
            dependencies.extend(resolve_dependencies_for_symbol(
                &raw_symbols[*index],
                raw_symbols,
                &name_index,
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

pub(crate) fn refresh_resolved_symbol_subgraph(
    raw_symbols: &[IndexedSymbol],
    old_resolved_map: &BTreeMap<String, SymbolMeta>,
    old_changed_symbols: &[IndexedSymbol],
    new_changed_symbols: &[IndexedSymbol],
    changed_file_paths: &BTreeSet<String>,
) -> (BTreeMap<String, SymbolMeta>, BTreeSet<String>) {
    let name_index = build_name_index(raw_symbols);
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

fn collect_c_include_closure(
    path: &Path,
    include_paths: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    let normalized_path = normalize_path(path);
    if !visited.insert(normalized_path) {
        return Ok(());
    }

    let source = read_source(path)?;
    let document = parse_document(path, &source)?;
    for include_target in c_include_targets(document.tree.root_node(), &source)? {
        let Some(include_path) = resolve_local_c_include(path, &include_target) else {
            continue;
        };
        let normalized_include = normalize_path(&include_path);
        if include_paths.insert(normalized_include) {
            collect_c_include_closure(&include_path, include_paths, visited)?;
        }
    }

    Ok(())
}

fn build_name_index(raw_symbols: &[IndexedSymbol]) -> BTreeMap<String, Vec<usize>> {
    let mut name_index = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        name_index
            .entry(symbol.base_name.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    name_index
}

fn symbol_id_for_index(index: usize, raw_symbols: &[IndexedSymbol]) -> Result<String> {
    let symbol = &raw_symbols[index];
    let path = Path::new(&symbol.file_path);
    if detect_language(path).ok() != Some(LanguageId::C) || symbol.semantic_path.contains("::") {
        return Ok(symbol.semantic_path.clone());
    }

    let anchor = if is_c_header_path(path) {
        symbol.file_path.clone()
    } else {
        c_symbol_family_anchor(symbol, raw_symbols)?
    };

    Ok(format!("{anchor}::{}", symbol.base_name))
}

fn c_symbol_family_anchor(symbol: &IndexedSymbol, raw_symbols: &[IndexedSymbol]) -> Result<String> {
    let include_context = c_include_context_for_file(&symbol.file_path)?;
    let source_path = Path::new(&symbol.file_path);

    let best_header = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.semantic_path == symbol.semantic_path
                && !candidate.semantic_path.contains("::")
                && is_c_header_path(Path::new(&candidate.file_path))
        })
        .map(|candidate| {
            let rank = c_family_header_rank(source_path, &candidate.file_path, &include_context);
            (candidate, rank)
        })
        .filter(|(_, rank)| *rank > 0)
        .max_by_key(|(_, rank)| *rank)
        .map(|(candidate, _)| candidate);

    Ok(best_header
        .map(|candidate| candidate.file_path.clone())
        .unwrap_or_else(|| symbol.file_path.clone()))
}

fn c_family_header_rank(
    source_path: &Path,
    header_file_path: &str,
    include_context: &CIncludeContext,
) -> usize {
    let mut rank = 0;
    let header_path = Path::new(header_file_path);
    if same_stem(source_path, header_path) {
        rank += 1000;
    }
    if include_context.include_paths.contains(header_file_path) {
        rank += 500;
    }
    rank
}

fn same_stem(left: &Path, right: &Path) -> bool {
    left.file_stem()
        .and_then(|stem| stem.to_str())
        .zip(right.file_stem().and_then(|stem| stem.to_str()))
        .is_some_and(|(left_stem, right_stem)| left_stem == right_stem)
}

fn raw_symbol_indexes_by_id(raw_symbols: &[IndexedSymbol]) -> BTreeMap<String, Vec<usize>> {
    let mut indexes = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        indexes
            .entry(symbol.symbol_id.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    indexes
}

fn resolve_dependencies_for_symbol(
    symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
) -> Vec<String> {
    let mut dependencies = BTreeSet::new();
    for reference_name in &symbol.references_by_name {
        if let Some(target_symbol_id) =
            resolve_reference_path(reference_name, symbol, raw_symbols, name_index)
            && target_symbol_id != symbol.symbol_id
        {
            dependencies.insert(target_symbol_id);
        }
    }
    dependencies.into_iter().collect()
}

fn resolve_reference_path(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
) -> Option<String> {
    let language_id = detect_language(Path::new(&source_symbol.file_path)).ok();
    let (lookup_name, module_hint) = if language_id == Some(LanguageId::Python) {
        python_reference_lookup(reference_name)
    } else {
        (reference_name, None)
    };
    let candidates = name_index.get(lookup_name)?;
    let visible_candidates: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.file_path == source_symbol.file_path
                || !candidate.semantic_path.contains("::")
        })
        .collect();
    let candidate_slice = if visible_candidates.is_empty() {
        candidates.as_slice()
    } else {
        visible_candidates.as_slice()
    };
    let hinted_candidates = if let Some(module_hint) = module_hint {
        let imported_summary = resolve_local_python_imported_symbol(
            Path::new(&source_symbol.file_path),
            module_hint,
            lookup_name,
        )
        .ok()
        .flatten();
        let filtered = candidate_slice
            .iter()
            .copied()
            .filter(|index| {
                python_symbol_matches_module_hint(
                    source_symbol,
                    &raw_symbols[*index],
                    module_hint,
                    imported_summary.as_ref(),
                )
            })
            .collect::<Vec<_>>();
        if filtered.is_empty() {
            candidate_slice.to_vec()
        } else {
            filtered
        }
    } else {
        candidate_slice.to_vec()
    };
    let include_context = c_include_context_for_file(&source_symbol.file_path).ok();

    hinted_candidates
        .iter()
        .copied()
        .max_by_key(|index| {
            indexed_symbol_candidate_rank(
                &raw_symbols[*index],
                Some(&source_symbol.file_path),
                include_context.as_ref(),
            )
        })
        .map(|index| raw_symbols[index].symbol_id.clone())
}

fn python_reference_lookup(reference_name: &str) -> (&str, Option<&str>) {
    reference_name
        .rsplit_once('.')
        .map(|(module_hint, symbol_name)| (symbol_name, Some(module_hint)))
        .unwrap_or((reference_name, None))
}

fn python_symbol_matches_module_hint(
    source_symbol: &IndexedSymbol,
    symbol: &IndexedSymbol,
    module_hint: &str,
    imported_summary: Option<&SymbolSummary>,
) -> bool {
    if let Some(imported_summary) = imported_summary {
        return imported_summary.file_path == symbol.file_path
            && imported_summary.semantic_path == symbol.semantic_path;
    }

    let Some(resolved_module_path) =
        resolve_local_python_module_path(Path::new(&source_symbol.file_path), module_hint)
    else {
        return false;
    };

    normalize_path(&resolved_module_path) == symbol.file_path
}

fn indexed_symbol_candidate_rank(
    symbol: &IndexedSymbol,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> usize {
    let mut rank = indexed_symbol_rank(symbol);

    if let Some(context_file) = context_file {
        if symbol.file_path == context_file {
            rank += 1000;
        } else if symbol.semantic_path.contains("::") {
            rank = rank.saturating_sub(100);
        }
    }

    if let Some(include_context) = include_context {
        if include_context.include_paths.contains(&symbol.file_path) {
            rank += 200;
        }
        if include_context
            .companion_source_paths
            .contains(&symbol.file_path)
        {
            rank += 300;
        }
    }

    rank
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
    reference_name
        .rsplit('.')
        .next()
        .unwrap_or(reference_name)
        .to_string()
}

fn indexed_symbol_rank(symbol: &IndexedSymbol) -> usize {
    symbol_kind_rank(&symbol.node_kind)
}
