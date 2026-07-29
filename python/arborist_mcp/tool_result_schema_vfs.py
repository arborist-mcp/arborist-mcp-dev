from __future__ import annotations

from .tool_result_schema_patching import PATCH_VALIDATION_RESULT_SCHEMA
from .tool_spec_models import _schema


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
