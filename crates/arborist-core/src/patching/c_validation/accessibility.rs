use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{
    ParsedDocument, c_companion_source_path, c_include_targets, first_identifier, node_text,
    normalize_path, parse_document, read_source, resolve_local_c_include,
};
use crate::model::{SymbolSummary, SymbolSummaryInit};
use crate::semantic::{
    c_is_callable_declaration, c_named_declaration_name, c_parameters, c_return_type,
    c_semantic_path, c_symbol_id_for_node, c_symbol_nodes, has_c_internal_linkage,
    semantic_parent_path,
};

#[derive(Debug, Clone)]
pub(super) struct CAccessibleSymbol {
    name: String,
    pub(super) summary: SymbolSummary,
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

fn collect_c_top_level_names(
    root: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    for child in c_symbol_nodes(root) {
        match child.kind() {
            "type_definition" | "function_definition" => {
                if let Some(name) = first_identifier(child, source)? {
                    names.insert(name);
                }
            }
            "alias_declaration" | "concept_definition" => {
                if let Some(name) = c_named_declaration_name(child, source)? {
                    names.insert(name);
                }
            }
            "declaration" | "field_declaration" => {
                if let Some(name) = first_identifier(child, source)? {
                    names.insert(name);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn collect_c_accessible_names(
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

pub(super) fn collect_c_accessible_symbols(
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

    let mut deduped = BTreeMap::new();
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
    for child in c_symbol_nodes(root) {
        let Some(name) = c_candidate_name(child, source)? else {
            continue;
        };
        let Some(semantic_path) = c_semantic_path(path, child, source)? else {
            continue;
        };
        if normalized_path != context_file && has_c_internal_linkage(child, source) {
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
        "alias_declaration" | "concept_definition" => c_named_declaration_name(node, source),
        "declaration" | "field_declaration" if c_is_callable_declaration(node) => {
            first_identifier(node, source)
        }
        _ => Ok(None),
    }
}

fn c_candidate_signature(node: Node<'_>, source: &str) -> Result<Option<String>> {
    match node.kind() {
        "function_definition" => Ok(Some(crate::semantic::c_function_header(node, source)?)),
        "declaration" | "field_declaration" if c_is_callable_declaration(node) => {
            Ok(Some(node_text(node, source)?.trim().to_string()))
        }
        "alias_declaration" | "concept_definition" | "type_definition" => {
            Ok(Some(node_text(node, source)?.trim().to_string()))
        }
        _ => Ok(None),
    }
}

fn c_candidate_node_rank(node_kind: &str) -> usize {
    match node_kind {
        "function_definition" => 30,
        "alias_declaration" | "concept_definition" | "type_definition" => 20,
        "declaration" | "field_declaration" => 10,
        _ => 0,
    }
}

pub(super) fn c_binding_candidates_for_name(
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
