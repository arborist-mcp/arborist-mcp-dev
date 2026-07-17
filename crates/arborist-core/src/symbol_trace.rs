use std::collections::{BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

use crate::model::{
    SymbolMeta, TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodEdge,
    TraceSymbolNeighborhoodNode, TraceSymbolNeighborhoodResult,
};
use crate::symbol_map::resolved_symbol_map;
use crate::symbol_summary::{summarize_symbols, symbol_summary_from_meta, trace_evidence_keys};

pub const MAX_TRACE_TIMEOUT_MS: u64 = 5 * 60 * 1_000;

#[derive(Debug, Clone, Copy)]
pub(crate) struct TraceQueryDeadline {
    deadline: Option<Instant>,
    timeout_ms: Option<u64>,
}

impl TraceQueryDeadline {
    pub(crate) fn new(timeout_ms: Option<u64>) -> Result<Self> {
        if timeout_ms == Some(0) {
            return Err(anyhow!(
                "invalid trace timeout_ms: value must be greater than zero"
            ));
        }
        if timeout_ms.is_some_and(|value| value > MAX_TRACE_TIMEOUT_MS) {
            return Err(anyhow!(
                "invalid trace timeout_ms: value must not exceed {}",
                MAX_TRACE_TIMEOUT_MS
            ));
        }

        Ok(Self {
            deadline: timeout_ms.map(|value| Instant::now() + Duration::from_millis(value)),
            timeout_ms,
        })
    }

    pub(crate) fn check(&self, phase: &str) -> Result<()> {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(anyhow!(
                "trace timeout exceeded during {phase}: timeout_ms={}",
                self.timeout_ms.unwrap_or_default()
            ));
        }
        Ok(())
    }
}

pub(crate) fn trace_from_symbol(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_from_symbol_with_timeout(resolved_symbols, indexed_files, symbol, direction, None)
}

pub(crate) fn trace_from_symbol_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("starting graph expansion")?;
    let symbol = symbol.clone().with_origin_type("trace_root");

    let callers = if matches!(direction, TraceDirection::Callers | TraceDirection::Both) {
        deadline.check("expanding callers")?;
        summarize_symbols(resolved_symbols, &symbol.references, None)
    } else {
        Vec::new()
    };

    let callees = if matches!(direction, TraceDirection::Callees | TraceDirection::Both) {
        deadline.check("expanding callees")?;
        summarize_symbols(
            resolved_symbols,
            &symbol.dependencies,
            Some(&symbol.file_path),
        )
    } else {
        Vec::new()
    };
    deadline.check("validating graph output")?;

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

#[allow(dead_code)]
pub(crate) fn trace_neighborhood_from_symbol(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
) -> Result<TraceSymbolNeighborhoodResult> {
    trace_neighborhood_from_symbol_with_timeout(
        resolved_symbols,
        indexed_files,
        symbol,
        direction,
        max_depth,
        max_nodes,
        None,
    )
}

pub(crate) fn trace_neighborhood_from_symbol_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    max_depth: usize,
    max_nodes: usize,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolNeighborhoodResult> {
    if max_nodes == 0 {
        return Err(anyhow!("max_nodes must be greater than zero"));
    }
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("starting neighborhood expansion")?;

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
        deadline.check("expanding neighborhood")?;
        if depth >= max_depth {
            continue;
        }

        let Some(current) = resolved_map.get(&symbol_id) else {
            continue;
        };

        for (from_symbol_id, to_symbol_id) in neighborhood_edges_for_symbol(current, &direction) {
            deadline.check("expanding neighborhood edges")?;
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
    deadline.check("validating neighborhood output")?;
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{MAX_TRACE_TIMEOUT_MS, TraceQueryDeadline};

    #[test]
    fn validates_trace_timeout_bounds() {
        assert!(TraceQueryDeadline::new(Some(0)).is_err());
        assert!(TraceQueryDeadline::new(Some(MAX_TRACE_TIMEOUT_MS + 1)).is_err());
        assert!(TraceQueryDeadline::new(Some(1)).is_ok());
    }

    #[test]
    fn reports_expired_trace_deadline() {
        let deadline = TraceQueryDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = deadline
            .check("test phase")
            .expect_err("expired trace deadline should fail");
        assert!(error.to_string().contains("trace timeout exceeded"));
        assert!(error.to_string().contains("timeout_ms=1"));
    }
}
