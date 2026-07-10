use std::collections::BTreeSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
    Python,
    C,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub row: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PositionEdit {
    pub start: Position,
    pub end: Position,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SemanticSkeleton {
    pub file: String,
    pub skeleton: String,
    pub available_paths: Vec<String>,
    pub available_symbols: Vec<SemanticSkeletonSymbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct SemanticSkeletonSymbol {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub node_kind: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QueryCaptureResult {
    pub capture_name: String,
    pub node_kind: String,
    pub text: String,
    pub owner_symbol_id: Option<String>,
    pub owner_semantic_path: Option<String>,
    pub owner_scope_path: Option<String>,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_point: Position,
    pub end_point: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidationIssue {
    pub kind: String,
    pub message: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_point: Position,
    pub end_point: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidationBinding {
    pub name: String,
    pub symbol: SymbolSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidationAmbiguity {
    pub name: String,
    pub candidates: Vec<SymbolSummary>,
    pub reason: String,
    pub disambiguation_context: DisambiguationContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidationBindingDecision {
    pub name: String,
    pub status: String,
    pub reason: String,
    pub selected_symbol_id: Option<String>,
    pub candidates: Vec<SymbolSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchEvidenceInvariantReport {
    pub name: String,
    pub status: String,
    pub reason: String,
    pub selected_evidence_key: Option<String>,
    pub candidate_evidence_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchCommitGateReport {
    pub status: String,
    pub allowed: bool,
    pub reason: String,
    pub bypass_reason: Option<String>,
    pub blocking_decisions: Vec<ValidationBindingDecision>,
    pub evidence_invariants: Vec<PatchEvidenceInvariantReport>,
    pub syntax_error_count: usize,
}

impl Default for PatchCommitGateReport {
    fn default() -> Self {
        Self {
            status: "not_evaluated".to_string(),
            allowed: false,
            reason: "patch commit gate has not been evaluated".to_string(),
            bypass_reason: None,
            blocking_decisions: Vec::new(),
            evidence_invariants: Vec::new(),
            syntax_error_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct DisambiguationContext {
    pub active_include_family: Option<String>,
    pub preferred_family: Option<String>,
    pub visible_include_families: Vec<String>,
    pub candidate_include_families: Vec<String>,
    pub candidate_symbol_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct PatchValidationReport {
    pub syntax_errors: Vec<ValidationIssue>,
    pub unresolved_identifiers: Vec<String>,
    pub resolved_identifiers: Vec<ValidationBinding>,
    pub ambiguous_identifiers: Vec<ValidationAmbiguity>,
    pub binding_decisions: Vec<ValidationBindingDecision>,
    pub commit_gate: PatchCommitGateReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchAstNodeResult {
    pub file: String,
    pub target_path: String,
    pub resolved_path: String,
    pub resolved_symbol_id: String,
    pub applied: bool,
    pub bypass_applied: bool,
    pub updated_source: String,
    pub validation: PatchValidationReport,
}

impl PatchAstNodeResult {
    pub fn validate_trace_replay_input(&self) -> Result<()> {
        ensure_nonblank(&self.file, "patch.file")?;
        ensure_nonblank(&self.target_path, "patch.target_path")?;
        ensure_nonblank(&self.resolved_path, "patch.resolved_path")?;
        ensure_nonblank(&self.resolved_symbol_id, "patch.resolved_symbol_id")?;
        self.validation.validate_trace_replay_input()?;
        self.validation.commit_gate.validate_trace_replay_input(
            self.applied,
            self.bypass_applied,
            self.validation.syntax_errors.len(),
        )
    }

    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.updated_source, "patch.updated_source")?;
        self.validate_trace_replay_input()
    }
}

impl SemanticSkeleton {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.file, "skeleton.file")?;
        ensure_nonblank_strings(&self.available_paths, "skeleton.available_paths")?;
        if self.available_paths.len() != self.available_symbols.len() {
            bail!(
                "invalid skeleton.available_symbols: expected available_symbols to align with skeleton.available_paths"
            );
        }

        for (index, symbol) in self.available_symbols.iter().enumerate() {
            symbol.validate_public_output(index)?;
            if self.available_paths[index] != symbol.semantic_path {
                bail!(
                    "invalid skeleton.available_paths[{index}]: expected available_paths to match skeleton.available_symbols semantic paths"
                );
            }
        }

        Ok(())
    }
}

impl SemanticSkeletonSymbol {
    fn validate_public_output(&self, index: usize) -> Result<()> {
        let prefix = format!("skeleton.available_symbols[{index}]");
        ensure_nonblank(&self.symbol_id, &format!("{prefix}.symbol_id"))?;
        ensure_nonblank(&self.semantic_path, &format!("{prefix}.semantic_path"))?;
        if let Some(scope_path) = &self.scope_path {
            ensure_nonblank(scope_path, &format!("{prefix}.scope_path"))?;
        }
        ensure_nonblank(&self.node_kind, &format!("{prefix}.node_kind"))?;
        if self.byte_range.0 > self.byte_range.1 {
            bail!("invalid {prefix}.byte_range: start byte is after end byte");
        }
        if let Some(signature) = &self.signature {
            ensure_nonblank(signature, &format!("{prefix}.signature"))?;
        }
        ensure_nonblank_strings(&self.parameters, &format!("{prefix}.parameters"))?;
        if let Some(return_type) = &self.return_type {
            ensure_nonblank(return_type, &format!("{prefix}.return_type"))?;
        }
        if let Some(docstring) = &self.docstring {
            ensure_nonblank(docstring, &format!("{prefix}.docstring"))?;
        }
        Ok(())
    }
}

impl QueryCaptureResult {
    pub(crate) fn validate_public_output(&self, index: usize) -> Result<()> {
        let prefix = format!("query.captures[{index}]");
        ensure_nonblank(&self.capture_name, &format!("{prefix}.capture_name"))?;
        ensure_nonblank(&self.node_kind, &format!("{prefix}.node_kind"))?;
        if self.start_byte > self.end_byte {
            bail!("invalid {prefix}: start byte is after end byte");
        }
        if point_is_after(&self.start_point, &self.end_point) {
            bail!("invalid {prefix}: start point is after end point");
        }

        match (&self.owner_symbol_id, &self.owner_semantic_path) {
            (Some(owner_symbol_id), Some(owner_semantic_path)) => {
                ensure_nonblank(owner_symbol_id, &format!("{prefix}.owner_symbol_id"))?;
                ensure_nonblank(
                    owner_semantic_path,
                    &format!("{prefix}.owner_semantic_path"),
                )?;
            }
            (None, None) => {}
            _ => {
                bail!(
                    "invalid {prefix}: expected owner_symbol_id and owner_semantic_path to either both be present or both be absent"
                );
            }
        }

        if let Some(owner_scope_path) = &self.owner_scope_path {
            ensure_nonblank(owner_scope_path, &format!("{prefix}.owner_scope_path"))?;
            if self.owner_semantic_path.is_none() {
                bail!(
                    "invalid {prefix}.owner_scope_path: expected owner_scope_path only when owner_semantic_path is present"
                );
            }
        }

        Ok(())
    }
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
                || self.file_state_entries.is_some())
        {
            bail!("invalid symbol_index_health: missing indexes must not report loaded metadata");
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TraceDirection {
    Callers,
    Callees,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolMeta {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub evidence_key: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
    pub dependencies: Vec<String>,
    pub references: Vec<String>,
}

pub struct SymbolMetaInit {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
    pub dependencies: Vec<String>,
    pub references: Vec<String>,
}

impl SymbolMeta {
    pub fn new(init: SymbolMetaInit) -> Self {
        let evidence_key = symbol_evidence_key(
            &init.symbol_id,
            &init.file_path,
            &init.node_kind,
            &init.origin_type,
            init.byte_range,
            init.signature.as_deref(),
        );

        Self {
            symbol_id: init.symbol_id,
            semantic_path: init.semantic_path,
            scope_path: init.scope_path,
            file_path: init.file_path,
            node_kind: init.node_kind,
            origin_type: init.origin_type,
            evidence_key,
            byte_range: init.byte_range,
            signature: init.signature,
            parameters: init.parameters,
            return_type: init.return_type,
            docstring: init.docstring,
            dependencies: init.dependencies,
            references: init.references,
        }
    }

    pub fn with_origin_type(&self, origin_type: &str) -> Self {
        Self::new(SymbolMetaInit {
            symbol_id: self.symbol_id.clone(),
            semantic_path: self.semantic_path.clone(),
            scope_path: self.scope_path.clone(),
            file_path: self.file_path.clone(),
            node_kind: self.node_kind.clone(),
            origin_type: origin_type.to_string(),
            byte_range: self.byte_range,
            signature: self.signature.clone(),
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            docstring: self.docstring.clone(),
            dependencies: self.dependencies.clone(),
            references: self.references.clone(),
        })
    }

    pub fn validate_trace_replay_input(&self, field: &str) -> Result<()> {
        validate_symbol_identity(
            SymbolIdentityRef {
                symbol_id: &self.symbol_id,
                semantic_path: &self.semantic_path,
                file_path: &self.file_path,
                node_kind: &self.node_kind,
                origin_type: &self.origin_type,
                evidence_key: &self.evidence_key,
                byte_range: self.byte_range,
                signature: self.signature.as_deref(),
            },
            field,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSummary {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub evidence_key: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
}

pub struct SymbolSummaryInit {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
}

impl SymbolSummary {
    pub fn new(init: SymbolSummaryInit) -> Self {
        let evidence_key = symbol_evidence_key(
            &init.symbol_id,
            &init.file_path,
            &init.node_kind,
            &init.origin_type,
            init.byte_range,
            init.signature.as_deref(),
        );

        Self {
            symbol_id: init.symbol_id,
            semantic_path: init.semantic_path,
            scope_path: init.scope_path,
            file_path: init.file_path,
            node_kind: init.node_kind,
            origin_type: init.origin_type,
            evidence_key,
            byte_range: init.byte_range,
            signature: init.signature,
            parameters: init.parameters,
            return_type: init.return_type,
            docstring: init.docstring,
        }
    }

    pub fn validate_trace_replay_input(&self, field: &str) -> Result<()> {
        validate_symbol_identity(
            SymbolIdentityRef {
                symbol_id: &self.symbol_id,
                semantic_path: &self.semantic_path,
                file_path: &self.file_path,
                node_kind: &self.node_kind,
                origin_type: &self.origin_type,
                evidence_key: &self.evidence_key,
                byte_range: self.byte_range,
                signature: self.signature.as_deref(),
            },
            field,
        )
    }
}

fn symbol_evidence_key(
    symbol_id: &str,
    file_path: &str,
    node_kind: &str,
    origin_type: &str,
    byte_range: (usize, usize),
    signature: Option<&str>,
) -> String {
    format!(
        "{symbol_id}|{file_path}|{node_kind}|{origin_type}|{}..{}|{}",
        byte_range.0,
        byte_range.1,
        signature.unwrap_or("")
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceEvidenceKeys {
    pub symbol: String,
    pub callers: Vec<String>,
    pub callees: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TracePatchEvidenceReplayItem {
    pub name: String,
    pub status: String,
    pub selected_evidence_key: Option<String>,
    pub matched_in_trace: bool,
    pub trace_match_scope: String,
    pub candidate_evidence_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TracePatchEvidenceReplayResult {
    pub consistent: bool,
    pub matched_items: usize,
    pub blocked_items: usize,
    pub items: Vec<TracePatchEvidenceReplayItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchTraceValidationResult {
    pub allowed: bool,
    pub status: String,
    pub reason: String,
    pub patch_gate_status: String,
    pub replay_status: String,
    pub replay: TracePatchEvidenceReplayResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceBackedPatchResult {
    pub patch: PatchAstNodeResult,
    pub trace_target: String,
    pub trace: Option<TraceSymbolGraphResult>,
    pub trace_validation: Option<PatchTraceValidationResult>,
    pub trace_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GraphBackedPatchResult {
    pub patch: PatchAstNodeResult,
    pub trace_target: String,
    pub trace: Option<TraceSymbolGraphResult>,
    pub neighborhood: Option<TraceSymbolNeighborhoodResult>,
    pub trace_validation: Option<PatchTraceValidationResult>,
    pub trace_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NeighborhoodContextPatchResult {
    pub patch: PatchAstNodeResult,
    pub trace_target: String,
    pub trace: Option<TraceSymbolGraphResult>,
    pub neighborhood_context: Option<SymbolNeighborhoodContextResult>,
    pub trace_validation: Option<PatchTraceValidationResult>,
    pub trace_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryContextPatchResult {
    pub patch: PatchAstNodeResult,
    pub trace_target: String,
    pub trace: Option<TraceSymbolGraphResult>,
    pub read: Option<SymbolReadResult>,
    pub neighborhood_context: Option<SymbolNeighborhoodContextResult>,
    pub trace_validation: Option<PatchTraceValidationResult>,
    pub trace_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceSymbolGraphResult {
    pub symbol: SymbolMeta,
    pub callers: Vec<SymbolSummary>,
    pub callees: Vec<SymbolSummary>,
    pub evidence_keys: TraceEvidenceKeys,
    pub indexed_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceSymbolNeighborhoodNode {
    pub symbol: SymbolSummary,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceSymbolNeighborhoodEdge {
    pub from_symbol_id: String,
    pub to_symbol_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceSymbolNeighborhoodResult {
    pub symbol: SymbolMeta,
    pub direction: TraceDirection,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub truncated: bool,
    pub indexed_files: usize,
    pub nodes: Vec<TraceSymbolNeighborhoodNode>,
    pub edges: Vec<TraceSymbolNeighborhoodEdge>,
}

impl PatchCommitGateReport {
    fn validate_trace_replay_input(
        &self,
        applied: bool,
        bypass_applied: bool,
        syntax_error_count_expected: usize,
    ) -> Result<()> {
        ensure_nonblank(&self.status, "patch.validation.commit_gate.status")?;
        ensure_nonblank(&self.reason, "patch.validation.commit_gate.reason")?;
        if let Some(bypass_reason) = &self.bypass_reason {
            ensure_nonblank(bypass_reason, "patch.validation.commit_gate.bypass_reason")?;
        }
        if self.syntax_error_count != syntax_error_count_expected {
            bail!(
                "invalid patch.validation.commit_gate.syntax_error_count: expected {syntax_error_count_expected} to match patch.validation.syntax_errors"
            );
        }
        for (index, decision) in self.blocking_decisions.iter().enumerate() {
            let prefix = format!("patch.validation.commit_gate.blocking_decisions[{index}]");
            decision.validate_trace_replay_input(&prefix)?;
            if decision.status == "resolved" {
                bail!("invalid {prefix}.status: blocking decisions must not be resolved");
            }
        }
        for (index, invariant) in self.evidence_invariants.iter().enumerate() {
            invariant.validate_trace_replay_input(index)?;
        }

        let has_evidence_failure = self
            .evidence_invariants
            .iter()
            .any(|invariant| invariant.status == "failed");
        let has_gate_blocker = syntax_error_count_expected > 0
            || !self.blocking_decisions.is_empty()
            || has_evidence_failure;

        match self.status.as_str() {
            "allowed" => {
                if !self.allowed {
                    bail!(
                        "invalid patch.validation.commit_gate.allowed: expected true when status is allowed"
                    );
                }
                if self.bypass_reason.is_some() {
                    bail!(
                        "invalid patch.validation.commit_gate.bypass_reason: expected no bypass reason when status is allowed"
                    );
                }
                if has_gate_blocker {
                    bail!(
                        "invalid patch.validation.commit_gate.status: allowed patches must not report syntax errors, blocking decisions, or failed evidence invariants"
                    );
                }
            }
            "allowed_with_bypass" => {
                if !self.allowed {
                    bail!(
                        "invalid patch.validation.commit_gate.allowed: expected true when status is allowed_with_bypass"
                    );
                }
                if self.bypass_reason.is_none() {
                    bail!(
                        "invalid patch.validation.commit_gate.bypass_reason: expected a bypass reason when status is allowed_with_bypass"
                    );
                }
                if !has_gate_blocker {
                    bail!(
                        "invalid patch.validation.commit_gate.status: allowed_with_bypass requires syntax errors, blocking decisions, or failed evidence invariants"
                    );
                }
            }
            "rejected" => {
                if self.allowed {
                    bail!(
                        "invalid patch.validation.commit_gate.allowed: expected false when status is rejected"
                    );
                }
                if self.bypass_reason.is_some() {
                    bail!(
                        "invalid patch.validation.commit_gate.bypass_reason: expected no bypass reason when status is rejected"
                    );
                }
                if !has_gate_blocker {
                    bail!(
                        "invalid patch.validation.commit_gate.status: rejected patches must report syntax errors, blocking decisions, or failed evidence invariants"
                    );
                }
            }
            other => {
                bail!("invalid patch.validation.commit_gate.status: unsupported status `{other}`");
            }
        }

        if applied != self.allowed {
            bail!(
                "invalid patch.applied: expected patch.applied to match patch.validation.commit_gate.allowed"
            );
        }
        if bypass_applied != (self.status == "allowed_with_bypass") {
            bail!(
                "invalid patch.bypass_applied: expected patch.bypass_applied to match patch.validation.commit_gate.status"
            );
        }
        Ok(())
    }
}

impl PatchEvidenceInvariantReport {
    fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
        let prefix = format!("patch.validation.commit_gate.evidence_invariants[{index}]");
        ensure_nonblank(&self.name, &format!("{prefix}.name"))?;
        ensure_nonblank(&self.status, &format!("{prefix}.status"))?;
        ensure_nonblank(&self.reason, &format!("{prefix}.reason"))?;
        if let Some(selected_evidence_key) = &self.selected_evidence_key {
            ensure_nonblank(
                selected_evidence_key,
                &format!("{prefix}.selected_evidence_key"),
            )?;
        }
        ensure_nonblank_strings(
            &self.candidate_evidence_keys,
            &format!("{prefix}.candidate_evidence_keys"),
        )?;
        ensure_unique_strings(
            &self.candidate_evidence_keys,
            &format!("{prefix}.candidate_evidence_keys"),
        )?;
        match self.status.as_str() {
            "passed" => {
                let selected_evidence_key =
                    self.selected_evidence_key.as_deref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "invalid {prefix}.selected_evidence_key: expected a selected evidence key when status is passed"
                        )
                    })?;
                if !self
                    .candidate_evidence_keys
                    .iter()
                    .any(|candidate| candidate == selected_evidence_key)
                {
                    bail!(
                        "invalid {prefix}.selected_evidence_key: expected passed invariant selected evidence key to appear in candidate_evidence_keys"
                    );
                }
            }
            "blocked" => {
                if self.selected_evidence_key.is_some() {
                    bail!(
                        "invalid {prefix}.selected_evidence_key: expected no selected evidence key when status is blocked"
                    );
                }
            }
            "failed" => {}
            other => {
                bail!("invalid {prefix}.status: unsupported status `{other}`");
            }
        }
        Ok(())
    }
}

impl PatchValidationReport {
    fn validate_trace_replay_input(&self) -> Result<()> {
        for (index, issue) in self.syntax_errors.iter().enumerate() {
            issue.validate_trace_replay_input(index)?;
        }
        ensure_nonblank_strings(
            &self.unresolved_identifiers,
            "patch.validation.unresolved_identifiers",
        )?;
        for (index, binding) in self.resolved_identifiers.iter().enumerate() {
            binding.validate_trace_replay_input(index)?;
        }
        for (index, ambiguity) in self.ambiguous_identifiers.iter().enumerate() {
            ambiguity.validate_trace_replay_input(index)?;
        }
        for (index, decision) in self.binding_decisions.iter().enumerate() {
            decision.validate_trace_replay_input(&format!(
                "patch.validation.binding_decisions[{index}]"
            ))?;
        }
        self.validate_binding_summary_consistency()?;
        Ok(())
    }

    fn validate_binding_summary_consistency(&self) -> Result<()> {
        let mut expected_unresolved = Vec::new();
        let mut expected_resolved = Vec::new();
        let mut expected_ambiguous = Vec::new();

        for decision in &self.binding_decisions {
            match decision.status.as_str() {
                "resolved"
                    if !expected_unresolved
                        .iter()
                        .any(|name| name == &decision.name)
                        && !expected_ambiguous.iter().any(|name| name == &decision.name)
                        && !expected_resolved.iter().any(|name| name == &decision.name) =>
                {
                    expected_resolved.push(decision.name.clone());
                }
                "ambiguous"
                    if !expected_unresolved
                        .iter()
                        .any(|name| name == &decision.name) =>
                {
                    expected_resolved.retain(|name| name != &decision.name);
                    if !expected_ambiguous.iter().any(|name| name == &decision.name) {
                        expected_ambiguous.push(decision.name.clone());
                    }
                }
                "unresolved" => {
                    expected_resolved.retain(|name| name != &decision.name);
                    expected_ambiguous.retain(|name| name != &decision.name);
                    if !expected_unresolved
                        .iter()
                        .any(|name| name == &decision.name)
                    {
                        expected_unresolved.push(decision.name.clone());
                    }
                }
                _ => {}
            }
        }

        if self.unresolved_identifiers != expected_unresolved {
            bail!(
                "invalid patch.validation.unresolved_identifiers: expected unresolved identifier summary derived from patch.validation.binding_decisions"
            );
        }

        let resolved_names = self
            .resolved_identifiers
            .iter()
            .map(|binding| binding.name.clone())
            .collect::<Vec<_>>();
        if resolved_names != expected_resolved {
            bail!(
                "invalid patch.validation.resolved_identifiers: expected resolved binding summary derived from patch.validation.binding_decisions"
            );
        }

        let ambiguous_names = self
            .ambiguous_identifiers
            .iter()
            .map(|ambiguity| ambiguity.name.clone())
            .collect::<Vec<_>>();
        if ambiguous_names != expected_ambiguous {
            bail!(
                "invalid patch.validation.ambiguous_identifiers: expected ambiguous binding summary derived from patch.validation.binding_decisions"
            );
        }

        let mut seen_resolved = BTreeSet::new();
        for (index, binding) in self.resolved_identifiers.iter().enumerate() {
            if !seen_resolved.insert(binding.name.clone()) {
                bail!(
                    "invalid patch.validation.resolved_identifiers[{index}].name: duplicate resolved binding names are not allowed"
                );
            }
            let has_match = self.binding_decisions.iter().any(|decision| {
                decision.status == "resolved"
                    && decision.name == binding.name
                    && decision.selected_symbol_id.as_deref()
                        == Some(binding.symbol.symbol_id.as_str())
                    && decision.candidates.first() == Some(&binding.symbol)
            });
            if !has_match {
                bail!(
                    "invalid patch.validation.resolved_identifiers[{index}]: expected resolved binding summary to match a resolved patch.validation.binding_decisions entry"
                );
            }
        }

        let mut seen_ambiguous = BTreeSet::new();
        for (index, ambiguity) in self.ambiguous_identifiers.iter().enumerate() {
            if !seen_ambiguous.insert(ambiguity.name.clone()) {
                bail!(
                    "invalid patch.validation.ambiguous_identifiers[{index}].name: duplicate ambiguous binding names are not allowed"
                );
            }
            let has_match = self.binding_decisions.iter().any(|decision| {
                decision.status == "ambiguous"
                    && decision.name == ambiguity.name
                    && decision.reason == ambiguity.reason
                    && decision.candidates == ambiguity.candidates
            });
            if !has_match {
                bail!(
                    "invalid patch.validation.ambiguous_identifiers[{index}]: expected ambiguous binding summary to match an ambiguous patch.validation.binding_decisions entry"
                );
            }
        }

        Ok(())
    }
}

impl ValidationBinding {
    fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
        let prefix = format!("patch.validation.resolved_identifiers[{index}]");
        ensure_nonblank(&self.name, &format!("{prefix}.name"))?;
        self.symbol
            .validate_trace_replay_input(&format!("{prefix}.symbol"))
    }
}

impl ValidationAmbiguity {
    fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
        let prefix = format!("patch.validation.ambiguous_identifiers[{index}]");
        ensure_nonblank(&self.name, &format!("{prefix}.name"))?;
        ensure_nonblank(&self.reason, &format!("{prefix}.reason"))?;
        if self.candidates.len() < 2 {
            bail!(
                "invalid {prefix}.candidates: ambiguous bindings must contain at least two candidates"
            );
        }
        for (candidate_index, candidate) in self.candidates.iter().enumerate() {
            candidate
                .validate_trace_replay_input(&format!("{prefix}.candidates[{candidate_index}]"))?;
        }
        ensure_unique_symbol_evidence_keys(&self.candidates, &format!("{prefix}.candidates"))?;
        self.disambiguation_context
            .validate_trace_replay_input(&format!("{prefix}.disambiguation_context"))
    }
}

impl ValidationBindingDecision {
    fn validate_trace_replay_input(&self, field: &str) -> Result<()> {
        ensure_nonblank(&self.name, &format!("{field}.name"))?;
        ensure_nonblank(&self.status, &format!("{field}.status"))?;
        ensure_nonblank(&self.reason, &format!("{field}.reason"))?;
        if let Some(selected_symbol_id) = &self.selected_symbol_id {
            ensure_nonblank(selected_symbol_id, &format!("{field}.selected_symbol_id"))?;
        }
        for (index, candidate) in self.candidates.iter().enumerate() {
            candidate.validate_trace_replay_input(&format!("{field}.candidates[{index}]"))?;
        }
        ensure_unique_symbol_evidence_keys(&self.candidates, &format!("{field}.candidates"))?;

        match self.status.as_str() {
            "resolved" => {
                let selected_symbol_id = self.selected_symbol_id.as_deref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "invalid {field}.selected_symbol_id: expected a selected symbol id when status is resolved"
                    )
                })?;
                if self.candidates.len() != 1 {
                    bail!(
                        "invalid {field}.candidates: resolved bindings must contain exactly one candidate"
                    );
                }
                if self.candidates[0].symbol_id != selected_symbol_id {
                    bail!(
                        "invalid {field}.selected_symbol_id: expected resolved selected symbol id to match the only candidate"
                    );
                }
            }
            "ambiguous" => {
                if self.selected_symbol_id.is_some() {
                    bail!(
                        "invalid {field}.selected_symbol_id: expected no selected symbol id when status is ambiguous"
                    );
                }
                if self.candidates.len() < 2 {
                    bail!(
                        "invalid {field}.candidates: ambiguous bindings must contain at least two candidates"
                    );
                }
            }
            "unresolved" => {
                if self.selected_symbol_id.is_some() {
                    bail!(
                        "invalid {field}.selected_symbol_id: expected no selected symbol id when status is unresolved"
                    );
                }
                if !self.candidates.is_empty() {
                    bail!(
                        "invalid {field}.candidates: unresolved bindings must not contain candidates"
                    );
                }
            }
            other => {
                bail!("invalid {field}.status: unsupported status `{other}`");
            }
        }

        Ok(())
    }
}

impl DisambiguationContext {
    fn validate_trace_replay_input(&self, field: &str) -> Result<()> {
        if let Some(active_include_family) = &self.active_include_family {
            ensure_nonblank(
                active_include_family,
                &format!("{field}.active_include_family"),
            )?;
        }
        if let Some(preferred_family) = &self.preferred_family {
            ensure_nonblank(preferred_family, &format!("{field}.preferred_family"))?;
        }
        ensure_nonblank_strings(
            &self.visible_include_families,
            &format!("{field}.visible_include_families"),
        )?;
        ensure_nonblank_strings(
            &self.candidate_include_families,
            &format!("{field}.candidate_include_families"),
        )?;
        ensure_nonblank_strings(
            &self.candidate_symbol_ids,
            &format!("{field}.candidate_symbol_ids"),
        )?;
        Ok(())
    }
}

impl ValidationIssue {
    fn validate_trace_replay_input(&self, index: usize) -> Result<()> {
        let prefix = format!("patch.validation.syntax_errors[{index}]");
        ensure_nonblank(&self.kind, &format!("{prefix}.kind"))?;
        ensure_nonblank(&self.message, &format!("{prefix}.message"))?;
        match self.kind.as_str() {
            "error" | "missing" => {}
            other => {
                bail!("invalid {prefix}.kind: unsupported syntax issue kind `{other}`");
            }
        }
        if self.start_byte > self.end_byte {
            bail!("invalid {prefix}: start byte is after end byte");
        }
        if point_is_after(&self.start_point, &self.end_point) {
            bail!("invalid {prefix}: start point is after end point");
        }
        Ok(())
    }
}

impl TraceSymbolGraphResult {
    pub fn validate_trace_replay_input(&self) -> Result<()> {
        self.symbol.validate_trace_replay_input("trace.symbol")?;
        if self.symbol.origin_type != "trace_root" {
            bail!(
                "invalid trace.symbol.origin_type: expected traced root symbol origin type to be `trace_root`"
            );
        }
        for (index, caller) in self.callers.iter().enumerate() {
            caller.validate_trace_replay_input(&format!("trace.callers[{index}]"))?;
        }
        for (index, callee) in self.callees.iter().enumerate() {
            callee.validate_trace_replay_input(&format!("trace.callees[{index}]"))?;
        }
        ensure_unique_symbol_evidence_keys(&self.callers, "trace.callers")?;
        ensure_unique_symbol_evidence_keys(&self.callees, "trace.callees")?;

        let expected_callers = self
            .callers
            .iter()
            .map(|symbol| symbol.evidence_key.clone())
            .collect::<Vec<_>>();
        let expected_callees = self
            .callees
            .iter()
            .map(|symbol| symbol.evidence_key.clone())
            .collect::<Vec<_>>();

        if self.evidence_keys.symbol != self.symbol.evidence_key {
            bail!(
                "invalid trace.evidence_keys.symbol: expected traced symbol evidence key to match trace.symbol.evidence_key"
            );
        }
        if self.evidence_keys.callers != expected_callers {
            bail!(
                "invalid trace.evidence_keys.callers: expected caller evidence keys to match trace.callers"
            );
        }
        if self.evidence_keys.callees != expected_callees {
            bail!(
                "invalid trace.evidence_keys.callees: expected callee evidence keys to match trace.callees"
            );
        }

        Ok(())
    }

    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.validate_trace_replay_input()
    }
}

impl TraceSymbolNeighborhoodNode {
    fn validate_public_output(&self, index: usize) -> Result<()> {
        self.symbol
            .validate_trace_replay_input(&format!("trace_neighborhood.nodes[{index}].symbol"))?;
        Ok(())
    }
}

impl TraceSymbolNeighborhoodEdge {
    fn validate_public_output(&self, index: usize) -> Result<()> {
        ensure_nonblank(
            &self.from_symbol_id,
            &format!("trace_neighborhood.edges[{index}].from_symbol_id"),
        )?;
        ensure_nonblank(
            &self.to_symbol_id,
            &format!("trace_neighborhood.edges[{index}].to_symbol_id"),
        )?;
        if self.from_symbol_id == self.to_symbol_id {
            bail!("invalid trace_neighborhood.edges[{index}]: self-edges are not allowed");
        }
        Ok(())
    }
}

impl TracePatchEvidenceReplayItem {
    fn validate_public_output(&self, index: usize) -> Result<()> {
        let prefix = format!("trace_replay.items[{index}]");
        ensure_nonblank(&self.name, &format!("{prefix}.name"))?;
        ensure_nonblank(&self.status, &format!("{prefix}.status"))?;
        if let Some(selected_evidence_key) = &self.selected_evidence_key {
            ensure_nonblank(
                selected_evidence_key,
                &format!("{prefix}.selected_evidence_key"),
            )?;
        }
        ensure_nonblank(
            &self.trace_match_scope,
            &format!("{prefix}.trace_match_scope"),
        )?;
        ensure_nonblank_strings(
            &self.candidate_evidence_keys,
            &format!("{prefix}.candidate_evidence_keys"),
        )?;
        ensure_unique_strings(
            &self.candidate_evidence_keys,
            &format!("{prefix}.candidate_evidence_keys"),
        )?;

        match self.trace_match_scope.as_str() {
            "callers" | "callees" | "symbol" | "patch_scope" | "none" => {}
            other => {
                bail!("invalid {prefix}.trace_match_scope: unsupported scope `{other}`");
            }
        }
        if self.matched_in_trace && self.trace_match_scope == "none" {
            bail!(
                "invalid {prefix}.trace_match_scope: expected a concrete scope when matched_in_trace is true"
            );
        }
        if !self.matched_in_trace && self.trace_match_scope != "none" {
            bail!(
                "invalid {prefix}.trace_match_scope: expected `none` when matched_in_trace is false"
            );
        }

        match self.status.as_str() {
            "matched" => {
                if !self.matched_in_trace {
                    bail!(
                        "invalid {prefix}.matched_in_trace: expected matched replay items to be matched in trace"
                    );
                }
                if self.selected_evidence_key.is_none() {
                    bail!(
                        "invalid {prefix}.selected_evidence_key: expected matched replay items to include a selected evidence key"
                    );
                }
            }
            "missing" => {
                if self.matched_in_trace {
                    bail!(
                        "invalid {prefix}.matched_in_trace: expected missing replay items not to be matched in trace"
                    );
                }
                if self.selected_evidence_key.is_none() {
                    bail!(
                        "invalid {prefix}.selected_evidence_key: expected missing replay items to include a selected evidence key"
                    );
                }
            }
            "blocked" => {
                if self.matched_in_trace {
                    bail!(
                        "invalid {prefix}.matched_in_trace: expected blocked replay items not to be matched in trace"
                    );
                }
                if self.selected_evidence_key.is_some() {
                    bail!(
                        "invalid {prefix}.selected_evidence_key: expected blocked replay items not to include a selected evidence key"
                    );
                }
            }
            "failed" => {}
            other => {
                bail!("invalid {prefix}.status: unsupported replay status `{other}`");
            }
        }

        Ok(())
    }
}

impl TracePatchEvidenceReplayResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        for (index, item) in self.items.iter().enumerate() {
            item.validate_public_output(index)?;
        }

        let expected_matched_items = self
            .items
            .iter()
            .filter(|item| item.status == "matched")
            .count();
        if self.matched_items != expected_matched_items {
            bail!(
                "invalid trace_replay.matched_items: expected matched_items to match replay item statuses"
            );
        }

        let expected_blocked_items = self
            .items
            .iter()
            .filter(|item| item.status == "blocked")
            .count();
        if self.blocked_items != expected_blocked_items {
            bail!(
                "invalid trace_replay.blocked_items: expected blocked_items to match replay item statuses"
            );
        }

        let expected_consistent = self
            .items
            .iter()
            .all(|item| matches!(item.status.as_str(), "matched" | "blocked"));
        if self.consistent != expected_consistent {
            bail!(
                "invalid trace_replay.consistent: expected consistent to match replay item statuses"
            );
        }

        Ok(())
    }
}

impl PatchTraceValidationResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.status, "trace_validation.status")?;
        ensure_nonblank(&self.reason, "trace_validation.reason")?;
        ensure_nonblank(
            &self.patch_gate_status,
            "trace_validation.patch_gate_status",
        )?;
        ensure_nonblank(&self.replay_status, "trace_validation.replay_status")?;
        self.replay.validate_public_output()?;

        let expected_replay_status = summarize_replay_status(&self.replay);
        if self.replay_status != expected_replay_status {
            bail!(
                "invalid trace_validation.replay_status: expected replay_status to match replay item statuses"
            );
        }

        match self.patch_gate_status.as_str() {
            "allowed" | "allowed_with_bypass" | "rejected" => {}
            other => {
                bail!(
                    "invalid trace_validation.patch_gate_status: unsupported patch gate status `{other}`"
                );
            }
        }

        match self.status.as_str() {
            "rejected_by_patch_gate" => {
                if self.allowed {
                    bail!(
                        "invalid trace_validation.allowed: rejected_by_patch_gate results must not be allowed"
                    );
                }
                if self.patch_gate_status != "rejected" {
                    bail!(
                        "invalid trace_validation.patch_gate_status: rejected_by_patch_gate results must report a rejected patch gate"
                    );
                }
            }
            "rejected_by_trace_replay" => {
                if self.allowed {
                    bail!(
                        "invalid trace_validation.allowed: rejected_by_trace_replay results must not be allowed"
                    );
                }
                if self.patch_gate_status == "rejected" {
                    bail!(
                        "invalid trace_validation.patch_gate_status: rejected_by_trace_replay results require the patch gate to have allowed the patch"
                    );
                }
                if !matches!(
                    self.replay_status.as_str(),
                    "missing" | "failed" | "blocked"
                ) {
                    bail!(
                        "invalid trace_validation.replay_status: rejected_by_trace_replay results require missing, failed, or blocked replay evidence"
                    );
                }
                if self.replay_status == "blocked"
                    && self.patch_gate_status == "allowed_with_bypass"
                {
                    bail!(
                        "invalid trace_validation.patch_gate_status: blocked replay evidence with an allowed_with_bypass patch gate should not be rejected by trace replay"
                    );
                }
            }
            "allowed" => {
                if !self.allowed {
                    bail!("invalid trace_validation.allowed: allowed results must be allowed");
                }
                if self.patch_gate_status != "allowed" {
                    bail!(
                        "invalid trace_validation.patch_gate_status: allowed results must report an allowed patch gate"
                    );
                }
                if self.replay_status != "matched" {
                    bail!(
                        "invalid trace_validation.replay_status: allowed results require matched replay evidence"
                    );
                }
            }
            "allowed_with_bypass" => {
                if !self.allowed {
                    bail!(
                        "invalid trace_validation.allowed: allowed_with_bypass results must be allowed"
                    );
                }
                if self.patch_gate_status != "allowed_with_bypass" {
                    bail!(
                        "invalid trace_validation.patch_gate_status: allowed_with_bypass results must report an allowed_with_bypass patch gate"
                    );
                }
                if !matches!(self.replay_status.as_str(), "matched" | "blocked") {
                    bail!(
                        "invalid trace_validation.replay_status: allowed_with_bypass results require matched or blocked replay evidence"
                    );
                }
            }
            other => {
                bail!(
                    "invalid trace_validation.status: unsupported trace validation status `{other}`"
                );
            }
        }

        Ok(())
    }
}

impl TraceBackedPatchResult {
    pub(crate) fn trace_skip_reason_for_syntax_errors() -> &'static str {
        "trace skipped because patch validation reported syntax errors"
    }

    pub(crate) fn trace_skip_reason_for_patch_gate_rejection() -> &'static str {
        "trace skipped because patch validation rejected the patch"
    }

    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                Self::trace_skip_reason_for_syntax_errors()
            } else {
                Self::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }

        Ok(())
    }
}

impl GraphBackedPatchResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.neighborhood.is_some() {
                bail!(
                    "invalid neighborhood: expected no neighborhood when the patch was not safely applied"
                );
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors()
            } else {
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let neighborhood = self.neighborhood.as_ref().ok_or_else(|| {
            anyhow::anyhow!("invalid neighborhood: expected neighborhood for applied patches")
        })?;
        neighborhood.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }
        if neighborhood.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid neighborhood.symbol.symbol_id: expected neighborhood root symbol id to match patch.resolved_symbol_id"
            );
        }
        if neighborhood.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid neighborhood.symbol.semantic_path: expected neighborhood root semantic path to match patch.resolved_path"
            );
        }
        if neighborhood.symbol.file_path != self.patch.file {
            bail!(
                "invalid neighborhood.symbol.file_path: expected neighborhood root file path to match patch.file"
            );
        }
        if neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
            bail!(
                "invalid neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
            );
        }

        Ok(())
    }
}

impl NeighborhoodContextPatchResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.neighborhood_context.is_some() {
                bail!(
                    "invalid neighborhood_context: expected no neighborhood_context when the patch was not safely applied"
                );
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors()
            } else {
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let neighborhood_context = self.neighborhood_context.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid neighborhood_context: expected neighborhood_context for applied patches"
            )
        })?;
        neighborhood_context.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }

        let neighborhood = &neighborhood_context.neighborhood;
        if neighborhood.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root symbol id to match patch.resolved_symbol_id"
            );
        }
        if neighborhood.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.semantic_path: expected neighborhood root semantic path to match patch.resolved_path"
            );
        }
        if neighborhood.symbol.file_path != self.patch.file {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.file_path: expected neighborhood root file path to match patch.file"
            );
        }
        if neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
            );
        }

        Ok(())
    }
}

