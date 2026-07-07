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
pub struct SemanticSkeleton {
    pub file: String,
    pub skeleton: String,
    pub available_paths: Vec<String>,
    pub available_symbols: Vec<SemanticSkeletonSymbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
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
pub struct TracePatchEvidenceReplayItem {
    pub name: String,
    pub status: String,
    pub selected_evidence_key: Option<String>,
    pub matched_in_trace: bool,
    pub trace_match_scope: String,
    pub candidate_evidence_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TracePatchEvidenceReplayResult {
    pub consistent: bool,
    pub matched_items: usize,
    pub blocked_items: usize,
    pub items: Vec<TracePatchEvidenceReplayItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatchTraceValidationResult {
    pub allowed: bool,
    pub status: String,
    pub reason: String,
    pub patch_gate_status: String,
    pub replay_status: String,
    pub replay: TracePatchEvidenceReplayResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceBackedPatchResult {
    pub patch: PatchAstNodeResult,
    pub trace_target: String,
    pub trace: Option<TraceSymbolGraphResult>,
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
pub struct SymbolIndexStats {
    pub db_path: String,
    pub indexed_files: usize,
    pub indexed_symbols: usize,
    pub rebuilt_files: usize,
    pub reused_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VirtualFileSnapshot {
    pub file: String,
    pub source: String,
    pub disk_source: String,
    pub dirty: bool,
    pub version: u64,
    pub syntax_error_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VirtualEditResult {
    pub file: String,
    pub source: String,
    pub dirty: bool,
    pub version: u64,
    pub incremental_parse: bool,
    pub validation: PatchValidationReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredSymbolIndex {
    pub workspace_root: String,
    pub db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VirtualFileStatus {
    pub file: String,
    pub dirty: bool,
    pub version: u64,
    pub syntax_error_count: usize,
}

#[cfg(test)]
mod tests {
    use super::{PatchAstNodeResult, Position, PositionEdit, TraceSymbolGraphResult};

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
}
