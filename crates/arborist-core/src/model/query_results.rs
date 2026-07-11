use std::collections::BTreeSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::{
    PatchCommitGateReport, PatchValidationReport, Position,
    SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION, SymbolSummary, TraceSymbolGraphResult,
    TraceSymbolNeighborhoodResult, ensure_nonblank, ensure_nonblank_strings, ensure_unique_strings,
    ensure_unique_symbol_evidence_keys,
};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolIndexStats {
    pub db_path: String,
    pub indexed_files: usize,
    pub indexed_symbols: usize,
    pub rebuilt_files: usize,
    pub reused_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VirtualFileSnapshot {
    pub file: String,
    pub source: String,
    pub disk_source: String,
    pub dirty: bool,
    pub version: u64,
    pub syntax_error_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VirtualEditResult {
    pub file: String,
    pub source: String,
    pub dirty: bool,
    pub version: u64,
    pub incremental_parse: bool,
    pub validation: PatchValidationReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RegisteredSymbolIndex {
    pub workspace_root: String,
    pub db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolIndexHealth {
    pub response_schema_version: String,
    pub db_path: String,
    pub exists: bool,
    pub ok: bool,
    pub schema_version: Option<String>,
    pub expected_schema_version: String,
    pub workspace_root: Option<String>,
    pub indexed_files: Option<usize>,
    pub indexed_symbols: Option<usize>,
    pub file_state_entries: Option<usize>,
    pub fresh_file_count: Option<usize>,
    pub stale_files: Vec<String>,
    pub missing_files: Vec<String>,
    pub unreadable_files: Vec<String>,
    pub issues: Vec<String>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolListResult {
    pub indexed_files: usize,
    pub total_symbols: usize,
    pub truncated: bool,
    pub symbols: Vec<SymbolSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolListContextResult {
    pub list: SymbolListResult,
    pub reads: Vec<SymbolReadResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolListNeighborhoodContextResult {
    pub list: SymbolListResult,
    pub contexts: Vec<SymbolNeighborhoodContextResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolListDiscoveryContextResult {
    pub list: SymbolListResult,
    pub reads: Vec<SymbolReadResult>,
    pub contexts: Vec<SymbolNeighborhoodContextResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSearchResult {
    pub query: String,
    pub indexed_files: usize,
    pub total_matches: usize,
    pub truncated: bool,
    pub matches: Vec<SymbolSummary>,
    pub match_details: Vec<SymbolSearchMatchDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSearchContextResult {
    pub search: SymbolSearchResult,
    pub reads: Vec<SymbolReadResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSearchNeighborhoodContextResult {
    pub search: SymbolSearchResult,
    pub contexts: Vec<SymbolNeighborhoodContextResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSearchDiscoveryContextResult {
    pub search: SymbolSearchResult,
    pub reads: Vec<SymbolReadResult>,
    pub contexts: Vec<SymbolNeighborhoodContextResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSearchMatchDetail {
    pub symbol_id: String,
    pub score: usize,
    pub matched_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VirtualFileStatus {
    pub file: String,
    pub dirty: bool,
    pub version: u64,
    pub syntax_error_count: usize,
}

impl SymbolIndexStats {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.db_path, "symbol_index.db_path")?;
        if self.rebuilt_files + self.reused_files != self.indexed_files {
            bail!(
                "invalid symbol_index.indexed_files: expected indexed_files to equal rebuilt_files + reused_files"
            );
        }
        Ok(())
    }
}

impl RegisteredSymbolIndex {
    pub(crate) fn validate_public_output(&self, index: usize) -> Result<()> {
        let prefix = format!("registered_symbol_indexes[{index}]");
        ensure_nonblank(&self.workspace_root, &format!("{prefix}.workspace_root"))?;
        ensure_nonblank(&self.db_path, &format!("{prefix}.db_path"))?;
        Ok(())
    }
}

impl SymbolIndexHealth {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        if self.response_schema_version != SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION {
            bail!(
                "invalid symbol_index_health.response_schema_version: expected response schema version {}",
                SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION
            );
        }
        ensure_nonblank(&self.db_path, "symbol_index_health.db_path")?;
        ensure_nonblank(
            &self.expected_schema_version,
            "symbol_index_health.expected_schema_version",
        )?;
        if self.ok && !self.issues.is_empty() {
            bail!("invalid symbol_index_health.ok: expected healthy indexes to have no issues");
        }
        if !self.ok && self.issues.is_empty() {
            bail!(
                "invalid symbol_index_health.issues: expected unhealthy indexes to report issues"
            );
        }
        if !self.exists
            && (self.schema_version.is_some()
                || self.workspace_root.is_some()
                || self.indexed_files.is_some()
                || self.indexed_symbols.is_some()
                || self.file_state_entries.is_some()
                || self.fresh_file_count.is_some()
                || !self.stale_files.is_empty()
                || !self.missing_files.is_empty()
                || !self.unreadable_files.is_empty())
        {
            bail!("invalid symbol_index_health: missing indexes must not report loaded metadata");
        }
        if let Some(fresh_file_count) = self.fresh_file_count {
            let Some(file_state_entries) = self.file_state_entries else {
                bail!(
                    "invalid symbol_index_health.fresh_file_count: expected file_state_entries when freshness is inspected"
                );
            };
            if fresh_file_count
                + self.stale_files.len()
                + self.missing_files.len()
                + self.unreadable_files.len()
                != file_state_entries
            {
                bail!(
                    "invalid symbol_index_health freshness counts: expected fresh, stale, missing, and unreadable files to equal file_state_entries"
                );
            }
        }
        for (index, file_path) in self.stale_files.iter().enumerate() {
            ensure_nonblank(
                file_path,
                &format!("symbol_index_health.stale_files[{index}]"),
            )?;
        }
        for (index, file_path) in self.missing_files.iter().enumerate() {
            ensure_nonblank(
                file_path,
                &format!("symbol_index_health.missing_files[{index}]"),
            )?;
        }
        for (index, file_path) in self.unreadable_files.iter().enumerate() {
            ensure_nonblank(
                file_path,
                &format!("symbol_index_health.unreadable_files[{index}]"),
            )?;
        }
        for (index, issue) in self.issues.iter().enumerate() {
            ensure_nonblank(issue, &format!("symbol_index_health.issues[{index}]"))?;
        }
        Ok(())
    }
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

impl SymbolListResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        if self.total_symbols < self.symbols.len() {
            bail!(
                "invalid symbol_list.total_symbols: expected total_symbols to be at least symbols.len()"
            );
        }
        if self.truncated != (self.total_symbols > self.symbols.len()) {
            bail!(
                "invalid symbol_list.truncated: expected truncated to match whether total_symbols exceeds symbols.len()"
            );
        }
        for (index, item) in self.symbols.iter().enumerate() {
            item.validate_trace_replay_input(&format!("symbol_list.symbols[{index}]"))?;
        }
        ensure_unique_symbol_evidence_keys(&self.symbols, "symbol_list.symbols")
    }
}

impl SymbolSearchResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.query, "symbol_search.query")?;
        if self.total_matches < self.matches.len() {
            bail!(
                "invalid symbol_search.total_matches: expected total_matches to be at least matches.len()"
            );
        }
        if self.truncated != (self.total_matches > self.matches.len()) {
            bail!(
                "invalid symbol_search.truncated: expected truncated to match whether total_matches exceeds matches.len()"
            );
        }
        if self.matches.len() != self.match_details.len() {
            bail!(
                "invalid symbol_search.match_details: expected match_details to align with matches"
            );
        }
        for (index, item) in self.matches.iter().enumerate() {
            item.validate_trace_replay_input(&format!("symbol_search.matches[{index}]"))?;
            self.match_details[index].validate_public_output(index, &item.symbol_id)?;
        }
        ensure_unique_symbol_evidence_keys(&self.matches, "symbol_search.matches")
    }
}

impl SymbolSearchContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.search.validate_public_output()?;
        if self.reads.len() != self.search.matches.len() {
            bail!(
                "invalid symbol_search_context.reads: expected reads to align with search.matches"
            );
        }

        for (index, read) in self.reads.iter().enumerate() {
            read.validate_public_output()?;
            let symbol = &self.search.matches[index];
            if read.indexed_files != self.search.indexed_files {
                bail!(
                    "invalid symbol_search_context.reads[{index}].indexed_files: expected indexed_files to match search.indexed_files"
                );
            }
            if read.symbol.symbol_id != symbol.symbol_id {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.symbol_id: expected reads to align with search.matches"
                );
            }
            if read.symbol.semantic_path != symbol.semantic_path {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.semantic_path: expected reads to align with search.matches"
                );
            }
            if read.symbol.file_path != symbol.file_path {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.file_path: expected reads to align with search.matches"
                );
            }
            if read.symbol.node_kind != symbol.node_kind {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.node_kind: expected reads to align with search.matches"
                );
            }
            if read.symbol.byte_range != symbol.byte_range {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.byte_range: expected reads to align with search.matches"
                );
            }
            if read.symbol.signature != symbol.signature {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.signature: expected reads to align with search.matches"
                );
            }
        }

        Ok(())
    }
}

impl SymbolSearchNeighborhoodContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.search.validate_public_output()?;
        if self.contexts.len() != self.search.matches.len() {
            bail!(
                "invalid symbol_search_neighborhood_context.contexts: expected contexts to align with search.matches"
            );
        }

        for (index, context) in self.contexts.iter().enumerate() {
            context.validate_public_output()?;
            let symbol = &self.search.matches[index];
            let root = &context.neighborhood.symbol;
            if context.neighborhood.indexed_files != self.search.indexed_files {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.indexed_files: expected indexed_files to match search.indexed_files"
                );
            }
            if root.symbol_id != symbol.symbol_id {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.symbol_id: expected contexts to align with search.matches"
                );
            }
            if root.semantic_path != symbol.semantic_path {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.semantic_path: expected contexts to align with search.matches"
                );
            }
            if root.file_path != symbol.file_path {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.file_path: expected contexts to align with search.matches"
                );
            }
            if root.node_kind != symbol.node_kind {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.node_kind: expected contexts to align with search.matches"
                );
            }
            if root.byte_range != symbol.byte_range {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.byte_range: expected contexts to align with search.matches"
                );
            }
            if root.signature != symbol.signature {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.signature: expected contexts to align with search.matches"
                );
            }
        }

        Ok(())
    }
}

