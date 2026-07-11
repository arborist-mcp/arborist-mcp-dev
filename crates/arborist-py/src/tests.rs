
use super::{
    ArboristCore, PatchAstNodeResult, PositionEdit, TraceSymbolGraphResult, parse_json_arg,
};
use std::sync::Once;

fn prepare_python() {
    static PREPARE: Once = Once::new();
    PREPARE.call_once(pyo3::prepare_freethreaded_python);
}

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

#[test]
fn replay_patch_evidence_against_trace_json_rejects_blank_selected_evidence_keys() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
            .replay_patch_evidence_against_trace_json(
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
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"ok",
                                "selected_evidence_key":"   ",
                                "candidate_evidence_keys":["top_level|sample.py|function_definition|trace_root|0..10|"]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
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
            .expect_err("blank selected evidence keys should be rejected");

    assert!(error.to_string().contains("selected_evidence_key"));
}

#[test]
fn replay_patch_evidence_against_trace_json_rejects_tampered_syntax_error_details() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
        .replay_patch_evidence_against_trace_json(
            r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":false,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return (\n",
                    "validation":{
                        "syntax_errors":[{
                            "kind":"error",
                            "message":"manually tampered",
                            "start_byte":0,
                            "end_byte":1,
                            "start_point":{"row":0,"column":0},
                            "end_point":{"row":0,"column":1}
                        }],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[],
                        "commit_gate":{
                            "status":"rejected",
                            "allowed":false,
                            "reason":"syntax validation failed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[],
                            "syntax_error_count":1
                        }
                    }
                }"#,
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
        .expect_err("tampered syntax error details should be rejected");

    assert!(error.to_string().contains("patch.validation.syntax_errors"));
}

#[test]
fn replay_patch_evidence_against_trace_json_rejects_blank_updated_source() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
        .replay_patch_evidence_against_trace_json(
            r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"   ",
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
        .expect_err("blank updated_source values should be rejected");

    assert!(error.to_string().contains("patch.updated_source"));
}

#[test]
fn replay_patch_evidence_against_trace_json_rejects_duplicate_candidate_evidence_keys() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[{
                            "name":"helper",
                            "symbol":{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }
                        }],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"helper",
                            "status":"resolved",
                            "reason":"resolved uniquely",
                            "selected_symbol_id":"helper",
                            "candidates":[{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }]
                        }],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"resolved binding has one selected candidate evidence key",
                                "selected_evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys":[
                                    "helper|sample.py|function_definition|callee|12..34|",
                                    "helper|sample.py|function_definition|callee|12..34|"
                                ]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
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
                    "callees":[{
                        "symbol_id":"helper",
                        "semantic_path":"helper",
                        "scope_path":null,
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"callee",
                        "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                        "byte_range":[12,34],
                        "signature":null,
                        "parameters":[],
                        "return_type":null,
                        "docstring":null
                    }],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":["helper|sample.py|function_definition|callee|12..34|"]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("duplicate candidate evidence keys should be rejected");

    assert!(error.to_string().contains("candidate_evidence_keys[1]"));
}

#[test]
fn replay_patch_evidence_against_trace_json_rejects_non_root_trace_symbol_origin_type() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
        .replay_patch_evidence_against_trace_json(
            r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return 2\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[],
                            "syntax_error_count":0
                        }
                    }
                }"#,
            r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"callee",
                        "evidence_key":"top_level|sample.py|function_definition|callee|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|callee|0..10|",
                        "callers":[],
                        "callees":[]
                    },
                    "indexed_files":1
                }"#,
        )
        .expect_err("non-root trace symbol origin types should be rejected");

    assert!(error.to_string().contains("trace.symbol.origin_type"));
}

#[test]
fn replay_patch_evidence_against_trace_json_rejects_tampered_resolved_identifier_summaries() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"helper",
                            "status":"resolved",
                            "reason":"resolved uniquely",
                            "selected_symbol_id":"helper",
                            "candidates":[{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }]
                        }],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"resolved binding has one selected candidate evidence key",
                                "selected_evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys":["helper|sample.py|function_definition|callee|12..34|"]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
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
                    "callees":[{
                        "symbol_id":"helper",
                        "semantic_path":"helper",
                        "scope_path":null,
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"callee",
                        "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                        "byte_range":[12,34],
                        "signature":null,
                        "parameters":[],
                        "return_type":null,
                        "docstring":null
                    }],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":["helper|sample.py|function_definition|callee|12..34|"]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("tampered resolved identifier summaries should be rejected");

    assert!(error.to_string().contains("resolved_identifiers"));
}

#[test]
fn replay_patch_evidence_against_trace_json_rejects_unsupported_binding_decision_statuses() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[{
                            "name":"helper",
                            "symbol":{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }
                        }],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"helper",
                            "status":"mystery",
                            "reason":"manually tampered",
                            "selected_symbol_id":"helper",
                            "candidates":[{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }]
                        }],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"resolved binding has one selected candidate evidence key",
                                "selected_evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys":["helper|sample.py|function_definition|callee|12..34|"]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
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
                    "callees":[{
                        "symbol_id":"helper",
                        "semantic_path":"helper",
                        "scope_path":null,
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"callee",
                        "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                        "byte_range":[12,34],
                        "signature":null,
                        "parameters":[],
                        "return_type":null,
                        "docstring":null
                    }],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":["helper|sample.py|function_definition|callee|12..34|"]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("unsupported binding decision statuses should be rejected");

    assert!(error.to_string().contains("binding_decisions[0].status"));
}

