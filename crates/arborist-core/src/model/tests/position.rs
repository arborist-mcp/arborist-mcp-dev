use super::*;

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
