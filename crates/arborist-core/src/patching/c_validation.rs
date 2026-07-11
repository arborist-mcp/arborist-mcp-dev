use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{
    ReferenceValidation, ambiguous_binding_decision, resolved_binding_decision,
    unresolved_binding_decision,
};
use crate::language::{
    ParsedDocument, c_companion_source_path, c_include_targets, contains_kind, first_identifier,
    node_text, normalize_path, parse_document, read_source, resolve_local_c_include, visit_tree,
};
use crate::model::{
    DisambiguationContext, SymbolSummary, SymbolSummaryInit, ValidationAmbiguity, ValidationBinding,
};
use crate::semantic::{
    c_parameters, c_return_type, c_semantic_path, c_symbol_id_for_node, semantic_parent_path,
};

#[derive(Debug, Clone)]
struct CAccessibleSymbol {
    name: String,
    summary: SymbolSummary,
    rank: usize,
}

struct CAccessibleCollection<'a> {
    base_rank: usize,
    origin_type: &'a str,
    allow_companion_sources: bool,
    context_file: &'a str,
}

struct CAccessibleState<'a> {
    symbols: &'a mut Vec<CAccessibleSymbol>,
    visited_files: &'a mut BTreeSet<String>,
    visited_companion_sources: &'a mut BTreeSet<String>,
}

pub(crate) fn collect_c_reference_validation(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
) -> Result<ReferenceValidation> {
    let mut accessible = BTreeSet::new();
    let mut visited = BTreeSet::new();
    collect_c_accessible_names(path, document, source, &mut accessible, &mut visited)?;
    let mut local_definitions = BTreeSet::new();
    collect_c_local_definitions(symbol_node, source, &mut local_definitions)?;

    let mut references = BTreeSet::new();
    collect_c_references(symbol_node, source, &mut references)?;

    let accessible_symbols = collect_c_accessible_symbols(path, document, source)?;
    let mut validation = ReferenceValidation::default();

    for name in references {
        if local_definitions.contains(&name) {
            continue;
        }

        let candidates = c_binding_candidates_for_name(&accessible_symbols, &name);
        match candidates.as_slice() {
            [] => {
                if !accessible.contains(&name) {
                    validation
                        .binding_decisions
                        .push(unresolved_binding_decision(&name));
                    validation.unresolved_identifiers.push(name);
                }
            }
            [single] => {
                validation
                    .binding_decisions
                    .push(resolved_binding_decision(&name, &single.summary));
                validation.resolved_identifiers.push(ValidationBinding {
                    name,
                    symbol: single.summary.clone(),
                });
            }
            _ => {
                let candidate_summaries = candidates
                    .into_iter()
                    .map(|candidate| candidate.summary)
                    .collect::<Vec<_>>();
                let reason = ambiguity_reason(&candidate_summaries);
                validation
                    .binding_decisions
                    .push(ambiguous_binding_decision(
                        &name,
                        &reason,
                        &candidate_summaries,
                    ));
                validation.ambiguous_identifiers.push(ValidationAmbiguity {
                    name,
                    reason,
                    disambiguation_context: ambiguity_disambiguation_context(
                        path,
                        document,
                        source,
                        &candidate_summaries,
                    )?,
                    candidates: candidate_summaries,
                });
            }
        }
    }

    Ok(validation)
}

