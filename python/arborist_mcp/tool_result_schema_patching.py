from __future__ import annotations

from .tool_result_schema_common import (
    NULLABLE_STRING_RESULT_SCHEMA,
    NULL_RESULT_SCHEMA,
    POSITION_RESULT_SCHEMA,
    STRING_ARRAY_RESULT_SCHEMA,
)
from .tool_result_schema_symbols import (
    SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
    SYMBOL_READ_RESULT_SCHEMA,
    SYMBOL_SUMMARY_RESULT_SCHEMA,
    TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
    TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA,
)
from .tool_spec_models import _schema


VALIDATION_ISSUE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Tree-sitter validation issue with byte and point ranges.",
    "properties": {
        "kind": _schema("string", "Validation issue kind."),
        "message": _schema("string", "Validation issue message."),
        "start_byte": _schema("integer", "Inclusive start byte.", minimum=0),
        "end_byte": _schema("integer", "Exclusive end byte.", minimum=0),
        "start_point": POSITION_RESULT_SCHEMA,
        "end_point": POSITION_RESULT_SCHEMA,
    },
    "required": ["kind", "message", "start_byte", "end_byte", "start_point", "end_point"],
    "additionalProperties": False,
}

VALIDATION_BINDING_RESULT_SCHEMA = {
    "type": "object",
    "description": "Resolved validation binding.",
    "properties": {
        "name": _schema("string", "Identifier name."),
        "symbol": SYMBOL_SUMMARY_RESULT_SCHEMA,
    },
    "required": ["name", "symbol"],
    "additionalProperties": False,
}

DISAMBIGUATION_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "C include-family disambiguation context.",
    "properties": {
        "active_include_family": NULLABLE_STRING_RESULT_SCHEMA,
        "preferred_family": NULLABLE_STRING_RESULT_SCHEMA,
        "visible_include_families": STRING_ARRAY_RESULT_SCHEMA,
        "candidate_include_families": STRING_ARRAY_RESULT_SCHEMA,
        "candidate_symbol_ids": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": [
        "active_include_family",
        "preferred_family",
        "visible_include_families",
        "candidate_include_families",
        "candidate_symbol_ids",
    ],
    "additionalProperties": False,
}

VALIDATION_AMBIGUITY_RESULT_SCHEMA = {
    "type": "object",
    "description": "Ambiguous identifier validation result.",
    "properties": {
        "name": _schema("string", "Identifier name."),
        "candidates": {
            "type": "array",
            "description": "Candidate symbols for the identifier.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
        "reason": _schema("string", "Why the identifier is ambiguous."),
        "disambiguation_context": DISAMBIGUATION_CONTEXT_RESULT_SCHEMA,
    },
    "required": ["name", "candidates", "reason", "disambiguation_context"],
    "additionalProperties": False,
}

VALIDATION_BINDING_DECISION_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch validation binding decision.",
    "properties": {
        "name": _schema("string", "Identifier name."),
        "status": _schema("string", "Decision status."),
        "reason": _schema("string", "Decision reason."),
        "selected_symbol_id": NULLABLE_STRING_RESULT_SCHEMA,
        "candidates": {
            "type": "array",
            "description": "Candidate symbols considered by the decision.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
    },
    "required": ["name", "status", "reason", "selected_symbol_id", "candidates"],
    "additionalProperties": False,
}

PATCH_EVIDENCE_INVARIANT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Trace evidence invariant checked by the commit gate.",
    "properties": {
        "name": _schema("string", "Invariant name."),
        "status": _schema("string", "Invariant status."),
        "reason": _schema("string", "Invariant reason."),
        "selected_evidence_key": NULLABLE_STRING_RESULT_SCHEMA,
        "candidate_evidence_keys": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": [
        "name",
        "status",
        "reason",
        "selected_evidence_key",
        "candidate_evidence_keys",
    ],
    "additionalProperties": False,
}

