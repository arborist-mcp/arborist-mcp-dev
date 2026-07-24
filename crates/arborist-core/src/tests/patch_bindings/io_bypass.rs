use super::*;

#[test]
fn rejects_patch_with_unresolved_identifier_without_bypass() {
    let source = r#"
def helper(value: int) -> int:
    return value + 1

def top_level(value: int) -> int:
    return helper(value)
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: int) -> int:\n    return missing_helper(value)\n",
        None,
    )
    .unwrap();

    assert!(!result.applied);
    assert!(!result.validation.commit_gate.allowed);
    assert_eq!(result.validation.commit_gate.status, "rejected");
    assert_eq!(
        result.validation.commit_gate.reason,
        "symbol binding is unresolved"
    );
    assert_eq!(
        result.validation.unresolved_identifiers,
        vec!["missing_helper"]
    );
    assert_eq!(result.validation.binding_decisions.len(), 2);
    let missing_helper_decision = result
        .validation
        .binding_decisions
        .iter()
        .find(|decision| decision.name == "missing_helper")
        .unwrap();
    assert_eq!(missing_helper_decision.status, "unresolved");
    assert_eq!(missing_helper_decision.selected_symbol_id, None);
    assert!(missing_helper_decision.candidates.is_empty());
    assert!(
        result
            .validation
            .binding_decisions
            .iter()
            .any(|decision| decision.name == "value" && decision.status == "resolved")
    );
}

#[test]
fn allows_patch_with_bypass_reason() {
    let source = r#"
def top_level(value: int) -> int:
    return value + 1
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: int) -> int:\n    return dynamic_runtime(value)\n",
        Some("resolved at runtime by the embedding host"),
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.bypass_applied);
    assert!(result.validation.commit_gate.allowed);
    assert_eq!(result.validation.commit_gate.status, "allowed_with_bypass");
    assert_eq!(
        result.validation.commit_gate.bypass_reason.as_deref(),
        Some("resolved at runtime by the embedding host")
    );
}

#[test]
fn rejects_blank_patch_bypass_reasons() {
    let source = r#"
def top_level(value: int) -> int:
    return value + 1
"#;

    let error = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 2\n",
        Some(" \t"),
    )
    .expect_err("blank bypass reasons should be rejected");

    assert!(error.to_string().contains("bypass_reason"));
    assert!(error.to_string().contains("blank"));
}

#[test]
fn writes_applied_patch_to_disk() {
    let dir = temporary_dir();
    let file = dir.join("patch_target.py");
    fs::write(
        &file,
        "def top_level(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let result = patch_ast_node_from_path(
        &file,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 2\n",
        None,
    )
    .unwrap();

    let updated = fs::read_to_string(&file).unwrap();
    assert!(result.applied);
    assert!(updated.contains("return value + 2"));
}

#[test]
fn previews_patch_diff_without_writing_to_disk() {
    let dir = temporary_dir();
    let file = dir.join("patch_target.py");
    let original = "def top_level(value: int) -> int:\n    return value + 1\n";
    fs::write(&file, original).unwrap();

    let result = preview_patch_ast_node_from_path(
        &file,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 2\n",
        None,
    )
    .unwrap();

    let disk_source = fs::read_to_string(&file).unwrap();
    assert_eq!(disk_source, original);
    assert!(result.patch.applied);
    assert!(result.changed);
    assert!(result.unified_diff.contains("--- a/"));
    assert!(result.unified_diff.contains("-    return value + 1"));
    assert!(result.unified_diff.contains("+    return value + 2"));
}
