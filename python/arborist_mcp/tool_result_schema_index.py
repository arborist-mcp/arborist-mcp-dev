from __future__ import annotations

from .tool_result_schema_common import (
    NULLABLE_INTEGER_RESULT_SCHEMA,
    NULLABLE_STRING_RESULT_SCHEMA,
)
from .tool_spec_models import _schema


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

SYMBOL_INDEX_STATS_ARRAY_RESULT_SCHEMA = {
    "type": "array",
    "description": "Refresh statistics for registered persisted symbol indexes.",
    "items": SYMBOL_INDEX_STATS_RESULT_SCHEMA,
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

SYMBOL_INDEX_MIGRATION_PLAN_RESULT_SCHEMA = {
    "type": "object",
    "description": "Machine-readable recommendation for making an inspected symbol index usable.",
    "properties": {
        "required": _schema("boolean", "Whether a migration, rebuild, or manual action is required."),
        "action": _schema(
            "string",
            "Recommended action for the inspected symbol index.",
            enum=("none", "migrate", "rebuild", "manual"),
        ),
        "reason": _schema("string", "Why this migration action was selected."),
    },
    "required": ["required", "action", "reason"],
    "additionalProperties": False,
}

SYMBOL_INDEX_HEALTH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Health summary for a persisted symbol index after inspection or migration.",
    "properties": {
        "response_schema_version": _schema(
            "string", "Version of the inspect_symbol_index response schema."
        ),
        "db_path": _schema("string", "Normalized SQLite symbol-index database path."),
        "exists": _schema("boolean", "Whether the database file exists."),
        "ok": _schema("boolean", "Whether the index passed all inspected health checks."),
        "schema_version": NULLABLE_STRING_RESULT_SCHEMA,
        "expected_schema_version": _schema("string", "Schema version supported by this Arborist build."),
        "migration": SYMBOL_INDEX_MIGRATION_PLAN_RESULT_SCHEMA,
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
        "unindexed_files": {
            "type": "array",
            "description": "Workspace source files that are absent from the persisted index.",
            "items": _schema("string", "Unindexed workspace source file path."),
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
        "migration",
        "workspace_root",
        "indexed_files",
        "indexed_symbols",
        "file_state_entries",
        "fresh_file_count",
        "stale_files",
        "missing_files",
        "unreadable_files",
        "unindexed_files",
        "issues",
    ],
    "additionalProperties": False,
}