impl SymbolListContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.list.validate_public_output()?;
        if self.reads.len() != self.list.symbols.len() {
            bail!("invalid symbol_list_context.reads: expected reads to align with list.symbols");
        }

        for (index, read) in self.reads.iter().enumerate() {
            read.validate_public_output()?;
            let symbol = &self.list.symbols[index];
            if read.indexed_files != self.list.indexed_files {
                bail!(
                    "invalid symbol_list_context.reads[{index}].indexed_files: expected indexed_files to match list.indexed_files"
                );
            }
            if read.symbol.symbol_id != symbol.symbol_id {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.symbol_id: expected reads to align with list.symbols"
                );
            }
            if read.symbol.semantic_path != symbol.semantic_path {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.semantic_path: expected reads to align with list.symbols"
                );
            }
            if read.symbol.file_path != symbol.file_path {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.file_path: expected reads to align with list.symbols"
                );
            }
            if read.symbol.node_kind != symbol.node_kind {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.node_kind: expected reads to align with list.symbols"
                );
            }
            if read.symbol.byte_range != symbol.byte_range {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.byte_range: expected reads to align with list.symbols"
                );
            }
            if read.symbol.signature != symbol.signature {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.signature: expected reads to align with list.symbols"
                );
            }
        }

        Ok(())
    }
}

impl SymbolListNeighborhoodContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.list.validate_public_output()?;
        if self.contexts.len() != self.list.symbols.len() {
            bail!(
                "invalid symbol_list_neighborhood_context.contexts: expected contexts to align with list.symbols"
            );
        }

        for (index, context) in self.contexts.iter().enumerate() {
            context.validate_public_output()?;
            let symbol = &self.list.symbols[index];
            let root = &context.neighborhood.symbol;
            if context.neighborhood.indexed_files != self.list.indexed_files {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.indexed_files: expected indexed_files to match list.indexed_files"
                );
            }
            if root.symbol_id != symbol.symbol_id {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.symbol_id: expected contexts to align with list.symbols"
                );
            }
            if root.semantic_path != symbol.semantic_path {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.semantic_path: expected contexts to align with list.symbols"
                );
            }
            if root.file_path != symbol.file_path {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.file_path: expected contexts to align with list.symbols"
                );
            }
            if root.node_kind != symbol.node_kind {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.node_kind: expected contexts to align with list.symbols"
                );
            }
            if root.byte_range != symbol.byte_range {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.byte_range: expected contexts to align with list.symbols"
                );
            }
            if root.signature != symbol.signature {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.signature: expected contexts to align with list.symbols"
                );
            }
        }

        Ok(())
    }
}

impl SymbolSearchDiscoveryContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        SymbolSearchContextResult {
            search: self.search.clone(),
            reads: self.reads.clone(),
        }
        .validate_public_output()?;
        SymbolSearchNeighborhoodContextResult {
            search: self.search.clone(),
            contexts: self.contexts.clone(),
        }
        .validate_public_output()?;
        Ok(())
    }
}

impl SymbolListDiscoveryContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        SymbolListContextResult {
            list: self.list.clone(),
            reads: self.reads.clone(),
        }
        .validate_public_output()?;
        SymbolListNeighborhoodContextResult {
            list: self.list.clone(),
            contexts: self.contexts.clone(),
        }
        .validate_public_output()?;
        Ok(())
    }
}

impl SymbolSearchMatchDetail {
    fn validate_public_output(&self, index: usize, expected_symbol_id: &str) -> Result<()> {
        let prefix = format!("symbol_search.match_details[{index}]");
        ensure_nonblank(&self.symbol_id, &format!("{prefix}.symbol_id"))?;
        if self.symbol_id != expected_symbol_id {
            bail!(
                "invalid {prefix}.symbol_id: expected match_details to align with matches symbol ids"
            );
        }
        if self.score == 0 {
            bail!("invalid {prefix}.score: expected score to be greater than zero");
        }
        ensure_nonblank_strings(&self.matched_fields, &format!("{prefix}.matched_fields"))?;
        ensure_unique_strings(&self.matched_fields, &format!("{prefix}.matched_fields"))?;
        for field in &self.matched_fields {
            match field.as_str() {
                "base_name" | "symbol_id" | "semantic_path" | "scope_path" | "file_path"
                | "node_kind" | "signature" | "parameters" | "return_type" | "docstring" => {}
                other => {
                    bail!("invalid {prefix}.matched_fields: unsupported field `{other}`");
                }
            }
        }
        Ok(())
    }
}

impl VirtualFileSnapshot {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.file, "virtual_snapshot.file")?;
        if self.dirty != (self.source != self.disk_source) {
            bail!(
                "invalid virtual_snapshot.dirty: expected dirty to match whether source differs from disk_source"
            );
        }
        Ok(())
    }
}

impl VirtualEditResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.file, "virtual_edit.file")?;
        for (index, issue) in self.validation.syntax_errors.iter().enumerate() {
            issue.validate_trace_replay_input(index)?;
        }
        ensure_nonblank_strings(
            &self.validation.unresolved_identifiers,
            "virtual_edit.validation.unresolved_identifiers",
        )?;
        if !self.validation.resolved_identifiers.is_empty() {
            bail!(
                "invalid virtual_edit.validation.resolved_identifiers: buffer edit results must not report resolved identifiers"
            );
        }
        if !self.validation.ambiguous_identifiers.is_empty() {
            bail!(
                "invalid virtual_edit.validation.ambiguous_identifiers: buffer edit results must not report ambiguous identifiers"
            );
        }
        if !self.validation.binding_decisions.is_empty() {
            bail!(
                "invalid virtual_edit.validation.binding_decisions: buffer edit results must not report binding decisions"
            );
        }
        if self.validation.commit_gate != PatchCommitGateReport::default() {
            bail!(
                "invalid virtual_edit.validation.commit_gate: buffer edit results must leave commit_gate at the default not_evaluated state"
            );
        }
        Ok(())
    }
}

impl VirtualFileStatus {
    pub(crate) fn validate_public_output(&self, index: usize) -> Result<()> {
        ensure_nonblank(&self.file, &format!("virtual_statuses[{index}].file"))
    }
}
