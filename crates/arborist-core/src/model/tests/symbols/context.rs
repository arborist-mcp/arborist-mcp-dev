use super::*;

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
