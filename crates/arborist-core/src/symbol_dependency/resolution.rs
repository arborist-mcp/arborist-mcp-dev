use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use super::c::{CIncludeContext, c_include_context_for_file, c_symbol_family_anchor};
use crate::language::{detect_language, is_c_header_path, normalize_path};
use crate::model::{LanguageId, SymbolMeta, SymbolMetaInit, SymbolSummary};
use crate::patching::{resolve_local_python_imported_symbol, resolve_local_python_module_path};
use crate::symbol_index_model::{IndexedSymbol, symbol_kind_rank};

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

pub(super) fn build_name_index(raw_symbols: &[IndexedSymbol]) -> BTreeMap<String, Vec<usize>> {
    let mut name_index = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        name_index
            .entry(symbol.base_name.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    name_index
}

pub(super) fn raw_symbol_indexes_by_id(
    raw_symbols: &[IndexedSymbol],
) -> BTreeMap<String, Vec<usize>> {
    let mut indexes = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        indexes
            .entry(symbol.symbol_id.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    indexes
}

pub(super) fn resolve_dependencies_for_symbol(
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

pub(super) fn indexed_symbol_rank(symbol: &IndexedSymbol) -> usize {
    symbol_kind_rank(&symbol.node_kind)
}

fn symbol_id_for_index(index: usize, raw_symbols: &[IndexedSymbol]) -> Result<String> {
    let symbol = &raw_symbols[index];
    let path = Path::new(&symbol.file_path);
    if !matches!(
        detect_language(path).ok(),
        Some(LanguageId::C | LanguageId::Cpp)
    ) || symbol.semantic_path.contains("::")
    {
        return Ok(symbol.semantic_path.clone());
    }

    let anchor = if is_c_header_path(path) {
        symbol.file_path.clone()
    } else {
        c_symbol_family_anchor(symbol, raw_symbols)?
    };

    Ok(format!("{anchor}::{}", symbol.base_name))
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
        let class_method_path = format!("{module_hint}.{lookup_name}");
        let filtered = candidate_slice
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
                source_symbol,
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
    source_symbol: &IndexedSymbol,
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

    if source_symbol_scope_matches(source_symbol, symbol) {
        rank += 500;
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

fn source_symbol_scope_matches(source_symbol: &IndexedSymbol, candidate: &IndexedSymbol) -> bool {
    detect_language(Path::new(&source_symbol.file_path)).ok() == Some(LanguageId::Cpp)
        && source_symbol.scope_path.is_some()
        && source_symbol.scope_path == candidate.scope_path
}