fn ambiguity_reason(candidates: &[SymbolSummary]) -> String {
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

fn ambiguity_disambiguation_context(
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

fn collect_c_top_level_names(
    root: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "type_definition" | "function_definition" => {
                if let Some(name) = first_identifier(child, source)? {
                    names.insert(name);
                }
            }
            "declaration" => {
                if let Some(name) = first_identifier(child, source)? {
                    names.insert(name);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_c_accessible_names(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    names: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    let normalized = normalize_path(path);
    if !visited.insert(normalized) {
        return Ok(());
    }

    collect_c_top_level_names(document.tree.root_node(), source, names)?;
    for include_target in c_include_targets(document.tree.root_node(), source)? {
        let Some(include_path) = resolve_local_c_include(path, &include_target) else {
            continue;
        };
        let include_source = read_source(&include_path)?;
        let include_document = parse_document(&include_path, &include_source)?;
        collect_c_accessible_names(
            &include_path,
            &include_document,
            &include_source,
            names,
            visited,
        )?;
    }

    Ok(())
}

fn collect_c_accessible_symbols(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
) -> Result<Vec<CAccessibleSymbol>> {
    let mut symbols = Vec::new();
    let mut visited_files = BTreeSet::new();
    let mut visited_companion_sources = BTreeSet::new();
    let context_file = normalize_path(path);
    let mut state = CAccessibleState {
        symbols: &mut symbols,
        visited_files: &mut visited_files,
        visited_companion_sources: &mut visited_companion_sources,
    };
    collect_c_accessible_symbols_from_document(
        path,
        document,
        source,
        CAccessibleCollection {
            base_rank: 1000,
            origin_type: "local_file",
            allow_companion_sources: true,
            context_file: &context_file,
        },
        &mut state,
    )?;

    let mut deduped = std::collections::BTreeMap::new();
    for symbol in symbols {
        deduped
            .entry(symbol.summary.symbol_id.clone())
            .and_modify(|existing: &mut CAccessibleSymbol| {
                if symbol.rank > existing.rank {
                    *existing = symbol.clone();
                }
            })
            .or_insert(symbol);
    }

    Ok(deduped.into_values().collect())
}

fn collect_c_accessible_symbols_from_document(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    collection: CAccessibleCollection<'_>,
    state: &mut CAccessibleState<'_>,
) -> Result<()> {
    let normalized = normalize_path(path);
    if !state.visited_files.insert(normalized.clone()) {
        return Ok(());
    }

    collect_c_symbol_candidates_from_root(
        path,
        document.tree.root_node(),
        source,
        collection.base_rank,
        collection.origin_type,
        collection.context_file,
        state.symbols,
    )?;

    for include_target in c_include_targets(document.tree.root_node(), source)? {
        let Some(include_path) = resolve_local_c_include(path, &include_target) else {
            continue;
        };

        let include_source = read_source(&include_path)?;
        let include_document = parse_document(&include_path, &include_source)?;
        collect_c_accessible_symbols_from_document(
            &include_path,
            &include_document,
            &include_source,
            CAccessibleCollection {
                base_rank: 500,
                origin_type: "include_header",
                allow_companion_sources: true,
                context_file: collection.context_file,
            },
            state,
        )?;

        if collection.allow_companion_sources
            && let Some(companion_source_path) = c_companion_source_path(&include_path)
        {
            let normalized_companion = normalize_path(&companion_source_path);
            if state.visited_companion_sources.insert(normalized_companion) {
                let companion_source = read_source(&companion_source_path)?;
                let companion_document = parse_document(&companion_source_path, &companion_source)?;
                collect_c_symbol_candidates_from_root(
                    &companion_source_path,
                    companion_document.tree.root_node(),
                    &companion_source,
                    600,
                    "companion_source",
                    collection.context_file,
                    state.symbols,
                )?;
            }
        }
    }

    Ok(())
}

fn collect_c_symbol_candidates_from_root(
    path: &Path,
    root: Node<'_>,
    source: &str,
    base_rank: usize,
    origin_type: &str,
    context_file: &str,
    symbols: &mut Vec<CAccessibleSymbol>,
) -> Result<()> {
    let normalized_path = normalize_path(path);
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        let Some(name) = c_candidate_name(child, source)? else {
            continue;
        };
        let Some(semantic_path) = c_semantic_path(path, child, source)? else {
            continue;
        };
        if normalized_path != context_file && semantic_path.contains("::") {
            continue;
        }
        let Some(symbol_id) = c_symbol_id_for_node(path, child, source)? else {
            continue;
        };
        let scope_path = semantic_parent_path(&semantic_path);

        symbols.push(CAccessibleSymbol {
            name,
            summary: SymbolSummary::new(SymbolSummaryInit {
                symbol_id,
                semantic_path,
                scope_path,
                file_path: normalized_path.clone(),
                node_kind: child.kind().to_string(),
                origin_type: origin_type.to_string(),
                byte_range: (child.start_byte(), child.end_byte()),
                signature: c_candidate_signature(child, source)?,
                parameters: c_parameters(child, source)?,
                return_type: c_return_type(child, source)?,
                docstring: None,
            }),
            rank: base_rank + c_candidate_node_rank(child.kind()),
        });
    }

    Ok(())
}

fn c_candidate_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    match node.kind() {
        "type_definition" | "function_definition" => first_identifier(node, source),
        "declaration" if contains_kind(node, "function_declarator") => {
            first_identifier(node, source)
        }
        _ => Ok(None),
    }
}

fn c_candidate_signature(node: Node<'_>, source: &str) -> Result<Option<String>> {
    match node.kind() {
        "function_definition" => Ok(Some(crate::semantic::c_function_header(node, source)?)),
        "declaration" if contains_kind(node, "function_declarator") => {
            Ok(Some(node_text(node, source)?.trim().to_string()))
        }
        "type_definition" => Ok(Some(node_text(node, source)?.trim().to_string())),
        _ => Ok(None),
    }
}

fn c_candidate_node_rank(node_kind: &str) -> usize {
    match node_kind {
        "function_definition" => 30,
        "type_definition" => 20,
        "declaration" => 10,
        _ => 0,
    }
}

fn c_binding_candidates_for_name(
    accessible_symbols: &[CAccessibleSymbol],
    name: &str,
) -> Vec<CAccessibleSymbol> {
    let mut candidates = accessible_symbols
        .iter()
        .filter(|symbol| symbol.name == name)
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| left.summary.symbol_id.cmp(&right.summary.symbol_id))
    });

    let Some(best_rank) = candidates.first().map(|candidate| candidate.rank) else {
        return Vec::new();
    };

    candidates
        .into_iter()
        .filter(|candidate| candidate.rank == best_rank)
        .collect()
}

fn collect_c_local_definitions(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if let Some(parent) = candidate.parent()
            && candidate.kind() == "identifier"
            && matches!(
                parent.kind(),
                "declaration"
                    | "init_declarator"
                    | "parameter_declaration"
                    | "function_declarator"
                    | "pointer_declarator"
                    | "array_declarator"
            )
        {
            let _ = node_text(candidate, source).map(|text| names.insert(text.trim().to_string()));
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

pub(crate) fn collect_c_references(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() == "identifier" {
            let _ =
                node_text(candidate, source).map(|text| references.insert(text.trim().to_string()));
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}