PATCH_COMMIT_GATE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Commit gate decision for a patch result.",
    "properties": {
        "status": _schema("string", "Commit gate status."),
        "allowed": _schema("boolean", "Whether the patch may be committed."),
        "reason": _schema("string", "Commit gate reason."),
        "bypass_reason": NULLABLE_STRING_RESULT_SCHEMA,
        "blocking_decisions": {
            "type": "array",
            "description": "Binding decisions that block a normal commit.",
            "items": VALIDATION_BINDING_DECISION_RESULT_SCHEMA,
        },
        "evidence_invariants": {
            "type": "array",
            "description": "Trace evidence invariants evaluated by the gate.",
            "items": PATCH_EVIDENCE_INVARIANT_RESULT_SCHEMA,
        },
        "syntax_error_count": _schema("integer", "Number of syntax errors.", minimum=0),
    },
    "required": [
        "status",
        "allowed",
        "reason",
        "bypass_reason",
        "blocking_decisions",
        "evidence_invariants",
        "syntax_error_count",
    ],
    "additionalProperties": False,
}

PATCH_VALIDATION_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch validation audit report.",
    "properties": {
        "syntax_errors": {
            "type": "array",
            "description": "Syntax errors detected after patching.",
            "items": VALIDATION_ISSUE_RESULT_SCHEMA,
        },
        "unresolved_identifiers": STRING_ARRAY_RESULT_SCHEMA,
        "resolved_identifiers": {
            "type": "array",
            "description": "Identifiers resolved during validation.",
            "items": VALIDATION_BINDING_RESULT_SCHEMA,
        },
        "ambiguous_identifiers": {
            "type": "array",
            "description": "Identifiers that matched multiple candidate symbols.",
            "items": VALIDATION_AMBIGUITY_RESULT_SCHEMA,
        },
        "binding_decisions": {
            "type": "array",
            "description": "Binding decisions made by validation.",
            "items": VALIDATION_BINDING_DECISION_RESULT_SCHEMA,
        },
        "commit_gate": PATCH_COMMIT_GATE_RESULT_SCHEMA,
    },
    "required": [
        "syntax_errors",
        "unresolved_identifiers",
        "resolved_identifiers",
        "ambiguous_identifiers",
        "binding_decisions",
        "commit_gate",
    ],
    "additionalProperties": False,
}

PATCH_AST_NODE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Semantic patch result.",
    "properties": {
        "file": _schema("string", "Normalized patched file path."),
        "target_path": _schema("string", "Requested semantic target path."),
        "resolved_path": _schema("string", "Resolved semantic target path."),
        "resolved_symbol_id": _schema("string", "Resolved target symbol identifier."),
        "applied": _schema("boolean", "Whether the patch was applied."),
        "bypass_applied": _schema("boolean", "Whether a bypass reason was used."),
        "updated_source": _schema("string", "Updated source text.", allow_empty=True),
        "validation": PATCH_VALIDATION_RESULT_SCHEMA,
    },
    "required": [
        "file",
        "target_path",
        "resolved_path",
        "resolved_symbol_id",
        "applied",
        "bypass_applied",
        "updated_source",
        "validation",
    ],
    "additionalProperties": False,
}

PATCH_PREVIEW_RESULT_SCHEMA = {
    "type": "object",
    "description": "Dry-run semantic patch preview.",
    "properties": {
        "patch": PATCH_AST_NODE_RESULT_SCHEMA,
        "unified_diff": _schema("string", "Unified diff for the preview.", allow_empty=True),
        "changed": _schema("boolean", "Whether the preview changes source text."),
    },
    "required": ["patch", "unified_diff", "changed"],
    "additionalProperties": False,
}

TRACE_PATCH_EVIDENCE_REPLAY_ITEM_RESULT_SCHEMA = {
    "type": "object",
    "description": "Single trace evidence replay check.",
    "properties": {
        "name": _schema("string", "Replay item name."),
        "status": _schema("string", "Replay item status."),
        "selected_evidence_key": NULLABLE_STRING_RESULT_SCHEMA,
        "matched_in_trace": _schema("boolean", "Whether selected evidence was found in trace."),
        "trace_match_scope": _schema("string", "Where the evidence matched."),
        "candidate_evidence_keys": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": [
        "name",
        "status",
        "selected_evidence_key",
        "matched_in_trace",
        "trace_match_scope",
        "candidate_evidence_keys",
    ],
    "additionalProperties": False,
}

