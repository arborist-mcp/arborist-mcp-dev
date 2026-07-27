use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::language::{
    normalize_absolute_path, normalize_path, offset_for_position, parse_document, read_source,
    validate_source_length, validate_source_size,
};
use crate::model::{
    MAX_WORKSPACE_EDIT_PREVIEW_FILES, PatchValidationReport, WorkspaceEditPreviewFile,
    WorkspaceEditPreviewResult, WorkspacePositionEdits,
};
use crate::patching::{collect_syntax_errors, splice_source, unified_diff};
use crate::workspace_scan::MAX_WORKSPACE_SCAN_TIMEOUT_MS;

struct WorkspaceEditPreviewDeadline {
    deadline: Option<Instant>,
    timeout_ms: Option<u64>,
}

impl WorkspaceEditPreviewDeadline {
    fn new(timeout_ms: Option<u64>) -> Result<Self> {
        if timeout_ms == Some(0) {
            bail!("invalid workspace edit preview timeout_ms: value must be greater than zero");
        }
        if timeout_ms.is_some_and(|value| value > MAX_WORKSPACE_SCAN_TIMEOUT_MS) {
            bail!(
                "invalid workspace edit preview timeout_ms: value must not exceed {MAX_WORKSPACE_SCAN_TIMEOUT_MS}"
            );
        }
        Ok(Self {
            deadline: timeout_ms.map(|value| Instant::now() + Duration::from_millis(value)),
            timeout_ms,
        })
    }

    fn check(&self, phase: &str) -> Result<()> {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            bail!(
                "workspace edit preview timeout exceeded during {phase}: timeout_ms={}",
                self.timeout_ms.unwrap_or_default()
            );
        }
        Ok(())
    }
}

pub fn preview_workspace_position_edits(
    requests: &[WorkspacePositionEdits],
) -> Result<WorkspaceEditPreviewResult> {
    preview_workspace_position_edits_with_timeout(requests, None)
}

pub fn preview_workspace_position_edits_with_timeout(
    requests: &[WorkspacePositionEdits],
    timeout_ms: Option<u64>,
) -> Result<WorkspaceEditPreviewResult> {
    let deadline = WorkspaceEditPreviewDeadline::new(timeout_ms)?;
    if requests.is_empty() {
        bail!("workspace edit preview requires at least one file");
    }
    if requests.len() > MAX_WORKSPACE_EDIT_PREVIEW_FILES {
        bail!("workspace edit preview accepts at most {MAX_WORKSPACE_EDIT_PREVIEW_FILES} files");
    }

    let mut seen_paths = BTreeSet::new();
    let mut files = Vec::with_capacity(requests.len());
    for (index, request) in requests.iter().enumerate() {
        deadline.check("file validation")?;
        request.validate_input(index)?;
        let path = normalize_absolute_path(request.file_path.as_ref())?;
        let normalized = normalize_path(&path);
        if !seen_paths.insert(normalized.clone()) {
            bail!("workspace edit preview contains duplicate file: {normalized}");
        }

        deadline.check("source read")?;
        let original_source = match &request.source {
            Some(source) => source.clone(),
            None => read_source(&path)?,
        };
        deadline.check("source validation")?;
        validate_source_size(&path, &original_source)?;
        let mut updated_source = original_source.clone();
        for (edit_index, edit) in request.edits.iter().enumerate() {
            deadline.check("position edit application")?;
            let start = offset_for_position(&updated_source, &edit.start)
                .with_context(|| format!("failed to apply position edit at index {edit_index}"))?;
            let end = offset_for_position(&updated_source, &edit.end)
                .with_context(|| format!("failed to apply position edit at index {edit_index}"))?;
            if start > end {
                bail!("failed to apply position edit at index {edit_index}: start is after end");
            }
            let result_len = updated_source
                .len()
                .checked_sub(end - start)
                .and_then(|length| length.checked_add(edit.new_text.len()))
                .ok_or_else(|| anyhow::anyhow!("updated source size overflowed"))?;
            validate_source_length(&path, result_len)?;
            updated_source = splice_source(&updated_source, start..end, &edit.new_text);
        }

        deadline.check("updated source parse")?;
        let document = parse_document(&path, &updated_source)?;
        deadline.check("workspace edit diff")?;
        let unified_diff = unified_diff(&path, &original_source, &updated_source);
        deadline.check("workspace edit syntax validation")?;
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

    deadline.check("workspace edit preview result")?;
    let result = WorkspaceEditPreviewResult {
        changed: files.iter().any(|file| file.changed),
        files,
    };
    result.validate_public_output()?;
    Ok(result)
}
