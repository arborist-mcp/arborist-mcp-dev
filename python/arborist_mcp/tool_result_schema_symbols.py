from __future__ import annotations

from .tool_result_schema_common import (
    BYTE_RANGE_RESULT_SCHEMA,
    NULLABLE_STRING_RESULT_SCHEMA,
    POSITION_RESULT_SCHEMA,
    STRING_ARRAY_RESULT_SCHEMA,
)
from .tool_specs import _schema


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
