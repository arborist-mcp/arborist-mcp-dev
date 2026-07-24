use anyhow::Result;
use serde_json::{Value, json};

use crate::model::PatchAstNodeResult;

pub fn export_patch_diagnostics_sarif(patch: &PatchAstNodeResult) -> Result<Value> {
    super::validate_replay_patch_payload(patch)?;
    let artifact_uri = sarif_artifact_uri(&patch.file);

    let mut rules = std::collections::BTreeMap::new();
    let mut results = Vec::new();
    for issue in &patch.validation.syntax_errors {
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

    Ok(json!({
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
    }))
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
