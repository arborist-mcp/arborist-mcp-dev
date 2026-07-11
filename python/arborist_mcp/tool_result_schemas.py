from __future__ import annotations

from .tool_specs import BATCH_ALLOWED_TOOLS, TOOL_NAMES, TOOL_SPECS, _schema

OBJECT_RESULT_SCHEMA = {
    "type": "object",
    "description": "JSON object result returned by Arborist for this tool.",
    "additionalProperties": True,
}
OBJECT_ARRAY_RESULT_SCHEMA = {
    "type": "array",
    "description": "JSON array of object results returned by Arborist for this tool.",
    "items": OBJECT_RESULT_SCHEMA,
}
BATCH_CALL_RESULT_SCHEMA = {
    "type": "object",
    "description": "Result returned by one inner batch call.",
    "properties": {
        "name": _schema("string", "Arborist tool name that was called."),
        "result": {
            "description": "Result returned by the inner tool. Filled from the batch-allowed tool schemas below.",
        },
    },
    "required": ["name", "result"],
    "additionalProperties": False,
}
BATCH_RESULT_SCHEMA = {
    "type": "array",
    "description": "Ordered results for the requested read-only batch calls.",
    "items": BATCH_CALL_RESULT_SCHEMA,
}
BOOLEAN_RESULT_SCHEMA = {
    "type": "boolean",
    "description": "Boolean success result returned by Arborist for this tool.",
}
NULL_RESULT_SCHEMA = {"type": "null"}
NULLABLE_STRING_RESULT_SCHEMA = {"anyOf": [_schema("string", "String value."), NULL_RESULT_SCHEMA]}
NULLABLE_INTEGER_RESULT_SCHEMA = {
    "anyOf": [_schema("integer", "Integer value.", minimum=0), NULL_RESULT_SCHEMA]
}
POSITION_RESULT_SCHEMA = {
    "type": "object",
    "description": "Zero-based source position.",
    "properties": {
        "row": _schema("integer", "Zero-based row.", minimum=0),
        "column": _schema("integer", "Zero-based UTF-8 byte column.", minimum=0),
    },
    "required": ["row", "column"],
    "additionalProperties": False,
}
BYTE_RANGE_RESULT_SCHEMA = {
    "type": "array",
    "description": "Inclusive start and exclusive end byte offsets.",
    "items": _schema("integer", "Byte offset.", minimum=0),
    "minItems": 2,
    "maxItems": 2,
}
STRING_ARRAY_RESULT_SCHEMA = {
    "type": "array",
    "description": "String values.",
    "items": _schema("string", "String value."),
}
SEMANTIC_SKELETON_SYMBOL_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol metadata available from a semantic skeleton.",
    "properties": {
        "symbol_id": _schema("string", "Stable symbol identifier."),
        "semantic_path": _schema("string", "Stable Arborist semantic selector."),
        "scope_path": NULLABLE_STRING_RESULT_SCHEMA,
        "node_kind": _schema("string", "Tree-sitter node kind."),
        "byte_range": BYTE_RANGE_RESULT_SCHEMA,
        "signature": NULLABLE_STRING_RESULT_SCHEMA,
        "parameters": STRING_ARRAY_RESULT_SCHEMA,
        "return_type": NULLABLE_STRING_RESULT_SCHEMA,
        "docstring": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": [
        "symbol_id",
        "semantic_path",
        "scope_path",
        "node_kind",
        "byte_range",
        "signature",
        "parameters",
        "return_type",
        "docstring",
    ],
    "additionalProperties": False,
}
SEMANTIC_SKELETON_RESULT_SCHEMA = {
    "type": "object",
    "description": "Semantic skeleton and available semantic selectors for a source file.",
    "properties": {
        "file": _schema("string", "Normalized source file path."),
        "skeleton": _schema("string", "Semantic skeleton text.", allow_empty=True),
        "available_paths": {
            "type": "array",
            "description": "Semantic selectors available for expansion or patching.",
            "items": _schema("string", "Semantic selector."),
        },
        "available_symbols": {
            "type": "array",
            "description": "Symbol metadata aligned with available_paths.",
            "items": SEMANTIC_SKELETON_SYMBOL_RESULT_SCHEMA,
        },
    },
    "required": ["file", "skeleton", "available_paths", "available_symbols"],
    "additionalProperties": False,
}
SYMBOL_SUMMARY_RESULT_SCHEMA = {
    "type": "object",
    "description": "Compact symbol metadata.",
    "properties": {
        "symbol_id": _schema("string", "Stable symbol identifier."),
        "semantic_path": _schema("string", "Stable Arborist semantic selector."),
        "scope_path": NULLABLE_STRING_RESULT_SCHEMA,
        "file_path": _schema("string", "Normalized source file path."),
        "node_kind": _schema("string", "Tree-sitter node kind."),
        "origin_type": _schema("string", "Symbol origin classification."),
        "evidence_key": _schema("string", "Trace evidence identity key."),
        "byte_range": BYTE_RANGE_RESULT_SCHEMA,
        "signature": NULLABLE_STRING_RESULT_SCHEMA,
        "parameters": STRING_ARRAY_RESULT_SCHEMA,
        "return_type": NULLABLE_STRING_RESULT_SCHEMA,
        "docstring": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": [
        "symbol_id",
        "semantic_path",
        "scope_path",
        "file_path",
        "node_kind",
        "origin_type",
        "evidence_key",
        "byte_range",
        "signature",
        "parameters",
        "return_type",
        "docstring",
    ],
    "additionalProperties": False,
}
SYMBOL_META_RESULT_SCHEMA = {
    "type": "object",
    "description": "Resolved symbol metadata including graph relationships.",
    "properties": {
        **SYMBOL_SUMMARY_RESULT_SCHEMA["properties"],
        "dependencies": STRING_ARRAY_RESULT_SCHEMA,
        "references": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": [
        *SYMBOL_SUMMARY_RESULT_SCHEMA["required"],
        "dependencies",
        "references",
    ],
    "additionalProperties": False,
}
TRACE_EVIDENCE_KEYS_RESULT_SCHEMA = {
    "type": "object",
    "description": "Trace evidence keys for the root symbol and adjacent symbols.",
    "properties": {
        "symbol": _schema("string", "Root symbol evidence key."),
        "callers": STRING_ARRAY_RESULT_SCHEMA,
        "callees": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": ["symbol", "callers", "callees"],
    "additionalProperties": False,
}
TRACE_SYMBOL_GRAPH_RESULT_SCHEMA = {
    "type": "object",
    "description": "One-hop caller/callee symbol graph.",
    "properties": {
        "symbol": SYMBOL_META_RESULT_SCHEMA,
        "callers": {
            "type": "array",
            "description": "Direct caller symbols.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
        "callees": {
            "type": "array",
            "description": "Direct callee symbols.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
        "evidence_keys": TRACE_EVIDENCE_KEYS_RESULT_SCHEMA,
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
    },
    "required": ["symbol", "callers", "callees", "evidence_keys", "indexed_files"],
    "additionalProperties": False,
}
TRACE_NEIGHBORHOOD_NODE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol neighborhood node with traversal depth.",
    "properties": {
        "symbol": SYMBOL_SUMMARY_RESULT_SCHEMA,
        "depth": _schema("integer", "Traversal depth from root symbol.", minimum=0),
    },
    "required": ["symbol", "depth"],
    "additionalProperties": False,
}
TRACE_NEIGHBORHOOD_EDGE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Directed edge between two symbol identifiers.",
    "properties": {
        "from_symbol_id": _schema("string", "Source symbol identifier."),
        "to_symbol_id": _schema("string", "Target symbol identifier."),
    },
    "required": ["from_symbol_id", "to_symbol_id"],
    "additionalProperties": False,
}
TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA = {
    "type": "object",
    "description": "Bounded caller/callee symbol neighborhood.",
    "properties": {
        "symbol": SYMBOL_META_RESULT_SCHEMA,
        "direction": _schema(
            "string",
            "Graph direction used for traversal.",
            enum=("callers", "callees", "both"),
        ),
        "max_depth": _schema("integer", "Configured traversal depth.", minimum=0),
        "max_nodes": _schema("integer", "Configured node limit.", minimum=0),
        "truncated": _schema("boolean", "Whether traversal stopped at the node limit."),
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
        "nodes": {
            "type": "array",
            "description": "Neighborhood nodes.",
            "items": TRACE_NEIGHBORHOOD_NODE_RESULT_SCHEMA,
        },
        "edges": {
            "type": "array",
            "description": "Neighborhood directed edges.",
            "items": TRACE_NEIGHBORHOOD_EDGE_RESULT_SCHEMA,
        },
    },
    "required": [
        "symbol",
        "direction",
        "max_depth",
        "max_nodes",
        "truncated",
        "indexed_files",
        "nodes",
        "edges",
    ],
    "additionalProperties": False,
}
SYMBOL_READ_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol source slice and location.",
    "properties": {
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
        "symbol": SYMBOL_SUMMARY_RESULT_SCHEMA,
        "source": _schema("string", "Selected symbol source text.", allow_empty=True),
        "start_point": POSITION_RESULT_SCHEMA,
        "end_point": POSITION_RESULT_SCHEMA,
    },
    "required": ["indexed_files", "symbol", "source", "start_point", "end_point"],
    "additionalProperties": False,
}
SYMBOL_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol read result plus one-hop trace context.",
    "properties": {
        "read": SYMBOL_READ_RESULT_SCHEMA,
        "trace": TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
    },
    "required": ["read", "trace"],
    "additionalProperties": False,
}
SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol neighborhood plus source reads for included nodes.",
    "properties": {
        "neighborhood": TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA,
        "reads": {
            "type": "array",
            "description": "Source reads for neighborhood symbols.",
            "items": SYMBOL_READ_RESULT_SCHEMA,
        },
    },
    "required": ["neighborhood", "reads"],
    "additionalProperties": False,
}
SYMBOL_DISCOVERY_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Read, trace, and neighborhood context for symbol discovery.",
    "properties": {
        "read": SYMBOL_READ_RESULT_SCHEMA,
        "trace": TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "neighborhood_context": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
    },
    "required": ["read", "trace", "neighborhood_context"],
    "additionalProperties": False,
}
SYMBOL_LIST_RESULT_SCHEMA = {
    "type": "object",
    "description": "Bounded symbol list.",
    "properties": {
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
        "total_symbols": _schema("integer", "Total matching symbols before truncation.", minimum=0),
        "truncated": _schema("boolean", "Whether results were truncated by limit."),
        "symbols": {
            "type": "array",
            "description": "Symbol summaries.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
    },
    "required": ["indexed_files", "total_symbols", "truncated", "symbols"],
    "additionalProperties": False,
}
SYMBOL_LIST_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol list plus source reads.",
    "properties": {
        "list": SYMBOL_LIST_RESULT_SCHEMA,
        "reads": {
            "type": "array",
            "description": "Source reads for listed symbols.",
            "items": SYMBOL_READ_RESULT_SCHEMA,
        },
    },
    "required": ["list", "reads"],
    "additionalProperties": False,
}
SYMBOL_LIST_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol list plus neighborhood contexts.",
    "properties": {
        "list": SYMBOL_LIST_RESULT_SCHEMA,
        "contexts": {
            "type": "array",
            "description": "Neighborhood contexts for listed symbols.",
            "items": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        },
    },
    "required": ["list", "contexts"],
    "additionalProperties": False,
}
SYMBOL_LIST_DISCOVERY_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol list plus reads and neighborhood contexts.",
    "properties": {
        "list": SYMBOL_LIST_RESULT_SCHEMA,
        "reads": {
            "type": "array",
            "description": "Source reads for listed symbols.",
            "items": SYMBOL_READ_RESULT_SCHEMA,
        },
        "contexts": {
            "type": "array",
            "description": "Neighborhood contexts for listed symbols.",
            "items": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        },
    },
    "required": ["list", "reads", "contexts"],
    "additionalProperties": False,
}
SYMBOL_SEARCH_MATCH_DETAIL_RESULT_SCHEMA = {
    "type": "object",
    "description": "Matched fields and score for a search hit.",
    "properties": {
        "symbol_id": _schema("string", "Matched symbol identifier."),
        "score": _schema("integer", "Lower scores are better matches.", minimum=0),
        "matched_fields": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": ["symbol_id", "score", "matched_fields"],
    "additionalProperties": False,
}
SYMBOL_SEARCH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Bounded symbol search result.",
    "properties": {
        "query": _schema("string", "Search query."),
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
        "total_matches": _schema("integer", "Total matches before truncation.", minimum=0),
        "truncated": _schema("boolean", "Whether results were truncated by limit."),
        "matches": {
            "type": "array",
            "description": "Matched symbol summaries.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
        "match_details": {
            "type": "array",
            "description": "Search scoring and matched fields.",
            "items": SYMBOL_SEARCH_MATCH_DETAIL_RESULT_SCHEMA,
        },
    },
    "required": [
        "query",
        "indexed_files",
        "total_matches",
        "truncated",
        "matches",
        "match_details",
    ],
    "additionalProperties": False,
}
SYMBOL_SEARCH_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol search plus source reads.",
    "properties": {
        "search": SYMBOL_SEARCH_RESULT_SCHEMA,
        "reads": {
            "type": "array",
            "description": "Source reads for search matches.",
            "items": SYMBOL_READ_RESULT_SCHEMA,
        },
    },
    "required": ["search", "reads"],
    "additionalProperties": False,
}
SYMBOL_SEARCH_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol search plus neighborhood contexts.",
    "properties": {
        "search": SYMBOL_SEARCH_RESULT_SCHEMA,
        "contexts": {
            "type": "array",
            "description": "Neighborhood contexts for search matches.",
            "items": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        },
    },
    "required": ["search", "contexts"],
    "additionalProperties": False,
}
SYMBOL_SEARCH_DISCOVERY_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol search plus reads and neighborhood contexts.",
    "properties": {
        "search": SYMBOL_SEARCH_RESULT_SCHEMA,
        "reads": {
            "type": "array",
            "description": "Source reads for search matches.",
            "items": SYMBOL_READ_RESULT_SCHEMA,
        },
        "contexts": {
            "type": "array",
            "description": "Neighborhood contexts for search matches.",
            "items": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        },
    },
    "required": ["search", "reads", "contexts"],
    "additionalProperties": False,
}
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
VIRTUAL_EDIT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Virtual file edit result.",
    "properties": {
        "file": _schema("string", "Normalized virtual file path."),
        "source": _schema("string", "Current virtual buffer source.", allow_empty=True),
        "dirty": _schema("boolean", "Whether the virtual buffer differs from disk."),
        "version": _schema("integer", "Virtual buffer version.", minimum=0),
        "incremental_parse": _schema("boolean", "Whether Tree-sitter reused incremental parsing."),
        "validation": PATCH_VALIDATION_RESULT_SCHEMA,
    },
    "required": ["file", "source", "dirty", "version", "incremental_parse", "validation"],
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
        "trace_error": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": ["patch", "trace_target", "trace", "trace_validation", "trace_error"],
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
QUERY_CAPTURE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Tree-sitter query capture with optional Arborist owner metadata.",
    "properties": {
        "capture_name": _schema("string", "Tree-sitter capture name without the @ prefix."),
        "node_kind": _schema("string", "Captured Tree-sitter node kind."),
        "text": _schema("string", "Captured source text.", allow_empty=True),
        "owner_symbol_id": NULLABLE_STRING_RESULT_SCHEMA,
        "owner_semantic_path": NULLABLE_STRING_RESULT_SCHEMA,
        "owner_scope_path": NULLABLE_STRING_RESULT_SCHEMA,
        "start_byte": _schema("integer", "Inclusive start byte of the captured node.", minimum=0),
        "end_byte": _schema("integer", "Exclusive end byte of the captured node.", minimum=0),
        "start_point": POSITION_RESULT_SCHEMA,
        "end_point": POSITION_RESULT_SCHEMA,
    },
    "required": [
        "capture_name",
        "node_kind",
        "text",
        "owner_symbol_id",
        "owner_semantic_path",
        "owner_scope_path",
        "start_byte",
        "end_byte",
        "start_point",
        "end_point",
    ],
    "additionalProperties": False,
}
QUERY_CAPTURE_ARRAY_RESULT_SCHEMA = {
    "type": "array",
    "description": "Tree-sitter query captures.",
    "items": QUERY_CAPTURE_RESULT_SCHEMA,
}
VIRTUAL_FILE_SNAPSHOT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Session-scoped virtual file snapshot.",
    "properties": {
        "file": _schema("string", "Normalized virtual file path."),
        "source": _schema("string", "Current virtual buffer source.", allow_empty=True),
        "disk_source": _schema("string", "Current on-disk source baseline.", allow_empty=True),
        "dirty": _schema("boolean", "Whether the virtual buffer differs from disk."),
        "version": _schema("integer", "Virtual buffer version.", minimum=0),
        "syntax_error_count": _schema(
            "integer", "Current Tree-sitter syntax error count.", minimum=0
        ),
    },
    "required": [
        "file",
        "source",
        "disk_source",
        "dirty",
        "version",
        "syntax_error_count",
    ],
    "additionalProperties": False,
}
VIRTUAL_FILE_STATUS_RESULT_SCHEMA = {
    "type": "object",
    "description": "Virtual file list entry.",
    "properties": {
        "file": _schema("string", "Normalized virtual file path."),
        "dirty": _schema("boolean", "Whether the virtual buffer differs from disk."),
        "version": _schema("integer", "Virtual buffer version.", minimum=0),
        "syntax_error_count": _schema(
            "integer", "Current Tree-sitter syntax error count.", minimum=0
        ),
    },
    "required": ["file", "dirty", "version", "syntax_error_count"],
    "additionalProperties": False,
}
VIRTUAL_FILE_STATUS_ARRAY_RESULT_SCHEMA = {
    "type": "array",
    "description": "Virtual file status entries.",
    "items": VIRTUAL_FILE_STATUS_RESULT_SCHEMA,
}
SYMBOL_INDEX_STATS_RESULT_SCHEMA = {
    "type": "object",
    "description": "Persisted symbol-index rebuild or refresh statistics.",
    "properties": {
        "db_path": _schema("string", "Normalized SQLite symbol-index database path."),
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
        "indexed_symbols": _schema("integer", "Number of indexed symbols.", minimum=0),
        "rebuilt_files": _schema("integer", "Number of files rebuilt during this operation.", minimum=0),
        "reused_files": _schema("integer", "Number of indexed files reused from prior state.", minimum=0),
    },
    "required": ["db_path", "indexed_files", "indexed_symbols", "rebuilt_files", "reused_files"],
    "additionalProperties": False,
}
REGISTERED_SYMBOL_INDEX_RESULT_SCHEMA = {
    "type": "object",
    "description": "Registered workspace-to-symbol-index mapping.",
    "properties": {
        "workspace_root": _schema("string", "Normalized workspace root path."),
        "db_path": _schema("string", "Normalized SQLite symbol-index database path."),
    },
    "required": ["workspace_root", "db_path"],
    "additionalProperties": False,
}
REGISTERED_SYMBOL_INDEX_ARRAY_RESULT_SCHEMA = {
    "type": "array",
    "description": "Registered workspace-to-symbol-index mappings.",
    "items": REGISTERED_SYMBOL_INDEX_RESULT_SCHEMA,
}
SYMBOL_INDEX_HEALTH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Read-only diagnostic summary for a persisted symbol index.",
    "properties": {
        "response_schema_version": _schema(
            "string", "Version of the inspect_symbol_index response schema."
        ),
        "db_path": _schema("string", "Normalized SQLite symbol-index database path."),
        "exists": _schema("boolean", "Whether the database file exists."),
        "ok": _schema("boolean", "Whether the index passed all inspected health checks."),
        "schema_version": NULLABLE_STRING_RESULT_SCHEMA,
        "expected_schema_version": _schema("string", "Schema version supported by this Arborist build."),
        "workspace_root": NULLABLE_STRING_RESULT_SCHEMA,
        "indexed_files": NULLABLE_INTEGER_RESULT_SCHEMA,
        "indexed_symbols": NULLABLE_INTEGER_RESULT_SCHEMA,
        "file_state_entries": NULLABLE_INTEGER_RESULT_SCHEMA,
        "fresh_file_count": NULLABLE_INTEGER_RESULT_SCHEMA,
        "stale_files": {
            "type": "array",
            "description": "Indexed files whose current content no longer matches persisted fingerprints.",
            "items": _schema("string", "Stale indexed file path."),
        },
        "missing_files": {
            "type": "array",
            "description": "Indexed files that no longer exist on disk.",
            "items": _schema("string", "Missing indexed file path."),
        },
        "unreadable_files": {
            "type": "array",
            "description": "Indexed files that exist but could not be read during freshness inspection.",
            "items": _schema("string", "Unreadable indexed file path."),
        },
        "issues": {
            "type": "array",
            "description": "Human-readable health issues. Empty when ok is true.",
            "items": _schema("string", "Health issue."),
        },
    },
    "required": [
        "response_schema_version",
        "db_path",
        "exists",
        "ok",
        "schema_version",
        "expected_schema_version",
        "workspace_root",
        "indexed_files",
        "indexed_symbols",
        "file_state_entries",
        "fresh_file_count",
        "stale_files",
        "missing_files",
        "unreadable_files",
        "issues",
    ],
    "additionalProperties": False,
}
TOOL_RESULT_SCHEMAS = {
    spec.name: {
        "batch": BATCH_RESULT_SCHEMA,
        "object_array": OBJECT_ARRAY_RESULT_SCHEMA,
        "boolean": BOOLEAN_RESULT_SCHEMA,
        "semantic_skeleton": SEMANTIC_SKELETON_RESULT_SCHEMA,
        "query_capture_array": QUERY_CAPTURE_ARRAY_RESULT_SCHEMA,
        "virtual_file_snapshot": VIRTUAL_FILE_SNAPSHOT_RESULT_SCHEMA,
        "virtual_file_status_array": VIRTUAL_FILE_STATUS_ARRAY_RESULT_SCHEMA,
        "virtual_edit": VIRTUAL_EDIT_RESULT_SCHEMA,
        "symbol_index_stats": SYMBOL_INDEX_STATS_RESULT_SCHEMA,
        "registered_symbol_index": REGISTERED_SYMBOL_INDEX_RESULT_SCHEMA,
        "registered_symbol_index_array": REGISTERED_SYMBOL_INDEX_ARRAY_RESULT_SCHEMA,
        "symbol_index_health": SYMBOL_INDEX_HEALTH_RESULT_SCHEMA,
        "trace_symbol_graph": TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "trace_symbol_neighborhood": TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA,
        "symbol_read": SYMBOL_READ_RESULT_SCHEMA,
        "symbol_context": SYMBOL_CONTEXT_RESULT_SCHEMA,
        "symbol_neighborhood_context": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        "symbol_discovery_context": SYMBOL_DISCOVERY_CONTEXT_RESULT_SCHEMA,
        "symbol_list": SYMBOL_LIST_RESULT_SCHEMA,
        "symbol_list_context": SYMBOL_LIST_CONTEXT_RESULT_SCHEMA,
        "symbol_list_neighborhood_context": SYMBOL_LIST_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        "symbol_list_discovery_context": SYMBOL_LIST_DISCOVERY_CONTEXT_RESULT_SCHEMA,
        "symbol_search": SYMBOL_SEARCH_RESULT_SCHEMA,
        "symbol_search_context": SYMBOL_SEARCH_CONTEXT_RESULT_SCHEMA,
        "symbol_search_neighborhood_context": SYMBOL_SEARCH_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        "symbol_search_discovery_context": SYMBOL_SEARCH_DISCOVERY_CONTEXT_RESULT_SCHEMA,
        "patch_ast_node": PATCH_AST_NODE_RESULT_SCHEMA,
        "patch_preview": PATCH_PREVIEW_RESULT_SCHEMA,
        "trace_patch_evidence_replay": TRACE_PATCH_EVIDENCE_REPLAY_RESULT_SCHEMA,
        "patch_trace_validation": PATCH_TRACE_VALIDATION_RESULT_SCHEMA,
        "trace_backed_patch": TRACE_BACKED_PATCH_RESULT_SCHEMA,
        "graph_backed_patch": GRAPH_BACKED_PATCH_RESULT_SCHEMA,
        "neighborhood_context_patch": NEIGHBORHOOD_CONTEXT_PATCH_RESULT_SCHEMA,
        "discovery_context_patch": DISCOVERY_CONTEXT_PATCH_RESULT_SCHEMA,
    }[spec.result_schema]
    for spec in TOOL_SPECS
    if spec.result_schema != "object"
}
BATCH_CALL_RESULT_SCHEMA["properties"]["result"] = {
    "description": "Result returned by the inner read-only tool.",
    "anyOf": [
        TOOL_RESULT_SCHEMAS[tool_name]
        for tool_name in TOOL_NAMES
        if tool_name in BATCH_ALLOWED_TOOLS and tool_name in TOOL_RESULT_SCHEMAS
    ],
}
TOOL_RESULT_SCHEMAS["arborist/batch"] = BATCH_RESULT_SCHEMA


class JsonRpcError(ValueError):
    def __init__(self, code: int, message: str) -> None:
        super().__init__(message)
        self.code = code



