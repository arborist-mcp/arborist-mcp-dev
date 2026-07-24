use super::*;

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
