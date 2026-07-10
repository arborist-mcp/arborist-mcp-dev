use std::collections::{BTreeSet, VecDeque};

use anyhow::{Result, anyhow};

use crate::model::{
    SymbolMeta, TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodEdge,
    TraceSymbolNeighborhoodNode, TraceSymbolNeighborhoodResult,
};
use crate::symbol_map::resolved_symbol_map;
use crate::symbol_summary::{summarize_symbols, symbol_summary_from_meta, trace_evidence_keys};

pub(crate) fn trace_from_symbol(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let symbol = symbol.clone().with_origin_type("trace_root");

    let callers = if matches!(direction, TraceDirection::Callers | TraceDirection::Both) {
        summarize_symbols(resolved_symbols, &symbol.references, None)
    } else {
        Vec::new()
    };

    let callees = if matches!(direction, TraceDirection::Callees | TraceDirection::Both) {
        summarize_symbols(
            resolved_symbols,
            &symbol.dependencies,
            Some(&symbol.file_path),
        )
    } else {
        Vec::new()
    };

    let result = TraceSymbolGraphResult {
        evidence_keys: trace_evidence_keys(&symbol, &callers, &callees),
        symbol,
        callers,
        callees,
        indexed_files,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn trace_neighborhood_from_symbol(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    if max_nodes == 0 {
        return Err(anyhow!("max_nodes must be greater than zero"));
    }

    let root = symbol.clone().with_origin_type("trace_root");
    let resolved_map = resolved_symbol_map(resolved_symbols);

    let mut nodes = vec![TraceSymbolNeighborhoodNode {
        symbol: symbol_summary_from_meta(&root),
        depth: 0,
    }];
    let mut edges = Vec::new();
    let mut queued = BTreeSet::from([root.symbol_id.clone()]);
    let mut edge_keys = BTreeSet::new();
    let mut queue = VecDeque::from([(root.symbol_id.clone(), 0usize)]);
    let mut truncated = false;

    while let Some((symbol_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let Some(current) = resolved_map.get(&symbol_id) else {
            continue;
        };

        for (from_symbol_id, to_symbol_id) in neighborhood_edges_for_symbol(current, &direction) {
            let next_symbol_id = if from_symbol_id == current.symbol_id {
                &to_symbol_id
            } else {
                &from_symbol_id
            };

            let Some(next_symbol) = resolved_map.get(next_symbol_id) else {
                continue;
            };

            if !queued.contains(next_symbol_id) {
                if nodes.len() >= max_nodes {
                    truncated = true;
                    continue;
                }

                queued.insert(next_symbol_id.clone());
                queue.push_back((next_symbol_id.clone(), depth + 1));
                nodes.push(TraceSymbolNeighborhoodNode {
                    symbol: symbol_summary_from_meta(next_symbol),
                    depth: depth + 1,
                });
            }

            let edge_key = (from_symbol_id.clone(), to_symbol_id.clone());
            if edge_keys.insert(edge_key.clone()) {
                edges.push(TraceSymbolNeighborhoodEdge {
                    from_symbol_id: edge_key.0,
                    to_symbol_id: edge_key.1,
                });
            }
        }
    }

    let result = TraceSymbolNeighborhoodResult {
        symbol: root,
        direction,
        max_depth,
        max_nodes,
        truncated,
        indexed_files,
        nodes,
        edges,
    };
    result.validate_public_output()?;
    Ok(result)
}

fn neighborhood_edges_for_symbol(
    symbol: &SymbolMeta,
    direction: &TraceDirection,
) -> Vec<(String, String)> {
    let mut edges = Vec::new();

    if matches!(direction, TraceDirection::Callers | TraceDirection::Both) {
        edges.extend(
            symbol
                .references
                .iter()
                .cloned()
                .map(|caller_id| (caller_id, symbol.symbol_id.clone())),
        );
    }
    if matches!(direction, TraceDirection::Callees | TraceDirection::Both) {
        edges.extend(
            symbol
                .dependencies
                .iter()
                .cloned()
                .map(|callee_id| (symbol.symbol_id.clone(), callee_id)),
        );
    }

    edges
}