TRACE_PATCH_EVIDENCE_REPLAY_RESULT_SCHEMA = {
    "type": "object",
    "description": "Trace evidence replay result.",
    "properties": {
        "consistent": _schema("boolean", "Whether replay is consistent with the trace."),
        "matched_items": _schema("integer", "Number of matched replay items.", minimum=0),
        "blocked_items": _schema("integer", "Number of blocked replay items.", minimum=0),
        "items": {
            "type": "array",
            "description": "Replay item details.",
            "items": TRACE_PATCH_EVIDENCE_REPLAY_ITEM_RESULT_SCHEMA,
        },
    },
    "required": ["consistent", "matched_items", "blocked_items", "items"],
    "additionalProperties": False,
}

SARIF_RESULT_SCHEMA = {
    "type": "object",
    "description": "SARIF 2.1.0 diagnostic log generated from a patch validation result.",
    "properties": {
        "version": _schema("string", "SARIF format version."),
        "$schema": _schema("string", "SARIF JSON schema URI."),
        "runs": {"type": "array", "description": "SARIF analysis runs.", "minItems": 1},
    },
    "required": ["version", "$schema", "runs"],
    "additionalProperties": True,
}

WORKSPACE_EDIT_PREVIEW_FILE_SCHEMA = {
    "type": "object",
    "properties": {
        "file": _schema("string", "Normalized source file path."),
        "source": _schema("string", "Updated preview source."),
        "unified_diff": _schema("string", "Unified diff for this file."),
        "changed": _schema("boolean", "Whether this file changes."),
        "validation": PATCH_VALIDATION_RESULT_SCHEMA,
    },
    "required": ["file", "source", "unified_diff", "changed", "validation"],
    "additionalProperties": False,
}

WORKSPACE_EDIT_PREVIEW_RESULT_SCHEMA = {
    "type": "object",
    "description": "Read-only preview of sequential position edits across one or more files.",
    "properties": {
        "changed": _schema("boolean", "Whether any requested file changes."),
        "files": {"type": "array", "description": "Per-file preview results.", "minItems": 1, "items": WORKSPACE_EDIT_PREVIEW_FILE_SCHEMA},
    },
    "required": ["changed", "files"],
    "additionalProperties": False,
}

PATCH_TRACE_VALIDATION_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch commit decision against trace evidence.",
    "properties": {
        "allowed": _schema("boolean", "Whether trace validation allows commit."),
        "status": _schema("string", "Trace validation status."),
        "reason": _schema("string", "Trace validation reason."),
        "patch_gate_status": _schema("string", "Underlying patch commit gate status."),
        "replay_status": _schema("string", "Trace replay status."),
        "replay": TRACE_PATCH_EVIDENCE_REPLAY_RESULT_SCHEMA,
    },
    "required": [
        "allowed",
        "status",
        "reason",
        "patch_gate_status",
        "replay_status",
        "replay",
    ],
    "additionalProperties": False,
}

NULLABLE_TRACE_SYMBOL_GRAPH_RESULT_SCHEMA = {
    "anyOf": [TRACE_SYMBOL_GRAPH_RESULT_SCHEMA, NULL_RESULT_SCHEMA]
}

NULLABLE_TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA = {
    "anyOf": [TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA, NULL_RESULT_SCHEMA]
}

NULLABLE_PATCH_TRACE_VALIDATION_RESULT_SCHEMA = {
    "anyOf": [PATCH_TRACE_VALIDATION_RESULT_SCHEMA, NULL_RESULT_SCHEMA]
}

