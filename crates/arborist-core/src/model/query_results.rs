use std::collections::BTreeSet;

use anyhow::{Result, bail};

use super::{TraceSymbolNeighborhoodResult, ensure_unique_symbol_evidence_keys};

mod index_results;
mod list_search_results;
mod read_results;
mod virtual_results;

pub use index_results::{RegisteredSymbolIndex, SymbolIndexHealth, SymbolIndexStats};
pub use list_search_results::{
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
    SymbolSearchMatchDetail, SymbolSearchNeighborhoodContextResult, SymbolSearchResult,
};
pub use read_results::{
    SymbolContextResult, SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult,
    SymbolReadResult,
};
pub use virtual_results::{VirtualEditResult, VirtualFileSnapshot, VirtualFileStatus};

impl TraceSymbolNeighborhoodResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.symbol
            .validate_trace_replay_input("trace_neighborhood.symbol")?;
        if self.symbol.origin_type != "trace_root" {
            bail!(
                "invalid trace_neighborhood.symbol.origin_type: expected traced root symbol origin type to be `trace_root`"
            );
        }
        if self.max_nodes == 0 {
            bail!(
                "invalid trace_neighborhood.max_nodes: expected max_nodes to be greater than zero"
            );
        }
        if self.nodes.is_empty() {
            bail!("invalid trace_neighborhood.nodes: expected at least the root node");
        }

        let root_node = &self.nodes[0];
        root_node.validate_public_output(0)?;
        if root_node.depth != 0 {
            bail!(
                "invalid trace_neighborhood.nodes[0].depth: expected the root node to have depth 0"
            );
        }
        if root_node.symbol.symbol_id != self.symbol.symbol_id {
            bail!(
                "invalid trace_neighborhood.nodes[0].symbol.symbol_id: expected the root node to match trace_neighborhood.symbol"
            );
        }
        if root_node.symbol.semantic_path != self.symbol.semantic_path {
            bail!(
                "invalid trace_neighborhood.nodes[0].symbol.semantic_path: expected the root node to match trace_neighborhood.symbol"
            );
        }
        if root_node.symbol.file_path != self.symbol.file_path {
            bail!(
                "invalid trace_neighborhood.nodes[0].symbol.file_path: expected the root node to match trace_neighborhood.symbol"
            );
        }
        if root_node.symbol.node_kind != self.symbol.node_kind {
            bail!(
                "invalid trace_neighborhood.nodes[0].symbol.node_kind: expected the root node to match trace_neighborhood.symbol"
            );
        }
        if root_node.symbol.byte_range != self.symbol.byte_range {
            bail!(
                "invalid trace_neighborhood.nodes[0].symbol.byte_range: expected the root node to match trace_neighborhood.symbol"
            );
        }

        let mut node_ids = BTreeSet::new();
        let mut previous_depth = 0;
        for (index, node) in self.nodes.iter().enumerate() {
            node.validate_public_output(index)?;
            if node.depth > self.max_depth {
                bail!(
                    "invalid trace_neighborhood.nodes[{index}].depth: expected node depth to be at most trace_neighborhood.max_depth"
                );
            }
            if index > 0 && node.depth < previous_depth {
                bail!(
                    "invalid trace_neighborhood.nodes[{index}].depth: expected nodes to be ordered by nondecreasing depth"
                );
            }
            previous_depth = node.depth;
            if !node_ids.insert(node.symbol.symbol_id.clone()) {
                bail!(
                    "invalid trace_neighborhood.nodes[{index}].symbol.symbol_id: duplicate symbol ids are not allowed"
                );
            }
        }

        let node_summaries = self
            .nodes
            .iter()
            .map(|node| node.symbol.clone())
            .collect::<Vec<_>>();
        ensure_unique_symbol_evidence_keys(&node_summaries, "trace_neighborhood.nodes")?;

        let mut seen_edges = BTreeSet::new();
        for (index, edge) in self.edges.iter().enumerate() {
            edge.validate_public_output(index)?;
            if !node_ids.contains(&edge.from_symbol_id) {
                bail!(
                    "invalid trace_neighborhood.edges[{index}].from_symbol_id: expected edge endpoints to appear in trace_neighborhood.nodes"
                );
            }
            if !node_ids.contains(&edge.to_symbol_id) {
                bail!(
                    "invalid trace_neighborhood.edges[{index}].to_symbol_id: expected edge endpoints to appear in trace_neighborhood.nodes"
                );
            }
            if !seen_edges.insert((edge.from_symbol_id.clone(), edge.to_symbol_id.clone())) {
                bail!("invalid trace_neighborhood.edges[{index}]: duplicate edges are not allowed");
            }
        }

        Ok(())
    }
}
