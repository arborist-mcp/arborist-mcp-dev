use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use crate::deadline::CooperativeDeadline;
use crate::model::{
    PatchAstNodeResult, SymbolSummary, TracePatchImpactSummary, TraceSymbolGraphResult,
};
use crate::patching;
use crate::symbol_trace::TraceQueryDeadline;

mod path_index_context;
mod replay;
mod result_validation;
mod sarif;
#[cfg(test)]
mod tests;
mod workspace_context;

pub use path_index_context::*;
pub use replay::{
    replay_patch_evidence_against_trace, replay_patch_evidence_against_trace_with_timeout,
    validate_patch_commit_with_trace, validate_patch_commit_with_trace_with_timeout,
};
#[cfg(test)]
pub(crate) use replay::{
    replay_patch_evidence_against_trace_with_deadline,
    validate_patch_commit_with_trace_with_deadline,
};
pub(crate) use replay::{
    validate_replay_patch_payload_with_deadline, validate_replay_trace_target,
};
pub(crate) use result_validation::{
    validate_discovery_context_patch_result, validate_graph_backed_patch_result,
    validate_neighborhood_context_patch_result, validate_patch_trace_validation_result,
    validate_trace_backed_patch_result, validate_trace_patch_evidence_replay_result,
};
pub use sarif::{export_patch_diagnostics_sarif, export_patch_diagnostics_sarif_with_timeout};
#[cfg(test)]
pub(crate) use sarif::{export_patch_diagnostics_sarif_with_deadline, sarif_artifact_uri};
pub use workspace_context::*;
pub(crate) use workspace_context::{
    validate_patch_with_discovery_context_at_position_with_deadline,
    validate_patch_with_discovery_context_with_deadline,
};
pub(crate) use workspace_context::{
    validate_patch_with_graph_context_at_position_with_deadline,
    validate_patch_with_graph_context_with_deadline,
};
pub(crate) use workspace_context::{
    validate_patch_with_neighborhood_context_at_position_with_deadline,
    validate_patch_with_neighborhood_context_with_deadline,
};
pub(crate) use workspace_context::{
    validate_patch_with_trace_context_at_position_with_deadline,
    validate_patch_with_trace_context_with_deadline,
};

pub const MAX_PATCH_ANALYSIS_TIMEOUT_MS: u64 = 5 * 60 * 1_000;

pub(crate) fn patch_analysis_deadline(
    timeout_ms: Option<u64>,
    operation: &'static str,
) -> Result<CooperativeDeadline> {
    CooperativeDeadline::new(timeout_ms, MAX_PATCH_ANALYSIS_TIMEOUT_MS, operation)
}

pub(crate) fn patch_ast_node_with_trace_deadline(
    deadline: &TraceQueryDeadline,
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchAstNodeResult> {
    patching::patch_ast_node_with_deadline(
        path,
        source,
        semantic_target,
        new_code,
        bypass_reason,
        Some(deadline),
    )
}

pub(crate) fn trace_patch_impact_summary(
    before: &TraceSymbolGraphResult,
    after: &TraceSymbolGraphResult,
) -> TracePatchImpactSummary {
    let before_callers = symbols_by_symbol_id(&before.callers);
    let after_callers = symbols_by_symbol_id(&after.callers);
    let before_callees = symbols_by_symbol_id(&before.callees);
    let after_callees = symbols_by_symbol_id(&after.callees);

    let added_callers = changed_symbols(&after_callers, &before_callers);
    let removed_callers = changed_symbols(&before_callers, &after_callers);
    let added_callees = changed_symbols(&after_callees, &before_callees);
    let removed_callees = changed_symbols(&before_callees, &after_callees);
    let affected_symbol_count = added_callers
        .iter()
        .chain(&removed_callers)
        .chain(&added_callees)
        .chain(&removed_callees)
        .map(|symbol| symbol.symbol_id.clone())
        .collect::<BTreeSet<_>>()
        .len();

    TracePatchImpactSummary {
        added_callers,
        removed_callers,
        added_callees,
        removed_callees,
        affected_symbol_count,
    }
}

fn symbols_by_symbol_id(symbols: &[SymbolSummary]) -> BTreeMap<&str, &SymbolSummary> {
    symbols
        .iter()
        .map(|symbol| (symbol.symbol_id.as_str(), symbol))
        .collect()
}

fn changed_symbols(
    left: &BTreeMap<&str, &SymbolSummary>,
    right: &BTreeMap<&str, &SymbolSummary>,
) -> Vec<SymbolSummary> {
    left.iter()
        .filter(|(key, _)| !right.contains_key(**key))
        .map(|(_, symbol)| (*symbol).clone())
        .collect()
}
