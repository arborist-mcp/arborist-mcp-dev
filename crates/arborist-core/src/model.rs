use std::collections::BTreeSet;

use anyhow::{Result, bail};
pub const SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION: &str = "1";

mod patch_validation;
mod primitives;
mod query_results;
mod symbols;
mod trace_patch_results;
pub use patch_validation::{
    DisambiguationContext, PatchAstNodeResult, PatchCommitGateReport, PatchEvidenceInvariantReport,
    PatchPreviewResult, PatchValidationReport, ValidationAmbiguity, ValidationBinding,
    ValidationBindingDecision, ValidationIssue,
};
pub use primitives::{
    LanguageId, Position, PositionEdit, QueryCaptureResult, SemanticSkeleton,
    SemanticSkeletonSymbol, TraceDirection,
};
pub use query_results::{
    RegisteredSymbolIndex, SymbolContextResult, SymbolIndexHealth, SymbolIndexStats,
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult,
    SymbolReadResult, SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
    SymbolSearchMatchDetail, SymbolSearchNeighborhoodContextResult, SymbolSearchResult,
    VirtualEditResult, VirtualFileSnapshot, VirtualFileStatus,
};
pub(crate) use symbols::ensure_unique_symbol_evidence_keys;
pub use symbols::{SymbolMeta, SymbolMetaInit, SymbolSummary, SymbolSummaryInit};
pub use trace_patch_results::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    PatchTraceValidationResult, TraceBackedPatchResult, TraceEvidenceKeys,
    TracePatchEvidenceReplayItem, TracePatchEvidenceReplayResult, TraceSymbolGraphResult,
    TraceSymbolNeighborhoodEdge, TraceSymbolNeighborhoodNode, TraceSymbolNeighborhoodResult,
};

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

fn point_is_after(start: &Position, end: &Position) -> bool {
    start.row > end.row || (start.row == end.row && start.column > end.column)
}

#[cfg(test)]
mod tests;