impl DiscoveryContextPatchResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.read.is_some() {
                bail!("invalid read: expected no read when the patch was not safely applied");
            }
            if self.neighborhood_context.is_some() {
                bail!(
                    "invalid neighborhood_context: expected no neighborhood_context when the patch was not safely applied"
                );
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors()
            } else {
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let read = self
            .read
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid read: expected read for applied patches"))?;
        read.validate_public_output()?;
        let neighborhood_context = self.neighborhood_context.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid neighborhood_context: expected neighborhood_context for applied patches"
            )
        })?;
        neighborhood_context.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }
        if read.indexed_files != trace.indexed_files {
            bail!(
                "invalid read.indexed_files: expected read.indexed_files to match trace.indexed_files"
            );
        }
        if read.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid read.symbol.symbol_id: expected read symbol id to match patch.resolved_symbol_id"
            );
        }
        if read.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid read.symbol.semantic_path: expected read semantic path to match patch.resolved_path"
            );
        }
        if read.symbol.file_path != self.patch.file {
            bail!("invalid read.symbol.file_path: expected read file path to match patch.file");
        }
        let neighborhood = &neighborhood_context.neighborhood;
        if neighborhood.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root symbol id to match patch.resolved_symbol_id"
            );
        }
        if neighborhood.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.semantic_path: expected neighborhood root semantic path to match patch.resolved_path"
            );
        }
        if neighborhood.symbol.file_path != self.patch.file {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.file_path: expected neighborhood root file path to match patch.file"
            );
        }
        if neighborhood.indexed_files != trace.indexed_files {
            bail!(
                "invalid neighborhood_context.neighborhood.indexed_files: expected neighborhood indexed_files to match trace.indexed_files"
            );
        }
        if read.symbol.symbol_id != trace.symbol.symbol_id {
            bail!("invalid read.symbol.symbol_id: expected read symbol id to match trace root");
        }
        if neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
            );
        }

        Ok(())
    }
}

