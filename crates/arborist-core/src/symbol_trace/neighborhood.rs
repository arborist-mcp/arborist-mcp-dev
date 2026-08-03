use std::collections::{BTreeSet, VecDeque};

use anyhow::{Result, anyhow};

use crate::model::{
    SymbolMeta, TraceDirection, TraceSymbolNeighborhoodEdge, TraceSymbolNeighborhoodNode,
    TraceSymbolNeighborhoodResult,
};
use crate::symbol_map::resolved_symbol_ref_map_with_deadline;
use crate::symbol_summary::symbol_summary_from_meta;

use super::{MAX_GRAPH_DEPTH, MAX_GRAPH_NODES, TraceQueryDeadline};

pub(crate) fn trace_neighborhood_from_symbol_with_deadline(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    deadline: &TraceQueryDeadline,
) -> Result<TraceSymbolNeighborhoodResult> {
    validate_neighborhood_bounds(max_depth, max_nodes)?;
    deadline.check("starting neighborhood expansion")?;

    let root = symbol.clone().with_origin_type("trace_root");
    let resolved_map = resolved_symbol_ref_map_with_deadline(resolved_symbols, Some(deadline))?;

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
        deadline.check("expanding neighborhood")?;
        if depth >= max_depth {
            continue;
        }

        let Some(current) = resolved_map.get(symbol_id.as_str()) else {
            continue;
        };

        for (from_symbol_id, to_symbol_id) in neighborhood_edges_for_symbol(current, &direction) {
            deadline.check("expanding neighborhood edges")?;
            let next_symbol_id = if from_symbol_id == current.symbol_id {
                to_symbol_id
            } else {
                from_symbol_id
            };

            let Some(next_symbol) = resolved_map.get(next_symbol_id) else {
                continue;
            };

            if !queued.contains(next_symbol_id) {
                if nodes.len() >= max_nodes {
                    truncated = true;
                    continue;
                }

                queued.insert(next_symbol_id.to_owned());
                queue.push_back((next_symbol_id.to_owned(), depth + 1));
                nodes.push(TraceSymbolNeighborhoodNode {
                    symbol: symbol_summary_from_meta(next_symbol),
                    depth: depth + 1,
                });
            }

            let edge_key = (from_symbol_id.to_owned(), to_symbol_id.to_owned());
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
    deadline.check("validating neighborhood output")?;
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn validate_neighborhood_bounds(max_depth: usize, max_nodes: usize) -> Result<()> {
    if max_depth > MAX_GRAPH_DEPTH {
        return Err(anyhow!("max_depth must not exceed {}", MAX_GRAPH_DEPTH));
    }
    if max_nodes == 0 {
        return Err(anyhow!("max_nodes must be greater than zero"));
    }
    if max_nodes > MAX_GRAPH_NODES {
        return Err(anyhow!("max_nodes must not exceed {}", MAX_GRAPH_NODES));
    }
    Ok(())
}

fn neighborhood_edges_for_symbol<'a>(
    symbol: &'a SymbolMeta,
    direction: &TraceDirection,
) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
    let reference_count = symbol.references.len();
    let direction = *direction;
    let symbol_id = symbol.symbol_id.as_str();
    symbol
        .references
        .iter()
        .chain(symbol.dependencies.iter())
        .enumerate()
        .filter_map(move |(index, target_id)| {
            let is_caller = index < reference_count;
            if (is_caller && matches!(direction, TraceDirection::Callees))
                || (!is_caller && matches!(direction, TraceDirection::Callers))
            {
                return None;
            }

            if is_caller {
                Some((target_id.as_str(), symbol_id))
            } else {
                Some((symbol_id, target_id.as_str()))
            }
        })
}
