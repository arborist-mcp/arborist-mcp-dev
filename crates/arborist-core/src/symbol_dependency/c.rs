use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;

use crate::language::{
    c_companion_source_path, c_include_targets, detect_language, is_c_header_path, normalize_path,
    parse_document, read_source, resolve_local_c_include,
};
use crate::model::LanguageId;
use crate::symbol_index_model::IndexedSymbol;

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

pub(super) fn c_symbol_family_anchor(
    symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
) -> Result<String> {
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

pub(super) fn same_stem(left: &Path, right: &Path) -> bool {
    left.file_stem()
        .and_then(|stem| stem.to_str())
        .zip(right.file_stem().and_then(|stem| stem.to_str()))
        .is_some_and(|(left_stem, right_stem)| left_stem == right_stem)
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
