use serde::{Deserialize, Serialize};

use super::{
    PatchAstNodeResult, SymbolMeta, SymbolNeighborhoodContextResult, SymbolReadResult,
    SymbolSummary, TraceDirection,
};

mod graph;
mod patch_context;
mod replay;
mod validation;

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
pub struct TracePatchImpactSummary {
    pub added_callers: Vec<SymbolSummary>,
    pub removed_callers: Vec<SymbolSummary>,
    pub added_callees: Vec<SymbolSummary>,
    pub removed_callees: Vec<SymbolSummary>,
    pub affected_symbol_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceBackedPatchResult {
    pub patch: PatchAstNodeResult,
    pub trace_target: String,
    pub trace: Option<TraceSymbolGraphResult>,
    pub trace_validation: Option<PatchTraceValidationResult>,
    pub impact: Option<TracePatchImpactSummary>,
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
