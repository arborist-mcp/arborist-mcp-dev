use super::*;

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
