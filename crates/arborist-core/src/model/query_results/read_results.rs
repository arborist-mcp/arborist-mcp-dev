use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::super::{
    Position, SymbolSummary, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolReadResult {
    pub indexed_files: usize,
    pub symbol: SymbolSummary,
    pub source: String,
    pub start_point: Position,
    pub end_point: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolContextResult {
    pub read: SymbolReadResult,
    pub trace: TraceSymbolGraphResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolNeighborhoodContextResult {
    pub neighborhood: TraceSymbolNeighborhoodResult,
    pub reads: Vec<SymbolReadResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolReadDiscoveryContextResult {
    pub read: SymbolReadResult,
    pub trace: TraceSymbolGraphResult,
    pub neighborhood_context: SymbolNeighborhoodContextResult,
}

impl SymbolReadResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        if self.source.is_empty() {
            bail!("invalid symbol_read.source: expected source to be non-empty");
        }
        self.symbol
            .validate_trace_replay_input("symbol_read.symbol")?;
        if self.start_point.row > self.end_point.row
            || (self.start_point.row == self.end_point.row
                && self.start_point.column > self.end_point.column)
        {
            bail!("invalid symbol_read: expected start_point to be before end_point");
        }
        Ok(())
    }
}

impl SymbolContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.read.validate_public_output()?;
        self.trace.validate_public_output()?;

        if self.read.indexed_files != self.trace.indexed_files {
            bail!(
                "invalid symbol_context: expected read.indexed_files to match trace.indexed_files"
            );
        }
        if self.read.symbol.symbol_id != self.trace.symbol.symbol_id {
            bail!(
                "invalid symbol_context: expected read.symbol.symbol_id to match trace.symbol.symbol_id"
            );
        }
        if self.read.symbol.semantic_path != self.trace.symbol.semantic_path {
            bail!(
                "invalid symbol_context: expected read.symbol.semantic_path to match trace.symbol.semantic_path"
            );
        }
        if self.read.symbol.file_path != self.trace.symbol.file_path {
            bail!(
                "invalid symbol_context: expected read.symbol.file_path to match trace.symbol.file_path"
            );
        }
        if self.read.symbol.node_kind != self.trace.symbol.node_kind {
            bail!(
                "invalid symbol_context: expected read.symbol.node_kind to match trace.symbol.node_kind"
            );
        }
        if self.read.symbol.byte_range != self.trace.symbol.byte_range {
            bail!(
                "invalid symbol_context: expected read.symbol.byte_range to match trace.symbol.byte_range"
            );
        }
        if self.read.symbol.signature != self.trace.symbol.signature {
            bail!(
                "invalid symbol_context: expected read.symbol.signature to match trace.symbol.signature"
            );
        }

        Ok(())
    }
}

impl SymbolNeighborhoodContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.neighborhood.validate_public_output()?;
        if self.reads.len() != self.neighborhood.nodes.len() {
            bail!(
                "invalid symbol_neighborhood_context.reads: expected reads to align with neighborhood.nodes"
            );
        }

        for (index, read) in self.reads.iter().enumerate() {
            read.validate_public_output()?;
            let node = &self.neighborhood.nodes[index];
            if read.indexed_files != self.neighborhood.indexed_files {
                bail!(
                    "invalid symbol_neighborhood_context.reads[{index}].indexed_files: expected indexed_files to match neighborhood.indexed_files"
                );
            }
            if read.symbol.symbol_id != node.symbol.symbol_id {
                bail!(
                    "invalid symbol_neighborhood_context.reads[{index}].symbol.symbol_id: expected reads to align with neighborhood.nodes"
                );
            }
            if read.symbol.semantic_path != node.symbol.semantic_path {
                bail!(
                    "invalid symbol_neighborhood_context.reads[{index}].symbol.semantic_path: expected reads to align with neighborhood.nodes"
                );
            }
            if read.symbol.file_path != node.symbol.file_path {
                bail!(
                    "invalid symbol_neighborhood_context.reads[{index}].symbol.file_path: expected reads to align with neighborhood.nodes"
                );
            }
            if read.symbol.node_kind != node.symbol.node_kind {
                bail!(
                    "invalid symbol_neighborhood_context.reads[{index}].symbol.node_kind: expected reads to align with neighborhood.nodes"
                );
            }
            if read.symbol.byte_range != node.symbol.byte_range {
                bail!(
                    "invalid symbol_neighborhood_context.reads[{index}].symbol.byte_range: expected reads to align with neighborhood.nodes"
                );
            }
            if read.symbol.signature != node.symbol.signature {
                bail!(
                    "invalid symbol_neighborhood_context.reads[{index}].symbol.signature: expected reads to align with neighborhood.nodes"
                );
            }
        }

        Ok(())
    }
}

impl SymbolReadDiscoveryContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        SymbolContextResult {
            read: self.read.clone(),
            trace: self.trace.clone(),
        }
        .validate_public_output()?;
        self.neighborhood_context.validate_public_output()?;

        if self.neighborhood_context.neighborhood.indexed_files != self.trace.indexed_files {
            bail!(
                "invalid symbol_read_discovery_context.neighborhood_context.neighborhood.indexed_files: expected neighborhood indexed_files to match trace.indexed_files"
            );
        }
        if self.neighborhood_context.neighborhood.symbol.symbol_id != self.trace.symbol.symbol_id {
            bail!(
                "invalid symbol_read_discovery_context.neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root to match trace.symbol.symbol_id"
            );
        }
        if self.neighborhood_context.neighborhood.symbol.semantic_path
            != self.trace.symbol.semantic_path
        {
            bail!(
                "invalid symbol_read_discovery_context.neighborhood_context.neighborhood.symbol.semantic_path: expected neighborhood root to match trace.symbol.semantic_path"
            );
        }
        if self.neighborhood_context.neighborhood.symbol.file_path != self.trace.symbol.file_path {
            bail!(
                "invalid symbol_read_discovery_context.neighborhood_context.neighborhood.symbol.file_path: expected neighborhood root to match trace.symbol.file_path"
            );
        }

        Ok(())
    }
}
