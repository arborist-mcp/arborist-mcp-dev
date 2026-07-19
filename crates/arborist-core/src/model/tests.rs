use super::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    PatchAstNodeResult, PatchCommitGateReport, PatchTraceValidationResult, PatchValidationReport,
    Position, PositionEdit, QueryCaptureResult, RegisteredSymbolIndex, SemanticSkeleton,
    SemanticSkeletonSymbol, SymbolIndexHealth, SymbolIndexMigrationPlan, SymbolIndexStats,
    SymbolListContextResult, SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult,
    SymbolListResult, SymbolMeta, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, SymbolSearchContextResult,
    SymbolSearchDiscoveryContextResult, SymbolSearchMatchDetail,
    SymbolSearchNeighborhoodContextResult, SymbolSearchResult, SymbolSummary,
    TraceBackedPatchResult, TraceDirection, TraceEvidenceKeys, TracePatchEvidenceReplayItem,
    TracePatchEvidenceReplayResult, TraceSymbolGraphResult, TraceSymbolNeighborhoodNode,
    TraceSymbolNeighborhoodResult, ValidationBindingDecision, VirtualEditResult,
    VirtualFileSnapshot, VirtualFileStatus, WorkspaceEditPreviewFile, WorkspaceEditPreviewResult,
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
fn workspace_edit_preview_rejects_duplicate_files() {
    let result = WorkspaceEditPreviewResult {
        changed: false,
        files: vec![
            WorkspaceEditPreviewFile {
                file: "sample.py".to_string(),
                source: "value = 1\n".to_string(),
                unified_diff: String::new(),
                changed: false,
                validation: PatchValidationReport::default(),
            },
            WorkspaceEditPreviewFile {
                file: "sample.py".to_string(),
                source: "value = 1\n".to_string(),
                unified_diff: String::new(),
                changed: false,
                validation: PatchValidationReport::default(),
            },
        ],
    };

    let error = result
        .validate_public_output()
        .expect_err("workspace previews must not repeat files");

    assert!(error.to_string().contains("duplicate preview files"));
}

#[test]
fn symbol_index_health_rejects_required_migration_without_action() {
    let health = SymbolIndexHealth {
        response_schema_version: "4".to_string(),
        db_path: "symbols.db".to_string(),
        exists: false,
        ok: false,
        schema_version: None,
        expected_schema_version: "4".to_string(),
        migration: SymbolIndexMigrationPlan {
            required: true,
            action: "none".to_string(),
            reason: "index must be rebuilt".to_string(),
        },
        workspace_root: None,
        indexed_files: None,
        indexed_symbols: None,
        file_state_entries: None,
        fresh_file_count: None,
        stale_files: Vec::new(),
        missing_files: Vec::new(),
        unreadable_files: Vec::new(),
        unindexed_files: Vec::new(),
        issues: vec!["symbol index does not exist".to_string()],
    };

    let error = health
        .validate_public_output()
        .expect_err("required migrations must provide a concrete action");

    assert!(error.to_string().contains("migration.required"));
}

#[test]
fn symbol_index_health_rejects_non_rebuild_action_for_missing_index() {
    let health = SymbolIndexHealth {
        response_schema_version: "4".to_string(),
        db_path: "symbols.db".to_string(),
        exists: false,
        ok: false,
        schema_version: None,
        expected_schema_version: "4".to_string(),
        migration: SymbolIndexMigrationPlan {
            required: true,
            action: "manual".to_string(),
            reason: "index cannot be opened".to_string(),
        },
        workspace_root: None,
        indexed_files: None,
        indexed_symbols: None,
        file_state_entries: None,
        fresh_file_count: None,
        stale_files: Vec::new(),
        missing_files: Vec::new(),
        unreadable_files: Vec::new(),
        unindexed_files: Vec::new(),
        issues: vec!["symbol index does not exist".to_string()],
    };

    let error = health
        .validate_public_output()
        .expect_err("missing indexes must recommend rebuild");

    assert!(error.to_string().contains("migration.action"));
}

#[test]
fn symbol_index_health_rejects_incomplete_healthy_inspection() {
    let health = SymbolIndexHealth {
        response_schema_version: "4".to_string(),
        db_path: "symbols.db".to_string(),
        exists: true,
        ok: true,
        schema_version: Some("4".to_string()),
        expected_schema_version: "4".to_string(),
        migration: SymbolIndexMigrationPlan {
            required: false,
            action: "none".to_string(),
            reason: "index schema and persisted file fingerprints are current".to_string(),
        },
        workspace_root: Some("workspace".to_string()),
        indexed_files: Some(1),
        indexed_symbols: Some(1),
        file_state_entries: Some(1),
        fresh_file_count: None,
        stale_files: Vec::new(),
        missing_files: Vec::new(),
        unreadable_files: Vec::new(),
        unindexed_files: Vec::new(),
        issues: Vec::new(),
    };

    let error = health
        .validate_public_output()
        .expect_err("healthy indexes must include a complete inspection snapshot");

    assert!(error.to_string().contains("complete current inspection"));
}

#[test]
fn symbol_index_health_rejects_duplicate_freshness_file_paths() {
    let health = SymbolIndexHealth {
        response_schema_version: "4".to_string(),
        db_path: "symbols.db".to_string(),
        exists: true,
        ok: false,
        schema_version: Some("4".to_string()),
        expected_schema_version: "4".to_string(),
        migration: SymbolIndexMigrationPlan {
            required: true,
            action: "rebuild".to_string(),
            reason: "index health checks failed".to_string(),
        },
        workspace_root: Some("workspace".to_string()),
        indexed_files: Some(1),
        indexed_symbols: Some(1),
        file_state_entries: Some(2),
        fresh_file_count: Some(0),
        stale_files: vec!["workspace/helper.py".to_string()],
        missing_files: vec!["workspace/helper.py".to_string()],
        unreadable_files: Vec::new(),
        unindexed_files: Vec::new(),
        issues: vec!["indexed file is stale".to_string()],
    };

    let error = health
        .validate_public_output()
        .expect_err("freshness categories must not overlap");

    assert!(error.to_string().contains("duplicate freshness file paths"));
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
        evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|".to_string(),
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
        evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|".to_string(),
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
        evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|".to_string(),
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
        evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|".to_string(),
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
        evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|".to_string(),
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
        evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|".to_string(),
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

    assert!(
        error.to_string().contains(
            "symbol_search_neighborhood_context.contexts[0].neighborhood.symbol.symbol_id"
        )
    );
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
        evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|".to_string(),
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

    assert!(
        error
            .to_string()
            .contains("symbol_list_neighborhood_context.contexts[0].neighborhood.symbol.symbol_id")
    );
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
        evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|".to_string(),
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

    assert!(
        error.to_string().contains(
            "symbol_search_neighborhood_context.contexts[0].neighborhood.symbol.symbol_id"
        )
    );
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
                evidence_key: "helper|sample.py|function_definition|trace_root|0..10|".to_string(),
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
        evidence_key: "helper|sample.py|function_definition|workspace_symbol|0..10|".to_string(),
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

    assert!(
        error
            .to_string()
            .contains("symbol_list_neighborhood_context.contexts[0].neighborhood.symbol.symbol_id")
    );
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
            impact: None,
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
        impact: None,
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
fn neighborhood_context_patch_validation_rejects_missing_neighborhood_context_for_applied_patch() {
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
