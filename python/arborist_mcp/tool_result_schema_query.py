from __future__ import annotations

from .tool_result_schema_common import (
    NULLABLE_STRING_RESULT_SCHEMA,
    POSITION_RESULT_SCHEMA,
)
from .tool_spec_models import _schema


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