TRACE_PATCH_IMPACT_SUMMARY_RESULT_SCHEMA = {
    "type": "object",
    "description": "Direct caller and callee changes between pre-patch and post-patch traces.",
    "properties": {
        "added_callers": {"type": "array", "items": SYMBOL_SUMMARY_RESULT_SCHEMA},
        "removed_callers": {"type": "array", "items": SYMBOL_SUMMARY_RESULT_SCHEMA},
        "added_callees": {"type": "array", "items": SYMBOL_SUMMARY_RESULT_SCHEMA},
        "removed_callees": {"type": "array", "items": SYMBOL_SUMMARY_RESULT_SCHEMA},
        "affected_symbol_count": _schema(
            "integer", "Distinct callers or callees changed by the patch.", minimum=0
        ),
    },
    "required": [
        "added_callers",
        "removed_callers",
        "added_callees",
        "removed_callees",
        "affected_symbol_count",
    ],
    "additionalProperties": False,
}

NULLABLE_TRACE_PATCH_IMPACT_SUMMARY_RESULT_SCHEMA = {
    "anyOf": [TRACE_PATCH_IMPACT_SUMMARY_RESULT_SCHEMA, NULL_RESULT_SCHEMA]
}

NULLABLE_SYMBOL_READ_RESULT_SCHEMA = {"anyOf": [SYMBOL_READ_RESULT_SCHEMA, NULL_RESULT_SCHEMA]}

NULLABLE_SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA = {
    "anyOf": [SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA, NULL_RESULT_SCHEMA]
}

TRACE_BACKED_PATCH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch result with optional one-hop trace validation context.",
    "properties": {
        "patch": PATCH_AST_NODE_RESULT_SCHEMA,
        "trace_target": _schema("string", "Trace target symbol selector."),
        "trace": NULLABLE_TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "trace_validation": NULLABLE_PATCH_TRACE_VALIDATION_RESULT_SCHEMA,
        "impact": NULLABLE_TRACE_PATCH_IMPACT_SUMMARY_RESULT_SCHEMA,
        "trace_error": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": ["patch", "trace_target", "trace", "trace_validation", "impact", "trace_error"],
    "additionalProperties": False,
}

GRAPH_BACKED_PATCH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch result with optional trace graph and neighborhood context.",
    "properties": {
        "patch": PATCH_AST_NODE_RESULT_SCHEMA,
        "trace_target": _schema("string", "Trace target symbol selector."),
        "trace": NULLABLE_TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "neighborhood": NULLABLE_TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA,
        "trace_validation": NULLABLE_PATCH_TRACE_VALIDATION_RESULT_SCHEMA,
        "trace_error": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": [
        "patch",
        "trace_target",
        "trace",
        "neighborhood",
        "trace_validation",
        "trace_error",
    ],
    "additionalProperties": False,
}

NEIGHBORHOOD_CONTEXT_PATCH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch result with optional symbol neighborhood context.",
    "properties": {
        "patch": PATCH_AST_NODE_RESULT_SCHEMA,
        "trace_target": _schema("string", "Trace target symbol selector."),
        "trace": NULLABLE_TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "neighborhood_context": NULLABLE_SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        "trace_validation": NULLABLE_PATCH_TRACE_VALIDATION_RESULT_SCHEMA,
        "trace_error": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": [
        "patch",
        "trace_target",
        "trace",
        "neighborhood_context",
        "trace_validation",
        "trace_error",
    ],
    "additionalProperties": False,
}

DISCOVERY_CONTEXT_PATCH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch result with optional read and neighborhood discovery context.",
    "properties": {
        "patch": PATCH_AST_NODE_RESULT_SCHEMA,
        "trace_target": _schema("string", "Trace target symbol selector."),
        "trace": NULLABLE_TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "read": NULLABLE_SYMBOL_READ_RESULT_SCHEMA,
        "neighborhood_context": NULLABLE_SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        "trace_validation": NULLABLE_PATCH_TRACE_VALIDATION_RESULT_SCHEMA,
        "trace_error": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": [
        "patch",
        "trace_target",
        "trace",
        "read",
        "neighborhood_context",
        "trace_validation",
        "trace_error",
    ],
    "additionalProperties": False,
}
