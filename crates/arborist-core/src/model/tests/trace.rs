use super::*;

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
