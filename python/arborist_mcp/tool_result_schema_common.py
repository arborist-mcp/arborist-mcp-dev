from __future__ import annotations

from .tool_spec_models import _schema


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