fn summarize_replay_status(replay: &TracePatchEvidenceReplayResult) -> String {
    if replay.items.iter().any(|item| item.status == "failed") {
        return "failed".to_string();
    }
    if replay.items.iter().any(|item| item.status == "missing") {
        return "missing".to_string();
    }
    if replay.items.iter().any(|item| item.status == "blocked") {
        return "blocked".to_string();
    }
    "matched".to_string()
}

fn ensure_nonblank(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("invalid {field}: value must not be blank");
    }
    Ok(())
}

fn ensure_nonblank_strings(values: &[String], field: &str) -> Result<()> {
    if let Some(index) = values.iter().position(|value| value.trim().is_empty()) {
        bail!("invalid {field}[{index}]: value must not be blank");
    }
    Ok(())
}

fn ensure_unique_strings(values: &[String], field: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !seen.insert(value.clone()) {
            bail!("invalid {field}[{index}]: duplicate values are not allowed");
        }
    }
    Ok(())
}

fn ensure_unique_symbol_evidence_keys(symbols: &[SymbolSummary], field: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (index, symbol) in symbols.iter().enumerate() {
        if !seen.insert(symbol.evidence_key.clone()) {
            bail!("invalid {field}[{index}].evidence_key: duplicate evidence keys are not allowed");
        }
    }
    Ok(())
}

