mod api;
mod api_patch_validation;
mod api_source_query;
mod index_migration;
mod index_schema;
mod index_store;
mod language;
mod model;
mod patching;
mod query;
mod semantic;
mod source_overlay;
mod symbol_dependency;
mod symbol_extractor;
mod symbol_index_model;
mod symbol_index_state;
mod symbol_index_workspace;
mod symbol_map;
mod symbol_position;
mod symbol_query;
mod symbol_query_execution;
mod symbol_read;
mod symbol_search;
mod symbol_summary;
mod symbol_trace;
mod symbols;
mod vfs;
mod workspace_scan;

pub use model::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, LanguageId,
    NeighborhoodContextPatchResult, PatchAstNodeResult, PatchPreviewResult,
    PatchTraceValidationResult, PatchValidationReport, Position, PositionEdit, QueryCaptureResult,
    RegisteredSymbolIndex, SemanticSkeleton, SemanticSkeletonSymbol, SymbolContextResult,
    SymbolIndexHealth, SymbolIndexStats, SymbolListContextResult, SymbolListDiscoveryContextResult,
    SymbolListNeighborhoodContextResult, SymbolListResult, SymbolMeta,
    SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult, SymbolReadResult,
    SymbolSearchContextResult, SymbolSearchDiscoveryContextResult, SymbolSearchMatchDetail,
    SymbolSearchNeighborhoodContextResult, SymbolSearchResult, SymbolSummary,
    TraceBackedPatchResult, TraceDirection, TracePatchEvidenceReplayItem,
    TracePatchEvidenceReplayResult, TraceSymbolGraphResult, TraceSymbolNeighborhoodEdge,
    TraceSymbolNeighborhoodNode, TraceSymbolNeighborhoodResult, ValidationAmbiguity,
    ValidationBinding, ValidationIssue, VirtualEditResult, VirtualFileSnapshot, VirtualFileStatus,
};

pub use api::*;
#[cfg(test)]
pub(crate) use api::{
    validate_discovery_context_patch_result, validate_graph_backed_patch_result,
    validate_neighborhood_context_patch_result, validate_patch_trace_validation_result,
    validate_trace_backed_patch_result, validate_trace_patch_evidence_replay_result,
};
pub use language::{read_source, supported_languages};
pub use patching::{
    patch_ast_node, patch_ast_node_at_position, patch_ast_node_at_position_from_path,
    patch_ast_node_from_path, preview_patch_ast_node, preview_patch_ast_node_at_position,
    preview_patch_ast_node_at_position_from_path, preview_patch_ast_node_from_path,
};
pub use query::{
    DEFAULT_TREE_QUERY_MAX_BYTES, DEFAULT_TREE_QUERY_MAX_CAPTURES, execute_tree_query,
    execute_tree_query_from_path, execute_tree_query_from_path_with_limit,
    execute_tree_query_with_limit,
};
pub use symbol_index_state::inspect_symbol_index;
pub use symbol_query::SymbolQueryContext;
pub use symbols::{
    list_symbols, list_symbols_context, list_symbols_context_filtered,
    list_symbols_context_from_index, list_symbols_context_from_index_filtered,
    list_symbols_discovery_context, list_symbols_discovery_context_filtered,
    list_symbols_discovery_context_from_index, list_symbols_discovery_context_from_index_filtered,
    list_symbols_filtered, list_symbols_from_index, list_symbols_from_index_filtered,
    list_symbols_neighborhood_context, list_symbols_neighborhood_context_filtered,
    list_symbols_neighborhood_context_from_index,
    list_symbols_neighborhood_context_from_index_filtered, read_symbol, read_symbol_at_position,
    read_symbol_at_position_from_index, read_symbol_context, read_symbol_context_at_position,
    read_symbol_context_at_position_from_index, read_symbol_context_from_index,
    read_symbol_discovery_context, read_symbol_discovery_context_at_position,
    read_symbol_discovery_context_at_position_from_index, read_symbol_discovery_context_from_index,
    read_symbol_from_index, read_symbol_neighborhood_context,
    read_symbol_neighborhood_context_at_position,
    read_symbol_neighborhood_context_at_position_from_index,
    read_symbol_neighborhood_context_from_index, rebuild_symbol_index,
    rebuild_symbol_index_with_limits, refresh_symbol_index_for_file,
    refresh_symbol_index_for_file_with_limits, search_symbols, search_symbols_context,
    search_symbols_context_filtered, search_symbols_context_from_index,
    search_symbols_context_from_index_filtered, search_symbols_discovery_context,
    search_symbols_discovery_context_filtered, search_symbols_discovery_context_from_index,
    search_symbols_discovery_context_from_index_filtered, search_symbols_filtered,
    search_symbols_from_index, search_symbols_from_index_filtered,
    search_symbols_neighborhood_context, search_symbols_neighborhood_context_filtered,
    search_symbols_neighborhood_context_from_index,
    search_symbols_neighborhood_context_from_index_filtered, trace_symbol_graph,
    trace_symbol_graph_at_position, trace_symbol_graph_at_position_from_index,
    trace_symbol_graph_from_index, trace_symbol_neighborhood,
    trace_symbol_neighborhood_at_position, trace_symbol_neighborhood_at_position_from_index,
    trace_symbol_neighborhood_from_index,
};
pub use vfs::VirtualFileSystem;
pub use workspace_scan::{DEFAULT_WORKSPACE_MAX_FILES, WorkspaceScanLimits};

#[cfg(test)]
mod tests;
