use anyhow::Result;
use serde_json::{Value, json};

use crate::model::PatchAstNodeResult;

pub fn export_patch_diagnostics_sarif(patch: &PatchAstNodeResult) -> Result<Value> {
    export_patch_diagnostics_sarif_inner(patch, None)
}

pub fn export_patch_diagnostics_sarif_with_timeout(
    patch: &PatchAstNodeResult,
    timeout_ms: Option<u64>,
) -> Result<Value> {
    let deadline = super::patch_analysis_deadline(timeout_ms, "SARIF export")?;
    export_patch_diagnostics_sarif_inner(patch, Some(&deadline))
}

#[cfg(test)]
pub(crate) fn export_patch_diagnostics_sarif_with_deadline(
    patch: &PatchAstNodeResult,
    deadline: &dyn crate::deadline::DeadlineCheck,
) -> Result<Value> {
    export_patch_diagnostics_sarif_inner(patch, Some(deadline))
}

fn export_patch_diagnostics_sarif_inner(
    patch: &PatchAstNodeResult,
    deadline: Option<&dyn crate::deadline::DeadlineCheck>,
) -> Result<Value> {
    super::validate_replay_patch_payload_with_deadline(patch, deadline)?;
    check_deadline(deadline, "encoding SARIF artifact URI")?;
    let artifact_uri = sarif_artifact_uri(&patch.file);

    let mut rules = std::collections::BTreeMap::new();
    let mut results = Vec::new();
    for issue in &patch.validation.syntax_errors {
        check_deadline(deadline, "exporting SARIF syntax diagnostics")?;
        let rule_id = format!("arborist.syntax.{}", issue.kind);
        rules.entry(rule_id.clone()).or_insert_with(|| {
            json!({
                "id": rule_id,
                "name": "syntax-error",
                "shortDescription": { "text": "Arborist detected a syntax error." },
            })
        });
        results.push(json!({
            "ruleId": rule_id,
            "level": "error",
            "message": { "text": issue.message },
            "locations": [{
                "physicalLocation": {
                    "artifactLocation": { "uri": artifact_uri },
                    "region": {
                        "startLine": issue.start_point.row + 1,
                        "startColumn": issue.start_point.column + 1,
                        "endLine": issue.end_point.row + 1,
                        "endColumn": issue.end_point.column + 1,
                    },
                },
            }],
        }));
    }

    for decision in &patch.validation.binding_decisions {
        check_deadline(deadline, "exporting SARIF binding diagnostics")?;
        let (rule_id, level) = match decision.status.as_str() {
            "unresolved" => ("arborist.binding.unresolved", "error"),
            "ambiguous" => ("arborist.binding.ambiguous", "warning"),
            _ => continue,
        };
        rules.entry(rule_id.to_string()).or_insert_with(|| {
            json!({
                "id": rule_id,
                "name": "binding-validation",
                "shortDescription": { "text": "Arborist could not safely bind a patch reference." },
            })
        });
        results.push(json!({
            "ruleId": rule_id,
            "level": level,
            "message": { "text": format!("{}: {}", decision.name, decision.reason) },
        }));
    }

    check_deadline(deadline, "exporting SARIF patch gate diagnostic")?;
    if patch.validation.commit_gate.status != "allowed" {
        let level = if patch.validation.commit_gate.allowed {
            "warning"
        } else {
            "error"
        };
        rules
            .entry("arborist.patch-gate".to_string())
            .or_insert_with(|| {
                json!({
                    "id": "arborist.patch-gate",
                    "name": "patch-commit-gate",
                    "shortDescription": { "text": "Arborist patch commit gate decision." },
                })
            });
        results.push(json!({
            "ruleId": "arborist.patch-gate",
            "level": level,
            "message": { "text": patch.validation.commit_gate.reason },
        }));
    }

    check_deadline(deadline, "building SARIF result")?;
    let result = json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "Arborist",
                    "rules": rules.into_values().collect::<Vec<_>>(),
                },
            },
            "columnKind": "utf8CodeUnits",
            "results": results,
        }],
    });
    check_deadline(deadline, "finishing SARIF export")?;
    Ok(result)
}

fn check_deadline(
    deadline: Option<&dyn crate::deadline::DeadlineCheck>,
    phase: &str,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(())
}

pub(crate) fn sarif_artifact_uri(path: &str) -> String {
    let path = path.replace('\\', "/");
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    let encoded = path
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                format!("{}", byte as char).into_bytes()
            }
            _ => format!("%{byte:02X}").into_bytes(),
        })
        .map(char::from)
        .collect::<String>();
    if encoded.starts_with("//") {
        format!("file:{encoded}")
    } else {
        format!("file://{encoded}")
    }
}