fn point_is_after(start: &Position, end: &Position) -> bool {
    start.row > end.row || (start.row == end.row && start.column > end.column)
}

struct SymbolIdentityRef<'a> {
    symbol_id: &'a str,
    semantic_path: &'a str,
    file_path: &'a str,
    node_kind: &'a str,
    origin_type: &'a str,
    evidence_key: &'a str,
    byte_range: (usize, usize),
    signature: Option<&'a str>,
}

fn validate_symbol_identity(identity: SymbolIdentityRef<'_>, field: &str) -> Result<()> {
    ensure_nonblank(identity.symbol_id, &format!("{field}.symbol_id"))?;
    ensure_nonblank(identity.semantic_path, &format!("{field}.semantic_path"))?;
    ensure_nonblank(identity.file_path, &format!("{field}.file_path"))?;
    ensure_nonblank(identity.node_kind, &format!("{field}.node_kind"))?;
    ensure_nonblank(identity.origin_type, &format!("{field}.origin_type"))?;
    ensure_nonblank(identity.evidence_key, &format!("{field}.evidence_key"))?;
    if identity.byte_range.0 > identity.byte_range.1 {
        bail!("invalid {field}.byte_range: start byte is after end byte");
    }

    let expected = symbol_evidence_key(
        identity.symbol_id,
        identity.file_path,
        identity.node_kind,
        identity.origin_type,
        identity.byte_range,
        identity.signature,
    );
    if identity.evidence_key != expected {
        bail!("invalid {field}.evidence_key: expected evidence key to match symbol identity");
    }

    Ok(())
}

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
    pub db_path: String,
    pub exists: bool,
    pub ok: bool,
    pub schema_version: Option<String>,
    pub expected_schema_version: String,
    pub workspace_root: Option<String>,
    pub indexed_files: Option<usize>,
    pub indexed_symbols: Option<usize>,
    pub file_state_entries: Option<usize>,
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

#[cfg(test)]
mod tests {
    use super::{
        DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
        PatchAstNodeResult, PatchCommitGateReport, PatchTraceValidationResult,
        PatchValidationReport, Position, PositionEdit, QueryCaptureResult, RegisteredSymbolIndex,
        SemanticSkeleton, SemanticSkeletonSymbol, SymbolIndexStats, SymbolListContextResult,
        SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult, SymbolListResult,
        SymbolMeta, SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult,
        SymbolReadResult, SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
        SymbolSearchMatchDetail, SymbolSearchNeighborhoodContextResult, SymbolSearchResult,
        SymbolSummary, TraceBackedPatchResult, TraceDirection, TraceEvidenceKeys,
        TracePatchEvidenceReplayItem, TracePatchEvidenceReplayResult, TraceSymbolGraphResult,
        TraceSymbolNeighborhoodNode, TraceSymbolNeighborhoodResult, ValidationBindingDecision,
        VirtualEditResult, VirtualFileSnapshot, VirtualFileStatus,
    };

    #[test]
    fn position_rejects_unknown_fields() {
        let error = serde_json::from_str::<Position>(r#"{"row":0,"column":0,"character":0}"#)
            .expect_err("positions should reject unknown fields");

        assert!(error.to_string().contains("unknown field `character`"));
    }

    #[test]
    fn position_edit_rejects_unknown_fields() {
        let error = serde_json::from_str::<PositionEdit>(
            r#"{"start":{"row":0,"column":0},"end":{"row":0,"column":0},"new_text":"x","newText":"x"}"#,
        )
        .expect_err("position edits should reject unknown fields");

        assert!(error.to_string().contains("unknown field `newText`"));
    }

    #[test]
    fn patch_result_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<PatchAstNodeResult>(
            r#"{
                "file":"sample.py",
                "target_path":"top_level",
                "resolved_path":"top_level",
                "resolved_symbol_id":"top_level",
                "applied":true,
                "bypass_applied":false,
                "updated_source":"def top_level() -> int:\n    return 1\n",
                "validation":{
                    "syntax_errors":[],
                    "unresolved_identifiers":[],
                    "resolved_identifiers":[],
                    "ambiguous_identifiers":[],
                    "binding_decisions":[],
                    "commit_gate":{
                        "status":"allowed",
                        "allowed":true,
                        "reason":"ok",
                        "bypass_reason":null,
                        "blocking_decisions":[],
                        "evidence_invariants":[],
                        "syntax_error_count":0,
                        "unexpected":true
                    }
                }
            }"#,
        )
        .expect_err("patch results should reject unknown nested fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn patch_result_rejects_missing_nested_fields() {
        let error = serde_json::from_str::<PatchAstNodeResult>(
            r#"{
                "file":"sample.py",
                "target_path":"top_level",
                "resolved_path":"top_level",
                "resolved_symbol_id":"top_level",
                "applied":true,
                "bypass_applied":false,
                "updated_source":"def top_level() -> int:\n    return 1\n",
                "validation":{
                    "syntax_errors":[],
                    "resolved_identifiers":[],
                    "ambiguous_identifiers":[],
                    "binding_decisions":[],
                    "commit_gate":{
                        "status":"allowed",
                        "allowed":true,
                        "reason":"ok",
                        "bypass_reason":null,
                        "blocking_decisions":[],
                        "evidence_invariants":[],
                        "syntax_error_count":0
                    }
                }
            }"#,
        )
        .expect_err("patch results should reject missing nested validation fields");

