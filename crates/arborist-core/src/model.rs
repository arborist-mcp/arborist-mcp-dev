pub const SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION: &str = "4";

mod patch_validation;
mod primitives;
mod query_results;
mod symbols;
mod trace_patch_results;
mod validation;
mod workspace_edit_preview;
pub use patch_validation::{
    DisambiguationContext, PatchAstNodeResult, PatchCommitGateReport, PatchEvidenceInvariantReport,
    PatchPreviewResult, PatchValidationReport, ValidationAmbiguity, ValidationBinding,
    ValidationBindingDecision, ValidationIssue,
};
pub(crate) use primitives::validate_position_edit_batch;
pub use primitives::{
    LanguageId, MAX_POSITION_EDIT_NEW_TEXT_BYTES, MAX_POSITION_EDIT_TEXT_BYTES, MAX_POSITION_EDITS,
    MAX_SEMANTIC_EXPAND_NODES, MAX_WORKSPACE_EDIT_PREVIEW_FILES, Position, PositionEdit,
    QueryCaptureResult, SemanticSkeleton, SemanticSkeletonSymbol, TraceDirection,
};
pub use query_results::{
    RegisteredSymbolIndex, SymbolContextResult, SymbolIndexHealth, SymbolIndexMigrationPlan,
    SymbolIndexStats, SymbolListContextResult, SymbolListDiscoveryContextResult,
    SymbolListNeighborhoodContextResult, SymbolListResult, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, SymbolSearchContextResult,
    SymbolSearchDiscoveryContextResult, SymbolSearchMatchDetail,
    SymbolSearchNeighborhoodContextResult, SymbolSearchResult, VirtualEditResult,
    VirtualFileSnapshot, VirtualFileStatus,
};
pub(crate) use symbols::ensure_unique_symbol_evidence_keys;
pub use symbols::{SymbolMeta, SymbolMetaInit, SymbolSummary, SymbolSummaryInit};
pub use trace_patch_results::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    PatchTraceValidationResult, TraceBackedPatchResult, TraceEvidenceKeys,
    TracePatchEvidenceReplayItem, TracePatchEvidenceReplayResult, TracePatchImpactSummary,
    TraceSymbolGraphResult, TraceSymbolNeighborhoodEdge, TraceSymbolNeighborhoodNode,
    TraceSymbolNeighborhoodResult,
};
pub(crate) use validation::{
    ensure_nonblank, ensure_nonblank_strings, ensure_unique_strings, point_is_after,
};
pub use workspace_edit_preview::{
    WorkspaceEditPreviewFile, WorkspaceEditPreviewResult, WorkspacePositionEdits,
};

#[cfg(test)]
mod tests;
