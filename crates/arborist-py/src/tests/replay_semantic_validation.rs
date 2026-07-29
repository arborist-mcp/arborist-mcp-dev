use super::*;

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
