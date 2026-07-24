use super::*;

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
