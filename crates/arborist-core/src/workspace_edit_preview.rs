use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};

use crate::language::{
    normalize_absolute_path, normalize_path, offset_for_position, parse_document, read_source,
};
use crate::model::{
    PatchValidationReport, WorkspaceEditPreviewFile, WorkspaceEditPreviewResult,
    WorkspacePositionEdits,
};
use crate::patching::{collect_syntax_errors, splice_source, unified_diff};

pub fn preview_workspace_position_edits(
    requests: &[WorkspacePositionEdits],
) -> Result<WorkspaceEditPreviewResult> {
    if requests.is_empty() {
        bail!("workspace edit preview requires at least one file");
    }

    let mut seen_paths = BTreeSet::new();
    let mut files = Vec::with_capacity(requests.len());
    for (index, request) in requests.iter().enumerate() {
        request.validate_input(index)?;
        let path = normalize_absolute_path(request.file_path.as_ref())?;
        let normalized = normalize_path(&path);
        if !seen_paths.insert(normalized.clone()) {
            bail!("workspace edit preview contains duplicate file: {normalized}");
        }

        let original_source = match &request.source {
            Some(source) => source.clone(),
            None => read_source(&path)?,
        };
        let mut updated_source = original_source.clone();
        for (edit_index, edit) in request.edits.iter().enumerate() {
            let start = offset_for_position(&updated_source, &edit.start)
                .with_context(|| format!("failed to apply position edit at index {edit_index}"))?;
            let end = offset_for_position(&updated_source, &edit.end)
                .with_context(|| format!("failed to apply position edit at index {edit_index}"))?;
            if start > end {
                bail!("failed to apply position edit at index {edit_index}: start is after end");
            }
            updated_source = splice_source(&updated_source, start..end, &edit.new_text);
        }

        let document = parse_document(&path, &updated_source)?;
        let unified_diff = unified_diff(&path, &original_source, &updated_source);
        let validation = PatchValidationReport {
            syntax_errors: collect_syntax_errors(document.tree.root_node(), &updated_source),
            unresolved_identifiers: Vec::new(),
            resolved_identifiers: Vec::new(),
            ambiguous_identifiers: Vec::new(),
            binding_decisions: Vec::new(),
            commit_gate: Default::default(),
        };
        files.push(WorkspaceEditPreviewFile {
            file: normalized,
            source: updated_source,
            changed: !unified_diff.is_empty(),
            unified_diff,
            validation,
        });
    }

    let result = WorkspaceEditPreviewResult {
        changed: files.iter().any(|file| file.changed),
        files,
    };
    result.validate_public_output()?;
    Ok(result)
}
