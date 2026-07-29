use super::*;

#[test]
fn parse_json_arg_rejects_duplicate_top_level_keys() {
    prepare_python();

    let error = parse_json_arg::<PositionEdit>(
            r#"{"start":{"row":0,"column":0},"end":{"row":0,"column":1},"new_text":"x","new_text":"y"}"#,
        )
        .expect_err("duplicate top-level keys should be rejected");

    assert!(
        error
            .to_string()
            .contains("duplicate JSON object key `new_text`")
    );
}
#[test]
fn parse_json_arg_rejects_duplicate_nested_keys() {
    prepare_python();

    let error = parse_json_arg::<Vec<PositionEdit>>(
        r#"[{"start":{"row":0,"column":0,"row":1},"end":{"row":0,"column":1},"new_text":"x"}]"#,
    )
    .expect_err("duplicate nested keys should be rejected");

    assert!(
        error
            .to_string()
            .contains("duplicate JSON object key `row`")
    );
}
#[test]
fn parse_json_arg_accepts_valid_payloads() {
    prepare_python();

    let edits = parse_json_arg::<Vec<PositionEdit>>(
        r#"[{"start":{"row":0,"column":0},"end":{"row":0,"column":1},"new_text":"x"}]"#,
    )
    .expect("valid edit payload should parse");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "x");
}
#[test]
fn parse_json_arg_rejects_oversized_payloads() {
    let payload = format!("{{\"file_path\":\"{}\"}}", "x".repeat(MAX_JSON_ARG_BYTES));
    let error = parse_json_arg::<PositionEdit>(&payload)
        .expect_err("oversized JSON payload should be rejected");
    assert!(error.to_string().contains("exceeds maximum size"));
}
#[test]
fn parse_json_arg_rejects_excessive_nesting() {
    let mut payload = "[".repeat(MAX_JSON_ARG_DEPTH + 1);
    payload.push('0');
    payload.push_str(&"]".repeat(MAX_JSON_ARG_DEPTH + 1));

    let error = parse_json_arg::<Value>(&payload)
        .expect_err("excessively nested JSON payload should be rejected");
    assert!(error.to_string().contains("maximum nesting depth"));
}
#[test]
fn parse_json_arg_rejects_missing_nested_trace_fields() {
    prepare_python();

    let error = parse_json_arg::<TraceSymbolGraphResult>(
        r#"{
                "symbol":{"symbol_id":"top_level"},
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
    .expect_err("trace payloads should reject missing nested symbol fields");

    assert!(error.to_string().contains("missing field"));
}
#[test]
fn parse_json_arg_rejects_missing_nested_patch_fields() {
    prepare_python();

    let error = parse_json_arg::<PatchAstNodeResult>(
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
    .expect_err("patch payloads should reject missing nested validation fields");

    assert!(error.to_string().contains("missing field"));
}
