use super::*;

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