#[test]
fn validate_patch_commit_with_trace_json_rejects_inconsistent_patch_gate_flags() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
        .validate_patch_commit_with_trace_json(
            r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":false,
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
        .expect_err("inconsistent patch gate flags should be rejected");

    assert!(error.to_string().contains("patch.applied"));
}

#[test]
fn replay_patch_evidence_against_trace_json_rejects_tampered_patch_gate_reason() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[{
                            "name":"helper",
                            "symbol":{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }
                        }],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"helper",
                            "status":"resolved",
                            "reason":"resolved uniquely",
                            "selected_symbol_id":"helper",
                            "candidates":[{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }]
                        }],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"manually overridden",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"resolved binding has one selected candidate evidence key",
                                "selected_evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys":["helper|sample.py|function_definition|callee|12..34|"]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
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
                    "callees":[{
                        "symbol_id":"helper",
                        "semantic_path":"helper",
                        "scope_path":null,
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"callee",
                        "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                        "byte_range":[12,34],
                        "signature":null,
                        "parameters":[],
                        "return_type":null,
                        "docstring":null
                    }],
                    "evidence_keys":{
                        "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":["helper|sample.py|function_definition|callee|12..34|"]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("tampered patch gate reasons should be rejected");

    assert!(error.to_string().contains("commit_gate.reason"));
}

#[test]
fn replay_patch_evidence_against_trace_json_rejects_mismatched_trace_root() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample.py",
                    "target_path":"top_level",
                    "resolved_path":"top_level",
                    "resolved_symbol_id":"top_level",
                    "applied":true,
                    "bypass_applied":false,
                    "updated_source":"def top_level() -> int:\n    return helper()\n",
                    "validation":{
                        "syntax_errors":[],
                        "unresolved_identifiers":[],
                        "resolved_identifiers":[{
                            "name":"helper",
                            "symbol":{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }
                        }],
                        "ambiguous_identifiers":[],
                        "binding_decisions":[{
                            "name":"helper",
                            "status":"resolved",
                            "reason":"resolved uniquely",
                            "selected_symbol_id":"helper",
                            "candidates":[{
                                "symbol_id":"helper",
                                "semantic_path":"helper",
                                "scope_path":null,
                                "file_path":"sample.py",
                                "node_kind":"function_definition",
                                "origin_type":"callee",
                                "evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "byte_range":[12,34],
                                "signature":null,
                                "parameters":[],
                                "return_type":null,
                                "docstring":null
                            }]
                        }],
                        "commit_gate":{
                            "status":"allowed",
                            "allowed":true,
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[{
                                "name":"helper",
                                "status":"passed",
                                "reason":"resolved binding has one selected candidate evidence key",
                                "selected_evidence_key":"helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys":["helper|sample.py|function_definition|callee|12..34|"]
                            }],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"helper",
                        "semantic_path":"helper",
                        "file_path":"sample.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"helper|sample.py|function_definition|trace_root|12..34|",
                        "byte_range":[12,34],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[],
                    "evidence_keys":{
                        "symbol":"helper|sample.py|function_definition|trace_root|12..34|",
                        "callers":[],
                        "callees":[]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("mismatched trace roots should be rejected");

    assert!(error.to_string().contains("trace.symbol.symbol_id"));
}

#[test]
fn replay_patch_evidence_against_trace_json_rejects_mismatched_trace_root_file() {
    prepare_python();

    let core = ArboristCore::new();
    let error = core
            .replay_patch_evidence_against_trace_json(
                r#"{
                    "file":"sample_a.py",
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
                            "reason":"syntax and symbol binding validation passed",
                            "bypass_reason":null,
                            "blocking_decisions":[],
                            "evidence_invariants":[],
                            "syntax_error_count":0
                        }
                    }
                }"#,
                r#"{
                    "symbol":{
                        "symbol_id":"top_level",
                        "semantic_path":"top_level",
                        "file_path":"sample_b.py",
                        "node_kind":"function_definition",
                        "origin_type":"trace_root",
                        "evidence_key":"top_level|sample_b.py|function_definition|trace_root|0..10|",
                        "byte_range":[0,10],
                        "parameters":[],
                        "dependencies":[],
                        "references":[]
                    },
                    "callers":[],
                    "callees":[],
                    "evidence_keys":{
                        "symbol":"top_level|sample_b.py|function_definition|trace_root|0..10|",
                        "callers":[],
                        "callees":[]
                    },
                    "indexed_files":1
                }"#,
            )
            .expect_err("mismatched trace root files should be rejected");

    assert!(error.to_string().contains("trace.symbol.file_path"));
}