        assert!(error.to_string().contains("missing field"));
    }

    #[test]
    fn trace_result_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<TraceSymbolGraphResult>(
            r#"{
                "symbol":{
                    "symbol_id":"top_level",
                    "semantic_path":"top_level",
                    "file_path":"sample.py",
                    "node_kind":"function_definition",
                    "origin_type":"trace_root",
                    "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range":[0,10],
                    "parameters":[],
                    "dependencies":[],
                    "references":[],
                    "unexpected":true
                },
                "callers":[],
                "callees":[],
                "evidence_keys":{
                    "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers":[],
                    "callees":[]
                },
                "indexed_files":1
            }"#,
        )
        .expect_err("trace results should reject unknown nested fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn symbol_search_result_rejects_blank_query() {
        let result = SymbolSearchResult {
            query: "   ".to_string(),
            indexed_files: 1,
            total_matches: 0,
            truncated: false,
            matches: Vec::new(),
            match_details: Vec::new(),
        };

        let error = result
            .validate_public_output()
            .expect_err("blank search queries should be rejected");

        assert!(error.to_string().contains("symbol_search.query"));
    }

    #[test]
    fn symbol_list_result_rejects_duplicate_evidence_keys() {
        let summary = SymbolSummary {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "sample.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|"
                .to_string(),
            byte_range: (0, 10),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
        };
        let result = SymbolListResult {
            indexed_files: 1,
            total_symbols: 2,
            truncated: false,
            symbols: vec![summary.clone(), summary],
        };

        let error = result
            .validate_public_output()
            .expect_err("duplicate evidence keys should be rejected");

        assert!(error.to_string().contains("duplicate evidence keys"));
    }

    #[test]
    fn symbol_list_result_rejects_inconsistent_truncation() {
        let result = SymbolListResult {
            indexed_files: 1,
            total_symbols: 3,
            truncated: false,
            symbols: Vec::new(),
        };

        let error = result
            .validate_public_output()
            .expect_err("inconsistent truncation should be rejected");

        assert!(error.to_string().contains("symbol_list.truncated"));
    }

    #[test]
    fn symbol_read_result_rejects_empty_source() {
        let result = SymbolReadResult {
            indexed_files: 1,
            symbol: SymbolSummary {
                symbol_id: "helper".to_string(),
                semantic_path: "helper".to_string(),
                scope_path: None,
                file_path: "sample.py".to_string(),
                node_kind: "function_definition".to_string(),
                origin_type: "workspace_symbol".to_string(),
                evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|"
                    .to_string(),
                byte_range: (0, 10),
                signature: None,
                parameters: Vec::new(),
                return_type: None,
                docstring: None,
            },
            source: String::new(),
            start_point: Position { row: 0, column: 0 },
            end_point: Position { row: 0, column: 10 },
        };

        let error = result
            .validate_public_output()
            .expect_err("empty symbol source should be rejected");

        assert!(error.to_string().contains("symbol_read.source"));
    }

    #[test]
    fn symbol_neighborhood_context_rejects_misaligned_reads() {
        let result = SymbolNeighborhoodContextResult {
            neighborhood: serde_json::from_str(
                r#"{
                    "symbol":{
                        "symbol_id":"helper",
                        "semantic_path":"helper",
                        "scope_path":null,
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"helper|sample.py|function_definition|trace_root|0..10|",
                        "byte_range":[0,10],
                        "signature":null,
                        "parameters":[],
                        "return_type":null,
                        "docstring":null,
                        "dependencies":[],
                        "references":["orchestrate"]
                    },
                    "direction":"callers",
                    "max_depth":2,
                    "max_nodes":10,
                    "truncated":false,
                    "indexed_files":2,
                    "nodes":[
                        {
                            "symbol":{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"workspace_symbol",
                                "evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",
                                "byte_range":[0,10],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            },
                            "depth":0
                        }
                    ],
                    "edges":[]
                }"#,
            )
            .expect("valid neighborhood payload should deserialize"),
            reads: vec![SymbolReadResult {
                indexed_files: 2,
                symbol: SymbolSummary {
                    symbol_id: "other".to_string(),
                    semantic_path: "other".to_string(),
                    scope_path: None,
                    file_path: "sample.py".to_string(),
                    node_kind: "function_definition".to_string(),
                    origin_type: "workspace_symbol".to_string(),
                    evidence_key:
                        "other|sample.py|function_definition|workspace_symbol|0..10|".to_string(),
                    byte_range: (0, 10),
                    signature: None,
                    parameters: Vec::new(),
                    return_type: None,
                    docstring: None,
                },
                source: "def other() -> int:\n    return 1\n".to_string(),
                start_point: Position { row: 0, column: 0 },
                end_point: Position { row: 1, column: 12 },
            }],
        };

        let error = result
            .validate_public_output()
            .expect_err("neighborhood context reads should align with neighborhood nodes");

        assert!(
            error
                .to_string()
                .contains("symbol_neighborhood_context.reads[0].symbol.symbol_id")
        );
    }

    #[test]
    fn symbol_search_result_rejects_duplicate_evidence_keys() {
        let summary = SymbolSummary {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "sample.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|"
                .to_string(),
            byte_range: (0, 10),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
        };
        let result = SymbolSearchResult {
            query: "helper".to_string(),
            indexed_files: 1,
            total_matches: 2,
            truncated: false,
            matches: vec![summary.clone(), summary],
            match_details: vec![
                SymbolSearchMatchDetail {
                    symbol_id: "helper".to_string(),
                    score: 1000,
                    matched_fields: vec!["semantic_path".to_string()],
                },
                SymbolSearchMatchDetail {
                    symbol_id: "helper".to_string(),
                    score: 1000,
                    matched_fields: vec!["semantic_path".to_string()],
                },
            ],
        };

        let error = result
            .validate_public_output()
            .expect_err("duplicate evidence keys should be rejected");

        assert!(error.to_string().contains("duplicate evidence keys"));
    }

    #[test]
    fn symbol_search_result_rejects_misaligned_match_details() {
        let summary = SymbolSummary {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "sample.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|"
                .to_string(),
            byte_range: (0, 10),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
        };
        let result = SymbolSearchResult {
            query: "helper".to_string(),
            indexed_files: 1,
            total_matches: 1,
            truncated: false,
            matches: vec![summary],
            match_details: vec![SymbolSearchMatchDetail {
                symbol_id: "other".to_string(),
                score: 1000,
                matched_fields: vec!["semantic_path".to_string()],
            }],
        };

        let error = result
            .validate_public_output()
            .expect_err("misaligned match details should be rejected");

        assert!(error.to_string().contains("match_details"));
    }

    #[test]
    fn symbol_search_context_rejects_misaligned_reads() {
        let summary = SymbolSummary {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "sample.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|"
                .to_string(),
            byte_range: (0, 10),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
        };
        let result = SymbolSearchContextResult {
            search: SymbolSearchResult {
                query: "helper".to_string(),
                indexed_files: 1,
                total_matches: 1,
                truncated: false,
                matches: vec![summary],
                match_details: vec![SymbolSearchMatchDetail {
                    symbol_id: "helper".to_string(),
                    score: 1000,
                    matched_fields: vec!["semantic_path".to_string()],
                }],
            },
            reads: vec![SymbolReadResult {
                indexed_files: 1,
                symbol: SymbolSummary {
                    symbol_id: "other".to_string(),
                    semantic_path: "other".to_string(),
                    scope_path: None,
                    file_path: "sample.py".to_string(),
                    node_kind: "function_definition".to_string(),
                    origin_type: "workspace_symbol".to_string(),
                    evidence_key: "other|sample.py|function_definition|workspace_symbol|0..10|"
                        .to_string(),
                    byte_range: (0, 10),
                    signature: None,
                    parameters: Vec::new(),
                    return_type: None,
                    docstring: None,
                },
                source: "def other() -> int:\n    return 1\n".to_string(),
                start_point: Position { row: 0, column: 0 },
                end_point: Position { row: 1, column: 12 },
            }],
        };

        let error = result
            .validate_public_output()
            .expect_err("search context reads should align with search matches");

        assert!(
            error
                .to_string()
                .contains("symbol_search_context.reads[0].symbol.symbol_id")
        );
    }

    #[test]
    fn symbol_list_context_rejects_misaligned_reads() {
        let summary = SymbolSummary {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "sample.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|"
                .to_string(),
            byte_range: (0, 10),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
        };
        let result = SymbolListContextResult {
            list: SymbolListResult {
                indexed_files: 1,
                total_symbols: 1,
                truncated: false,
                symbols: vec![summary],
            },
            reads: vec![SymbolReadResult {
                indexed_files: 1,
                symbol: SymbolSummary {
                    symbol_id: "other".to_string(),
                    semantic_path: "other".to_string(),
                    scope_path: None,
                    file_path: "sample.py".to_string(),
                    node_kind: "function_definition".to_string(),
                    origin_type: "workspace_symbol".to_string(),
                    evidence_key: "other|sample.py|function_definition|workspace_symbol|0..10|"
                        .to_string(),
                    byte_range: (0, 10),
                    signature: None,
                    parameters: Vec::new(),
                    return_type: None,
                    docstring: None,
                },
                source: "def other() -> int:\n    return 1\n".to_string(),
                start_point: Position { row: 0, column: 0 },
                end_point: Position { row: 1, column: 12 },
            }],
        };

        let error = result
            .validate_public_output()
            .expect_err("list context reads should align with listed symbols");

        assert!(
            error
                .to_string()
                .contains("symbol_list_context.reads[0].symbol.symbol_id")
        );
    }

    #[test]
    fn symbol_search_neighborhood_context_rejects_misaligned_contexts() {
        let summary = SymbolSummary {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "sample.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|"
                .to_string(),
            byte_range: (0, 10),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
        };
        let result = SymbolSearchNeighborhoodContextResult {
            search: SymbolSearchResult {
                query: "helper".to_string(),
                indexed_files: 1,
                total_matches: 1,
                truncated: false,
                matches: vec![summary],
                match_details: vec![SymbolSearchMatchDetail {
                    symbol_id: "helper".to_string(),
                    score: 1000,
                    matched_fields: vec!["semantic_path".to_string()],
                }],
            },
            contexts: vec![SymbolNeighborhoodContextResult {
                neighborhood: TraceSymbolNeighborhoodResult {
                    symbol: SymbolMeta {
                        symbol_id: "other".to_string(),
                        semantic_path: "other".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                            .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                        dependencies: Vec::new(),
                        references: Vec::new(),
                    },
                    direction: TraceDirection::Both,
                    max_depth: 2,
                    max_nodes: 8,
                    truncated: false,
                    indexed_files: 1,
                    nodes: vec![TraceSymbolNeighborhoodNode {
                        symbol: SymbolSummary {
                            symbol_id: "other".to_string(),
                            semantic_path: "other".to_string(),
                            scope_path: None,
                            file_path: "sample.py".to_string(),
                            node_kind: "function_definition".to_string(),
                            origin_type: "trace_root".to_string(),
                            evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                                .to_string(),
                            byte_range: (0, 10),
                            signature: None,
                            parameters: Vec::new(),
                            return_type: None,
                            docstring: None,
                        },
                        depth: 0,
                    }],
                    edges: Vec::new(),
                },
                reads: vec![SymbolReadResult {
                    indexed_files: 1,
                    symbol: SymbolSummary {
                        symbol_id: "other".to_string(),
                        semantic_path: "other".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                            .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                    },
                    source: "def other() -> int:\n    return 1\n".to_string(),
                    start_point: Position { row: 0, column: 0 },
                    end_point: Position { row: 1, column: 12 },
                }],
            }],
        };

        let error = result
            .validate_public_output()
            .expect_err("search neighborhood contexts should align with search matches");

        assert!(error.to_string().contains(
            "symbol_search_neighborhood_context.contexts[0].neighborhood.symbol.symbol_id"
        ));
    }

    #[test]
    fn symbol_list_neighborhood_context_rejects_misaligned_contexts() {
        let summary = SymbolSummary {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "sample.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|"
                .to_string(),
            byte_range: (0, 10),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
        };
        let result = SymbolListNeighborhoodContextResult {
            list: SymbolListResult {
                indexed_files: 1,
                total_symbols: 1,
                truncated: false,
                symbols: vec![summary],
            },
            contexts: vec![SymbolNeighborhoodContextResult {
                neighborhood: TraceSymbolNeighborhoodResult {
                    symbol: SymbolMeta {
                        symbol_id: "other".to_string(),
                        semantic_path: "other".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                            .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                        dependencies: Vec::new(),
                        references: Vec::new(),
                    },
                    direction: TraceDirection::Both,
                    max_depth: 2,
                    max_nodes: 8,
                    truncated: false,
                    indexed_files: 1,
                    nodes: vec![TraceSymbolNeighborhoodNode {
                        symbol: SymbolSummary {
                            symbol_id: "other".to_string(),
                            semantic_path: "other".to_string(),
                            scope_path: None,
                            file_path: "sample.py".to_string(),
                            node_kind: "function_definition".to_string(),
                            origin_type: "trace_root".to_string(),
                            evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                                .to_string(),
                            byte_range: (0, 10),
                            signature: None,
                            parameters: Vec::new(),
                            return_type: None,
                            docstring: None,
                        },
                        depth: 0,
                    }],
                    edges: Vec::new(),
                },
                reads: vec![SymbolReadResult {
                    indexed_files: 1,
                    symbol: SymbolSummary {
                        symbol_id: "other".to_string(),
                        semantic_path: "other".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                            .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                    },
                    source: "def other() -> int:\n    return 1\n".to_string(),
                    start_point: Position { row: 0, column: 0 },
                    end_point: Position { row: 1, column: 12 },
                }],
            }],
        };

        let error = result
            .validate_public_output()
            .expect_err("list neighborhood contexts should align with listed symbols");

        assert!(error.to_string().contains(
            "symbol_list_neighborhood_context.contexts[0].neighborhood.symbol.symbol_id"
        ));
    }

    #[test]
    fn symbol_search_discovery_context_rejects_misaligned_contexts() {
        let summary = SymbolSummary {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "sample.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|"
                .to_string(),
            byte_range: (0, 10),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
        };
        let result = SymbolSearchDiscoveryContextResult {
            search: SymbolSearchResult {
                query: "helper".to_string(),
                indexed_files: 1,
                total_matches: 1,
                truncated: false,
                matches: vec![summary.clone()],
                match_details: vec![SymbolSearchMatchDetail {
                    symbol_id: "helper".to_string(),
                    score: 1000,
                    matched_fields: vec!["semantic_path".to_string()],
                }],
            },
            reads: vec![SymbolReadResult {
                indexed_files: 1,
                symbol: summary,
                source: "def helper() -> int:\n    return 1\n".to_string(),
                start_point: Position { row: 0, column: 0 },
                end_point: Position { row: 1, column: 12 },
            }],
            contexts: vec![SymbolNeighborhoodContextResult {
                neighborhood: TraceSymbolNeighborhoodResult {
                    symbol: SymbolMeta {
                        symbol_id: "other".to_string(),
                        semantic_path: "other".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                            .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                        dependencies: Vec::new(),
                        references: Vec::new(),
                    },
                    direction: TraceDirection::Both,
                    max_depth: 2,
                    max_nodes: 8,
                    truncated: false,
                    indexed_files: 1,
                    nodes: vec![TraceSymbolNeighborhoodNode {
                        symbol: SymbolSummary {
                            symbol_id: "other".to_string(),
                            semantic_path: "other".to_string(),
                            scope_path: None,
                            file_path: "sample.py".to_string(),
                            node_kind: "function_definition".to_string(),
                            origin_type: "trace_root".to_string(),
                            evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                                .to_string(),
                            byte_range: (0, 10),
                            signature: None,
                            parameters: Vec::new(),
                            return_type: None,
                            docstring: None,
                        },
                        depth: 0,
                    }],
                    edges: Vec::new(),
                },
                reads: vec![SymbolReadResult {
                    indexed_files: 1,
                    symbol: SymbolSummary {
                        symbol_id: "other".to_string(),
                        semantic_path: "other".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                            .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                    },
                    source: "def other() -> int:\n    return 1\n".to_string(),
                    start_point: Position { row: 0, column: 0 },
                    end_point: Position { row: 1, column: 12 },
                }],
            }],
        };

        let error = result
            .validate_public_output()
            .expect_err("search discovery contexts should align with search matches");

        assert!(error.to_string().contains(
            "symbol_search_neighborhood_context.contexts[0].neighborhood.symbol.symbol_id"
        ));
    }

    #[test]
    fn symbol_read_discovery_context_rejects_misaligned_neighborhood() {
        let result = SymbolReadDiscoveryContextResult {
            read: SymbolReadResult {
                indexed_files: 1,
                symbol: SymbolSummary {
                    symbol_id: "helper".to_string(),
                    semantic_path: "helper".to_string(),
                    scope_path: None,
                    file_path: "sample.py".to_string(),
                    node_kind: "function_definition".to_string(),
                    origin_type: "workspace_symbol".to_string(),
                    evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|"
                        .to_string(),
                    byte_range: (0, 10),
                    signature: None,
                    parameters: Vec::new(),
                    return_type: None,
                    docstring: None,
                },
                source: "def helper() -> int:\n    return 1\n".to_string(),
                start_point: Position { row: 0, column: 0 },
                end_point: Position { row: 1, column: 12 },
            },
            trace: TraceSymbolGraphResult {
                symbol: SymbolMeta {
                    symbol_id: "helper".to_string(),
                    semantic_path: "helper".to_string(),
                    scope_path: None,
                    file_path: "sample.py".to_string(),
                    node_kind: "function_definition".to_string(),
                    origin_type: "trace_root".to_string(),
                    evidence_key: "helper|sample.py|function_definition|trace_root|0..10|"
                        .to_string(),
                    byte_range: (0, 10),
                    signature: None,
                    parameters: Vec::new(),
                    return_type: None,
                    docstring: None,
                    dependencies: Vec::new(),
                    references: vec!["orchestrate".to_string()],
                },
                callers: vec![SymbolSummary {
                    symbol_id: "orchestrate".to_string(),
                    semantic_path: "orchestrate".to_string(),
                    scope_path: None,
                    file_path: "caller.py".to_string(),
                    node_kind: "function_definition".to_string(),
                    origin_type: "trace_caller".to_string(),
                    evidence_key: "orchestrate|caller.py|function_definition|trace_caller|0..20|"
                        .to_string(),
                    byte_range: (0, 20),
                    signature: None,
                    parameters: Vec::new(),
                    return_type: None,
                    docstring: None,
                }],
                callees: Vec::new(),
                evidence_keys: TraceEvidenceKeys {
                    symbol: "helper|sample.py|function_definition|trace_root|0..10|".to_string(),
                    callers: vec![
                        "orchestrate|caller.py|function_definition|trace_caller|0..20|".to_string(),
                    ],
                    callees: Vec::new(),
                },
                indexed_files: 1,
            },
            neighborhood_context: SymbolNeighborhoodContextResult {
                neighborhood: TraceSymbolNeighborhoodResult {
                    symbol: SymbolMeta {
                        symbol_id: "other".to_string(),
                        semantic_path: "other".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                            .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                        dependencies: Vec::new(),
                        references: Vec::new(),
                    },
                    direction: TraceDirection::Callers,
                    max_depth: 2,
                    max_nodes: 8,
                    truncated: false,
                    indexed_files: 1,
                    nodes: vec![TraceSymbolNeighborhoodNode {
                        symbol: SymbolSummary {
                            symbol_id: "other".to_string(),
                            semantic_path: "other".to_string(),
                            scope_path: None,
                            file_path: "sample.py".to_string(),
                            node_kind: "function_definition".to_string(),
                            origin_type: "trace_root".to_string(),
                            evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                                .to_string(),
                            byte_range: (0, 10),
                            signature: None,
                            parameters: Vec::new(),
                            return_type: None,
                            docstring: None,
                        },
                        depth: 0,
                    }],
                    edges: Vec::new(),
                },
                reads: vec![SymbolReadResult {
                    indexed_files: 1,
                    symbol: SymbolSummary {
                        symbol_id: "other".to_string(),
                        semantic_path: "other".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                            .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                    },
                    source: "def other() -> int:\n    return 1\n".to_string(),
                    start_point: Position { row: 0, column: 0 },
                    end_point: Position { row: 1, column: 12 },
                }],
            },
        };

        let error = result
            .validate_public_output()
            .expect_err("read discovery context should align the neighborhood root");

        assert!(error.to_string().contains(
            "symbol_read_discovery_context.neighborhood_context.neighborhood.symbol.symbol_id"
        ));
    }

    #[test]
    fn symbol_list_discovery_context_rejects_misaligned_contexts() {
        let summary = SymbolSummary {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "sample.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|"
                .to_string(),
            byte_range: (0, 10),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
        };
        let result = SymbolListDiscoveryContextResult {
            list: SymbolListResult {
                indexed_files: 1,
                total_symbols: 1,
                truncated: false,
                symbols: vec![summary.clone()],
            },
            reads: vec![SymbolReadResult {
                indexed_files: 1,
                symbol: summary,
                source: "def helper() -> int:\n    return 1\n".to_string(),
                start_point: Position { row: 0, column: 0 },
                end_point: Position { row: 1, column: 12 },
            }],
            contexts: vec![SymbolNeighborhoodContextResult {
                neighborhood: TraceSymbolNeighborhoodResult {
                    symbol: SymbolMeta {
                        symbol_id: "other".to_string(),
                        semantic_path: "other".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                            .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                        dependencies: Vec::new(),
                        references: Vec::new(),
                    },
                    direction: TraceDirection::Both,
                    max_depth: 2,
                    max_nodes: 8,
                    truncated: false,
                    indexed_files: 1,
                    nodes: vec![TraceSymbolNeighborhoodNode {
                        symbol: SymbolSummary {
                            symbol_id: "other".to_string(),
                            semantic_path: "other".to_string(),
                            scope_path: None,
                            file_path: "sample.py".to_string(),
                            node_kind: "function_definition".to_string(),
                            origin_type: "trace_root".to_string(),
                            evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                                .to_string(),
                            byte_range: (0, 10),
                            signature: None,
                            parameters: Vec::new(),
                            return_type: None,
                            docstring: None,
                        },
                        depth: 0,
                    }],
                    edges: Vec::new(),
                },
                reads: vec![SymbolReadResult {
                    indexed_files: 1,
                    symbol: SymbolSummary {
                        symbol_id: "other".to_string(),
                        semantic_path: "other".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key: "other|sample.py|function_definition|trace_root|0..10|"
                            .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                    },
                    source: "def other() -> int:\n    return 1\n".to_string(),
                    start_point: Position { row: 0, column: 0 },
                    end_point: Position { row: 1, column: 12 },
                }],
            }],
        };

        let error = result
            .validate_public_output()
            .expect_err("list discovery contexts should align with listed symbols");

        assert!(error.to_string().contains(
            "symbol_list_neighborhood_context.contexts[0].neighborhood.symbol.symbol_id"
        ));
    }

    #[test]
    fn trace_result_rejects_missing_nested_fields() {
        let error = serde_json::from_str::<TraceSymbolGraphResult>(
            r#"{
                "symbol":{
                    "symbol_id":"top_level"
                },
                "callers":[],
                "callees":[],
                "evidence_keys":{
                    "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers":[],
                    "callees":[]
                },
                "indexed_files":1
            }"#,
        )
        .expect_err("trace results should reject missing nested symbol fields");

        assert!(error.to_string().contains("missing field"));
    }

    #[test]
    fn replay_result_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<TracePatchEvidenceReplayResult>(
            r#"{
                "consistent":true,
                "matched_items":1,
                "blocked_items":0,
                "items":[{
                    "name":"helper",
                    "status":"matched",
                    "selected_evidence_key":"helper|sample.py|function_definition|local_file|0..10|",
                    "matched_in_trace":true,
                    "trace_match_scope":"callees",
                    "candidate_evidence_keys":["helper|sample.py|function_definition|local_file|0..10|"],
                    "unexpected":true
                }]
            }"#,
        )
        .expect_err("replay results should reject unknown nested fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn trace_validation_result_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<PatchTraceValidationResult>(
            r#"{
                "allowed":true,
                "status":"allowed",
                "reason":"ok",
                "patch_gate_status":"allowed",
                "replay_status":"matched",
                "replay":{
                    "consistent":true,
                    "matched_items":1,
                    "blocked_items":0,
                    "items":[{
                        "name":"helper",
                        "status":"matched",
                        "selected_evidence_key":"helper|sample.py|function_definition|local_file|0..10|",
                        "matched_in_trace":true,
                        "trace_match_scope":"callees",
                        "candidate_evidence_keys":["helper|sample.py|function_definition|local_file|0..10|"]
                    }],
                    "unexpected":true
                }
            }"#,
        )
        .expect_err("trace validation results should reject unknown nested replay fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn trace_backed_patch_result_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<TraceBackedPatchResult>(
            r#"{
                "patch":{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":false,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return missing_helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":["missing_helper"],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"missing_helper",
                            "status":"unresolved",
                            "reason":"identifier is not visible from the patched symbol scope",
                            "selected_symbol_id":null,
                            "candidates":[]
                        }],
                        "commit_gate":{
                            "status":"rejected",
                            "allowed":false,
                            "reason":"symbol binding could not be resolved",
                            "bypass_reason":null,
                            "blocking_decisions":[{
                                "name":"missing_helper",
                                "status":"unresolved",
                                "reason":"identifier is not visible from the patched symbol scope",
                                "selected_symbol_id":null,
                                "candidates":[]
                            }],
                            "evidence_invariants":[{
                                "name":"missing_helper",
                                "status":"blocked",
                                "reason":"no candidate evidence key is available for this binding",
                                "selected_evidence_key":null,
                                "candidate_evidence_keys":[]
                            }],
                            "syntax_error_count":0
                        }
                    }
                },
                "trace_target":"top_level",
                "trace":null,
                "trace_validation":null,
                "trace_error":"trace skipped because patch validation rejected the patch",
                "unexpected":true
            }"#,
        )
        .expect_err("trace-backed patch results should reject unknown top-level fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn graph_backed_patch_result_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<GraphBackedPatchResult>(
            r#"{
                "patch":{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":false,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return missing_helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":["missing_helper"],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"missing_helper",
                            "status":"unresolved",
                            "reason":"identifier is not visible from the patched symbol scope",
                            "selected_symbol_id":null,
                            "candidates":[]
                        }],
                        "commit_gate":{
                            "status":"rejected",
                            "allowed":false,
                            "reason":"symbol binding could not be resolved",
                            "bypass_reason":null,
                            "blocking_decisions":[{
                                "name":"missing_helper",
                                "status":"unresolved",
                                "reason":"identifier is not visible from the patched symbol scope",
                                "selected_symbol_id":null,
                                "candidates":[]
                            }],
                            "evidence_invariants":[{
                                "name":"missing_helper",
                                "status":"blocked",
                                "reason":"no candidate evidence key is available for this binding",
                                "selected_evidence_key":null,
                                "candidate_evidence_keys":[]
                            }],
                            "syntax_error_count":0
                        }
                    }
                },
                "trace_target":"top_level",
                "trace":null,
                "neighborhood":null,
                "trace_validation":null,
                "trace_error":"trace skipped because patch validation rejected the patch",
                "unexpected":true
            }"#,
        )
        .expect_err("graph-backed patch results should reject unknown top-level fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn semantic_skeleton_rejects_unknown_nested_fields() {
        let error = serde_json::from_str::<SemanticSkeleton>(
            r#"{
                "file":"sample.py",
                "skeleton":"def top_level() -> int:\n    return 1\n",
                "available_paths":["top_level"],
                "available_symbols":[{
                    "symbol_id":"sample.py::top_level",
                    "semantic_path":"top_level",
                    "node_kind":"function_definition",
                    "byte_range":[0,10],
                    "parameters":[],
                    "unexpected":true
                }]
            }"#,
        )
        .expect_err("semantic skeletons should reject unknown nested symbol fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn query_capture_result_rejects_unknown_fields() {
        let error = serde_json::from_str::<QueryCaptureResult>(
            r#"{
                "capture_name":"name",
                "node_kind":"identifier",
                "text":"top_level",
                "owner_symbol_id":"sample.py::top_level",
                "owner_semantic_path":"top_level",
                "owner_scope_path":null,
                "start_byte":0,
                "end_byte":9,
                "start_point":{"row":0,"column":0},
                "end_point":{"row":0,"column":9},
                "unexpected":true
            }"#,
        )
        .expect_err("query capture results should reject unknown fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn symbol_index_stats_reject_unknown_fields() {
        let error = serde_json::from_str::<SymbolIndexStats>(
            r#"{
                "db_path":"symbols.db",
                "indexed_files":1,
                "indexed_symbols":2,
                "rebuilt_files":1,
                "reused_files":0,
                "unexpected":true
            }"#,
        )
        .expect_err("symbol index stats should reject unknown fields");

        assert!(error.to_string().contains("unknown field `unexpected`"));
    }

    #[test]
    fn virtual_results_reject_unknown_fields() {
        let snapshot_error = serde_json::from_str::<VirtualFileSnapshot>(
            r#"{
                "file":"sample.py",
                "source":"def top_level() -> int:\n    return 1\n",
                "disk_source":"def top_level() -> int:\n    return 1\n",
                "dirty":false,
                "version":1,
                "syntax_error_count":0,
                "unexpected":true
            }"#,
        )
        .expect_err("virtual file snapshots should reject unknown fields");
        assert!(
            snapshot_error
                .to_string()
                .contains("unknown field `unexpected`")
        );

        let edit_error = serde_json::from_str::<VirtualEditResult>(
            r#"{
                "file":"sample.py",
                "source":"def top_level() -> int:\n    return 1\n",
                "dirty":false,
                "version":1,
                "incremental_parse":true,
                "validation":{
                    "syntax_errors":[],
                    "unresolved_identifiers":[],
                    "resolved_identifiers":[],
                    "ambiguous_identifiers":[],
                    "binding_decisions":[],
                    "commit_gate":{
                        "status":"allowed",
                        "allowed":true,
                        "reason":"ok",
                        "bypass_reason":null,
                        "blocking_decisions":[],
                        "evidence_invariants":[],
                        "syntax_error_count":0
                    }
                },
                "unexpected":true
            }"#,
        )
        .expect_err("virtual edit results should reject unknown fields");
        assert!(
            edit_error
                .to_string()
                .contains("unknown field `unexpected`")
        );

        let registration_error = serde_json::from_str::<RegisteredSymbolIndex>(
            r#"{
                "workspace_root":"workspace",
                "db_path":"symbols.db",
                "unexpected":true
            }"#,
        )
        .expect_err("registered symbol index results should reject unknown fields");
        assert!(
            registration_error
                .to_string()
                .contains("unknown field `unexpected`")
        );

        let status_error = serde_json::from_str::<VirtualFileStatus>(
            r#"{
                "file":"sample.py",
                "dirty":false,
                "version":1,
                "syntax_error_count":0,
                "unexpected":true
            }"#,
        )
        .expect_err("virtual file status results should reject unknown fields");
        assert!(
            status_error
                .to_string()
                .contains("unknown field `unexpected`")
        );
    }

    #[test]
    fn semantic_skeleton_validation_rejects_path_symbol_mismatch() {
        let skeleton = SemanticSkeleton {
            file: "sample.py".to_string(),
            skeleton: "def top_level() -> int:\n    return 1\n".to_string(),
            available_paths: vec!["other".to_string()],
            available_symbols: vec![SemanticSkeletonSymbol {
                symbol_id: "sample.py::top_level".to_string(),
                semantic_path: "top_level".to_string(),
                scope_path: None,
                node_kind: "function_definition".to_string(),
                byte_range: (0, 10),
                signature: Some("def top_level(value: int) -> int:".to_string()),
                parameters: vec!["value: int".to_string()],
                return_type: Some("int".to_string()),
                docstring: None,
            }],
        };

        let error = skeleton
            .validate_public_output()
            .expect_err("semantic skeleton validation should reject path-symbol mismatches");

        assert!(error.to_string().contains("skeleton.available_paths[0]"));
    }

    #[test]
    fn query_capture_validation_rejects_partial_owner_fields() {
        let capture = QueryCaptureResult {
            capture_name: "name".to_string(),
            node_kind: "identifier".to_string(),
            text: "top_level".to_string(),
            owner_symbol_id: Some("top_level".to_string()),
            owner_semantic_path: None,
            owner_scope_path: None,
            start_byte: 0,
            end_byte: 9,
            start_point: Position { row: 0, column: 0 },
            end_point: Position { row: 0, column: 9 },
        };

        let error = capture
            .validate_public_output(0)
            .expect_err("query capture validation should reject partial owner fields");

        assert!(error.to_string().contains("owner_symbol_id"));
    }

    #[test]
    fn symbol_index_stats_validation_rejects_inconsistent_totals() {
        let stats = SymbolIndexStats {
            db_path: "symbols.db".to_string(),
            indexed_files: 3,
            indexed_symbols: 4,
            rebuilt_files: 1,
            reused_files: 1,
        };

        let error = stats
            .validate_public_output()
            .expect_err("symbol index stats validation should reject inconsistent totals");

        assert!(error.to_string().contains("symbol_index.indexed_files"));
    }

    #[test]
    fn virtual_snapshot_validation_rejects_dirty_state_mismatch() {
        let snapshot = VirtualFileSnapshot {
            file: "sample.py".to_string(),
            source: "def value() -> int:\n    return 2\n".to_string(),
            disk_source: "def value() -> int:\n    return 1\n".to_string(),
            dirty: false,
            version: 1,
            syntax_error_count: 0,
        };

        let error = snapshot
            .validate_public_output()
            .expect_err("virtual snapshots should reject dirty/source mismatches");

        assert!(error.to_string().contains("virtual_snapshot.dirty"));
    }

    #[test]
    fn virtual_edit_validation_rejects_non_default_commit_gate() {
        let result = VirtualEditResult {
            file: "sample.py".to_string(),
            source: "def value() -> int:\n    return 1\n".to_string(),
            dirty: false,
            version: 1,
            incremental_parse: true,
            validation: PatchValidationReport {
                syntax_errors: Vec::new(),
                unresolved_identifiers: Vec::new(),
                resolved_identifiers: Vec::new(),
                ambiguous_identifiers: Vec::new(),
                binding_decisions: Vec::new(),
                commit_gate: PatchCommitGateReport {
                    status: "allowed".to_string(),
                    allowed: true,
                    reason: "tampered".to_string(),
                    ..Default::default()
                },
            },
        };

        let error = result
            .validate_public_output()
            .expect_err("virtual edit validation should reject non-default commit gates");

        assert!(
            error
                .to_string()
                .contains("virtual_edit.validation.commit_gate")
        );
    }

    #[test]
    fn patch_result_validation_rejects_tampered_commit_gate_flags() {
        let mut patch = serde_json::from_str::<PatchAstNodeResult>(
            r#"{
                "file":"sample.py",
                "target_path":"top_level",
                "resolved_path":"top_level",
                "resolved_symbol_id":"top_level",
                "applied":true,
                "bypass_applied":false,
                "updated_source":"def top_level() -> int:\n    return 1\n",
                "validation":{
                    "syntax_errors":[],
                    "unresolved_identifiers":[],
                    "resolved_identifiers":[],
                    "ambiguous_identifiers":[],
                    "binding_decisions":[],
                    "commit_gate":{
                        "status":"allowed",
                        "allowed":true,
                        "reason":"ok",
                        "bypass_reason":null,
                        "blocking_decisions":[],
                        "evidence_invariants":[],
                        "syntax_error_count":0
                    }
                }
            }"#,
        )
        .expect("valid patch payload should deserialize");
        patch.applied = false;

        let error = patch
            .validate_public_output()
            .expect_err("patch validation should reject tampered applied flags");

        assert!(error.to_string().contains("patch.applied"));
    }

    #[test]
    fn trace_result_validation_rejects_tampered_evidence_keys() {
        let mut trace = serde_json::from_str::<TraceSymbolGraphResult>(
            r#"{
                "symbol":{
                    "symbol_id":"top_level",
                    "semantic_path":"top_level",
                    "file_path":"sample.py",
                    "node_kind":"function_definition",
                    "origin_type":"trace_root",
                    "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range":[0,10],
                    "parameters":[],
                    "dependencies":[],
                    "references":[]
                },
                "callers":[],
                "callees":[],
                "evidence_keys":{
                    "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers":[],
                    "callees":[]
                },
                "indexed_files":1
            }"#,
        )
        .expect("valid trace payload should deserialize");
        trace.evidence_keys.symbol = "tampered".to_string();

        let error = trace
            .validate_public_output()
            .expect_err("trace validation should reject tampered evidence key summaries");

        assert!(error.to_string().contains("trace.evidence_keys.symbol"));
    }

    #[test]
    fn trace_replay_validation_rejects_tampered_match_counts() {
        let replay = TracePatchEvidenceReplayResult {
            consistent: true,
            matched_items: 0,
            blocked_items: 0,
            items: vec![TracePatchEvidenceReplayItem {
                name: "helper".to_string(),
                status: "matched".to_string(),
                selected_evidence_key: Some(
                    "helper|sample.py|function_definition|callee|0..10|".to_string(),
                ),
                matched_in_trace: true,
                trace_match_scope: "callees".to_string(),
                candidate_evidence_keys: vec![
                    "helper|sample.py|function_definition|callee|0..10|".to_string(),
                ],
            }],
        };

        let error = replay
            .validate_public_output()
            .expect_err("trace replay validation should reject tampered match counts");

        assert!(error.to_string().contains("trace_replay.matched_items"));
    }

    #[test]
    fn trace_validation_rejects_tampered_replay_status() {
        let result = PatchTraceValidationResult {
            allowed: true,
            status: "allowed".to_string(),
            reason: "ok".to_string(),
            patch_gate_status: "allowed".to_string(),
            replay_status: "blocked".to_string(),
            replay: TracePatchEvidenceReplayResult {
                consistent: true,
                matched_items: 1,
                blocked_items: 0,
                items: vec![TracePatchEvidenceReplayItem {
                    name: "helper".to_string(),
                    status: "matched".to_string(),
                    selected_evidence_key: Some(
                        "helper|sample.py|function_definition|callee|0..10|".to_string(),
                    ),
                    matched_in_trace: true,
                    trace_match_scope: "callees".to_string(),
                    candidate_evidence_keys: vec![
                        "helper|sample.py|function_definition|callee|0..10|".to_string(),
                    ],
                }],
            },
        };

        let error = result
            .validate_public_output()
            .expect_err("trace validation should reject tampered replay status");

        assert!(error.to_string().contains("trace_validation.replay_status"));
    }

    #[test]
    fn trace_backed_patch_validation_rejects_trace_without_validation() {
        let result = TraceBackedPatchResult {
            patch: PatchAstNodeResult {
                file: "sample.py".to_string(),
                target_path: "top_level".to_string(),
                resolved_path: "top_level".to_string(),
                resolved_symbol_id: "top_level".to_string(),
                applied: true,
                bypass_applied: false,
                updated_source: "def top_level() -> int:\n    return 1\n".to_string(),
                validation: PatchValidationReport {
                    syntax_errors: Vec::new(),
                    unresolved_identifiers: Vec::new(),
                    resolved_identifiers: Vec::new(),
                    ambiguous_identifiers: Vec::new(),
                    binding_decisions: Vec::new(),
                    commit_gate: PatchCommitGateReport {
                        status: "allowed".to_string(),
                        allowed: true,
                        reason: "ok".to_string(),
                        bypass_reason: None,
                        blocking_decisions: Vec::new(),
                        evidence_invariants: Vec::new(),
                        syntax_error_count: 0,
                    },
                },
            },
            trace_target: "top_level".to_string(),
            trace: Some(
                serde_json::from_str(
                    r#"{
                        "symbol":{
                            "symbol_id":"top_level",
                            "semantic_path":"top_level",
                            "file_path":"sample.py",
                            "node_kind":"function_definition",
                            "origin_type":"trace_root",
                            "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                            "byte_range":[0,10],
                            "parameters":[],
                            "dependencies":[],
                            "references":[]
                        },
                        "callers":[],
                        "callees":[],
                        "evidence_keys":{
                            "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                            "callers":[],
                            "callees":[]
                        },
                        "indexed_files":1
                    }"#,
                )
                .expect("valid trace payload should deserialize"),
            ),
            trace_validation: None,
            trace_error: None,
        };

        let error = result.validate_public_output().expect_err(
            "trace-backed patch validation should require trace validation for applied patches",
        );

        assert!(error.to_string().contains("trace_validation"));
    }

    #[test]
    fn trace_backed_patch_validation_rejects_wrong_skip_reason() {
        let result = TraceBackedPatchResult {
            patch: PatchAstNodeResult {
                file: "sample.py".to_string(),
                target_path: "top_level".to_string(),
                resolved_path: "top_level".to_string(),
                resolved_symbol_id: "top_level".to_string(),
                applied: false,
                bypass_applied: false,
                updated_source: "def top_level() -> int:\n    return 1\n".to_string(),
                validation: PatchValidationReport {
                    syntax_errors: Vec::new(),
                    unresolved_identifiers: vec!["missing".to_string()],
                    resolved_identifiers: Vec::new(),
                    ambiguous_identifiers: Vec::new(),
                    binding_decisions: vec![ValidationBindingDecision {
                        name: "missing".to_string(),
                        status: "unresolved".to_string(),
                        reason: "missing binding".to_string(),
                        selected_symbol_id: None,
                        candidates: Vec::new(),
                    }],
                    commit_gate: PatchCommitGateReport {
                        status: "rejected".to_string(),
                        allowed: false,
                        reason: "missing binding".to_string(),
                        bypass_reason: None,
                        blocking_decisions: vec![ValidationBindingDecision {
                            name: "missing".to_string(),
                            status: "unresolved".to_string(),
                            reason: "missing binding".to_string(),
                            selected_symbol_id: None,
                            candidates: Vec::new(),
                        }],
                        evidence_invariants: Vec::new(),
                        syntax_error_count: 0,
                    },
                },
            },
            trace_target: "top_level".to_string(),
            trace: None,
            trace_validation: None,
            trace_error: Some(
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors().to_string(),
            ),
        };

        let error = result
            .validate_public_output()
            .expect_err("trace-backed patch validation should reject inconsistent skip reasons");

        assert!(error.to_string().contains("trace_error"));
    }

    #[test]
    fn graph_backed_patch_validation_rejects_missing_neighborhood_for_applied_patch() {
        let result = GraphBackedPatchResult {
            patch: PatchAstNodeResult {
                file: "sample.py".to_string(),
                target_path: "top_level".to_string(),
                resolved_path: "top_level".to_string(),
                resolved_symbol_id: "top_level".to_string(),
                applied: true,
                bypass_applied: false,
                updated_source: "def top_level() -> int:\n    return 1\n".to_string(),
                validation: PatchValidationReport {
                    syntax_errors: Vec::new(),
                    unresolved_identifiers: Vec::new(),
                    resolved_identifiers: Vec::new(),
                    ambiguous_identifiers: Vec::new(),
                    binding_decisions: Vec::new(),
                    commit_gate: PatchCommitGateReport {
                        status: "allowed".to_string(),
                        allowed: true,
                        reason: "ok".to_string(),
                        bypass_reason: None,
                        blocking_decisions: Vec::new(),
                        evidence_invariants: Vec::new(),
                        syntax_error_count: 0,
                    },
                },
            },
            trace_target: "top_level".to_string(),
            trace: Some(
                serde_json::from_str(
                    r#"{
                        "symbol":{
                            "symbol_id":"top_level",
                            "semantic_path":"top_level",
                            "file_path":"sample.py",
                            "node_kind":"function_definition",
                            "origin_type":"trace_root",
                            "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                            "byte_range":[0,10],
                            "parameters":[],
                            "dependencies":[],
                            "references":[]
                        },
                        "callers":[],
                        "callees":[],
                        "evidence_keys":{
                            "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                            "callers":[],
                            "callees":[]
                        },
                        "indexed_files":1
                    }"#,
                )
                .expect("valid trace payload should deserialize"),
            ),
            neighborhood: None,
            trace_validation: Some(PatchTraceValidationResult {
                allowed: true,
                status: "allowed".to_string(),
                reason: "ok".to_string(),
                patch_gate_status: "allowed".to_string(),
                replay_status: "matched".to_string(),
                replay: TracePatchEvidenceReplayResult {
                    consistent: true,
                    matched_items: 0,
                    blocked_items: 0,
                    items: Vec::new(),
                },
            }),
            trace_error: None,
        };

        let error = result
            .validate_public_output()
            .expect_err("applied graph-backed patch results should require a neighborhood");

        assert!(error.to_string().contains("neighborhood"));
    }

    #[test]
    fn neighborhood_context_patch_validation_rejects_missing_neighborhood_context_for_applied_patch()
     {
        let result = NeighborhoodContextPatchResult {
            patch: PatchAstNodeResult {
                file: "sample.py".to_string(),
                target_path: "top_level".to_string(),
                resolved_path: "top_level".to_string(),
                resolved_symbol_id: "top_level".to_string(),
                applied: true,
                bypass_applied: false,
                updated_source: "def top_level() -> int:\n    return 1\n".to_string(),
                validation: PatchValidationReport {
                    syntax_errors: Vec::new(),
                    unresolved_identifiers: Vec::new(),
                    resolved_identifiers: Vec::new(),
                    ambiguous_identifiers: Vec::new(),
                    binding_decisions: Vec::new(),
                    commit_gate: PatchCommitGateReport {
                        status: "allowed".to_string(),
                        allowed: true,
                        reason: "ok".to_string(),
                        bypass_reason: None,
                        blocking_decisions: Vec::new(),
                        evidence_invariants: Vec::new(),
                        syntax_error_count: 0,
                    },
                },
            },
            trace_target: "top_level".to_string(),
            trace: Some(
                serde_json::from_str(
                    r#"{
                        "symbol":{
                            "symbol_id":"top_level",
                            "semantic_path":"top_level",
                            "file_path":"sample.py",
                            "node_kind":"function_definition",
                            "origin_type":"trace_root",
                            "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                            "byte_range":[0,10],
                            "parameters":[],
                            "dependencies":[],
                            "references":[]
                        },
                        "callers":[],
                        "callees":[],
                        "evidence_keys":{
                            "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                            "callers":[],
                            "callees":[]
                        },
                        "indexed_files":1
                    }"#,
                )
                .expect("valid trace payload should deserialize"),
            ),
            neighborhood_context: None,
            trace_validation: Some(PatchTraceValidationResult {
                allowed: true,
                status: "allowed".to_string(),
                reason: "ok".to_string(),
                patch_gate_status: "allowed".to_string(),
                replay_status: "matched".to_string(),
                replay: TracePatchEvidenceReplayResult {
                    consistent: true,
                    matched_items: 0,
                    blocked_items: 0,
                    items: Vec::new(),
                },
            }),
            trace_error: None,
        };

        let error = result.validate_public_output().expect_err(
            "applied neighborhood-context patch results should require neighborhood_context",
        );

        assert!(error.to_string().contains("neighborhood_context"));
    }

    #[test]
    fn discovery_context_patch_validation_rejects_missing_read_for_applied_patch() {
        let result = DiscoveryContextPatchResult {
            patch: PatchAstNodeResult {
                file: "sample.py".to_string(),
                target_path: "top_level".to_string(),
                resolved_path: "top_level".to_string(),
                resolved_symbol_id: "top_level".to_string(),
                applied: true,
                bypass_applied: false,
                updated_source: "def top_level() -> int:\n    return 1\n".to_string(),
                validation: PatchValidationReport {
                    syntax_errors: Vec::new(),
                    unresolved_identifiers: Vec::new(),
                    resolved_identifiers: Vec::new(),
                    ambiguous_identifiers: Vec::new(),
                    binding_decisions: Vec::new(),
                    commit_gate: PatchCommitGateReport {
                        status: "allowed".to_string(),
                        allowed: true,
                        reason: "ok".to_string(),
                        bypass_reason: None,
                        blocking_decisions: Vec::new(),
                        evidence_invariants: Vec::new(),
                        syntax_error_count: 0,
                    },
                },
            },
            trace_target: "top_level".to_string(),
            trace: Some(
                serde_json::from_str(
                    r#"{
                        "symbol":{
                            "symbol_id":"top_level",
                            "semantic_path":"top_level",
                            "file_path":"sample.py",
                            "node_kind":"function_definition",
                            "origin_type":"trace_root",
                            "evidence_key":"top_level|sample.py|function_definition|trace_root|0..10|",
                            "byte_range":[0,10],
                            "parameters":[],
                            "dependencies":[],
                            "references":[]
                        },
                        "callers":[],
                        "callees":[],
                        "evidence_keys":{
                            "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                            "callers":[],
                            "callees":[]
                        },
                        "indexed_files":1
                    }"#,
                )
                .expect("valid trace payload should deserialize"),
            ),
            read: None,
            neighborhood_context: Some(SymbolNeighborhoodContextResult {
                neighborhood: TraceSymbolNeighborhoodResult {
                    symbol: SymbolMeta {
                        symbol_id: "top_level".to_string(),
                        semantic_path: "top_level".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key:
                            "top_level|sample.py|function_definition|trace_root|0..10|"
                                .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                        dependencies: Vec::new(),
                        references: Vec::new(),
                    },
                    direction: TraceDirection::Both,
                    max_depth: 2,
                    max_nodes: 8,
                    truncated: false,
                    indexed_files: 1,
                    nodes: vec![TraceSymbolNeighborhoodNode {
                        symbol: SymbolSummary {
                            symbol_id: "top_level".to_string(),
                            semantic_path: "top_level".to_string(),
                            scope_path: None,
                            file_path: "sample.py".to_string(),
                            node_kind: "function_definition".to_string(),
                            origin_type: "trace_root".to_string(),
                            evidence_key:
                                "top_level|sample.py|function_definition|trace_root|0..10|"
                                    .to_string(),
                            byte_range: (0, 10),
                            signature: None,
                            parameters: Vec::new(),
                            return_type: None,
                            docstring: None,
                        },
                        depth: 0,
                    }],
                    edges: Vec::new(),
                },
                reads: vec![SymbolReadResult {
                    indexed_files: 1,
                    symbol: SymbolSummary {
                        symbol_id: "top_level".to_string(),
                        semantic_path: "top_level".to_string(),
                        scope_path: None,
                        file_path: "sample.py".to_string(),
                        node_kind: "function_definition".to_string(),
                        origin_type: "trace_root".to_string(),
                        evidence_key:
                            "top_level|sample.py|function_definition|trace_root|0..10|"
                                .to_string(),
                        byte_range: (0, 10),
                        signature: None,
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                    },
                    source: "def top_level() -> int:\n    return 1\n".to_string(),
                    start_point: Position { row: 0, column: 0 },
                    end_point: Position { row: 1, column: 12 },
                }],
            }),
            trace_validation: Some(PatchTraceValidationResult {
                allowed: true,
                status: "allowed".to_string(),
                reason: "ok".to_string(),
                patch_gate_status: "allowed".to_string(),
                replay_status: "matched".to_string(),
                replay: TracePatchEvidenceReplayResult {
                    consistent: true,
                    matched_items: 0,
                    blocked_items: 0,
                    items: Vec::new(),
                },
            }),
            trace_error: None,
        };

        let error = result
            .validate_public_output()
            .expect_err("applied discovery-context patch results should require read");

        assert!(error.to_string().contains("read"));
    }
}
