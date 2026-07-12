use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;

use crate::language::{
    ParsedDocument, c_include_targets, normalize_path, parse_document, read_source,
    resolve_local_c_include,
};
use crate::model::{DisambiguationContext, SymbolSummary};

pub(super) fn ambiguity_reason(candidates: &[SymbolSummary]) -> String {
    let distinct_families = candidates
        .iter()
        .filter_map(symbol_include_family)
        .collect::<BTreeSet<_>>();

    if distinct_families.len() > 1 {
        "multiple equally-ranked definitions across include families".to_string()
    } else {
        "multiple equally-ranked visible bindings".to_string()
    }
}

pub(super) fn ambiguity_disambiguation_context(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    candidates: &[SymbolSummary],
) -> Result<DisambiguationContext> {
    let visible_include_families = collect_visible_include_families(path, document, source)?
        .into_iter()
        .collect::<Vec<_>>();
    let candidate_include_families = ordered_candidate_include_families(candidates);
    let matched_visible_families = visible_include_families
        .iter()
        .filter(|family| candidate_include_families.contains(family))
        .cloned()
        .collect::<Vec<_>>();
    let preferred_family = if matched_visible_families.len() == 1 {
        matched_visible_families.first().cloned()
    } else {
        None
    };
    let active_include_family = if candidate_include_families.len() == 1 {
        candidate_include_families.first().cloned()
    } else {
        preferred_family.clone()
    };

    Ok(DisambiguationContext {
        active_include_family,
        preferred_family,
        visible_include_families,
        candidate_include_families,
        candidate_symbol_ids: candidates
            .iter()
            .map(|candidate| candidate.symbol_id.clone())
            .collect(),
    })
}

fn symbol_include_family(candidate: &SymbolSummary) -> Option<String> {
    candidate
        .symbol_id
        .rsplit_once("::")
        .map(|(family, _)| family.to_string())
}

fn ordered_candidate_include_families(candidates: &[SymbolSummary]) -> Vec<String> {
    let mut families = Vec::new();
    for family in candidates.iter().filter_map(symbol_include_family) {
        push_unique(&mut families, family);
    }
    families
}

fn collect_visible_include_families(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
) -> Result<Vec<String>> {
    let mut families = Vec::new();
    let mut visited = BTreeSet::new();
    collect_visible_include_families_from_document(
        path,
        document,
        source,
        &mut families,
        &mut visited,
    )?;
    Ok(families)
}

fn collect_visible_include_families_from_document(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    families: &mut Vec<String>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    let normalized = normalize_path(path);
    if !visited.insert(normalized) {
        return Ok(());
    }

    for include_target in c_include_targets(document.tree.root_node(), source)? {
        let Some(include_path) = resolve_local_c_include(path, &include_target) else {
            continue;
        };
        let include_family = normalize_path(&include_path);
        push_unique(families, include_family);

        let include_source = read_source(&include_path)?;
        let include_document = parse_document(&include_path, &include_source)?;
        collect_visible_include_families_from_document(
            &include_path,
            &include_document,
            &include_source,
            families,
            visited,
        )?;
    }

    Ok(())
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}
