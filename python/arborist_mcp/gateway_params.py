from __future__ import annotations

import json
import math
from typing import Any

from .mcp_validation import reject_unexpected_params
from .tool_result_schemas import JsonRpcError
from .tool_specs import TEXT_PARAM_MAX_LENGTH, TOOL_PARAM_SPECS


class GatewayParameterValidation:
    @staticmethod
    def _require_string(
        params: dict[str, Any],
        key: str,
        allow_empty: bool = False,
        max_length: int | None = None,
    ) -> str:
        value = params.get(key)
        if not isinstance(value, str) or (not allow_empty and not value.strip()):
            raise JsonRpcError(-32602, f"missing required string param: {key}")
        param_spec = TOOL_PARAM_SPECS.get(key)
        effective_max_length = max_length or (
            param_spec.string_max_length if param_spec is not None else None
        )
        GatewayParameterValidation._validate_string_length(value, key, effective_max_length)
        return value

    @staticmethod
    def _require_int(params: dict[str, Any], key: str) -> int:
        value = params.get(key)
        if not isinstance(value, int) or isinstance(value, bool):
            raise JsonRpcError(-32602, f"missing required int param: {key}")
        return value

    @staticmethod
    def _require_nonnegative_int(params: dict[str, Any], key: str) -> int:
        value = GatewayParameterValidation._require_int(params, key)
        if value < 0:
            raise JsonRpcError(-32602, f"invalid non-negative int param: {key}")
        return value

    @staticmethod
    def _optional_string(
        params: dict[str, Any],
        key: str,
        default: str | None = None,
        allow_empty: bool = False,
        max_length: int | None = None,
    ) -> str | None:
        value = params[key] if key in params else default
        if value is None:
            if key in params and default is not None:
                raise JsonRpcError(-32602, f"invalid string param: {key}")
            return None
        if not isinstance(value, str) or (not allow_empty and not value.strip()):
            raise JsonRpcError(-32602, f"invalid string param: {key}")
        param_spec = TOOL_PARAM_SPECS.get(key)
        effective_max_length = max_length or (
            param_spec.string_max_length if param_spec is not None else None
        )
        GatewayParameterValidation._validate_string_length(value, key, effective_max_length)
        return value

    @staticmethod
    def _validate_string_length(
        value: str,
        key: str,
        max_length: int | None,
    ) -> None:
        if max_length is not None and len(value) > max_length:
            raise JsonRpcError(
                -32602,
                f"invalid string param: {key} exceeds max length {max_length}",
            )

    @staticmethod
    def _optional_int(params: dict[str, Any], key: str, default: int) -> int:
        value = params.get(key, default)
        if not isinstance(value, int) or isinstance(value, bool):
            raise JsonRpcError(-32602, f"invalid int param: {key}")
        if value < 0:
            raise JsonRpcError(-32602, f"invalid non-negative int param: {key}")
        param_spec = TOOL_PARAM_SPECS.get(key)
        max_value = param_spec.int_max_value if param_spec is not None else None
        if max_value is not None and value > max_value:
            raise JsonRpcError(
                -32602,
                f"invalid int param: {key} exceeds maximum {max_value}",
            )
        return value

    @staticmethod
    def _optional_positive_int(params: dict[str, Any], key: str, default: int) -> int:
        value = GatewayParameterValidation._optional_int(params, key, default)
        if value == 0:
            raise JsonRpcError(-32602, f"invalid positive int param: {key}")
        return value

    @staticmethod
    def _optional_positive_int_or_none(params: dict[str, Any], key: str) -> int | None:
        if key not in params:
            return None
        value = GatewayParameterValidation._optional_int(params, key, 1)
        if value == 0:
            raise JsonRpcError(-32602, f"invalid positive int param: {key}")
        return value

    @staticmethod
    def _optional_bool(params: dict[str, Any], key: str, default: bool) -> bool:
        value = params.get(key, default)
        if not isinstance(value, bool):
            raise JsonRpcError(-32602, f"invalid bool param: {key}")
        return value

    @staticmethod
    def _optional_string_list(params: dict[str, Any], key: str) -> list[str] | None:
        value = params.get(key)
        if value is None:
            return None
        if not isinstance(value, list) or not all(
            isinstance(item, str) and item.strip() for item in value
        ):
            raise JsonRpcError(-32602, f"invalid string list param: {key}")
        return value

    @staticmethod
    def _optional_choice(
        params: dict[str, Any],
        key: str,
        *,
        default: str,
        allowed: tuple[str, ...],
    ) -> str:
        value = GatewayParameterValidation._optional_string(params, key, default=default)
        if value not in allowed:
            choices = "|".join(allowed)
            raise JsonRpcError(-32602, f"invalid {key} param: expected {choices}")
        return value

    @staticmethod
    def _validate_position_edits(edits: list[Any]) -> None:
        for index, edit in enumerate(edits):
            if not isinstance(edit, dict):
                raise JsonRpcError(-32602, f"invalid position edit at index {index}")
            extra_keys = set(edit) - {"start", "end", "new_text"}
            if extra_keys:
                key = sorted(extra_keys)[0]
                raise JsonRpcError(
                    -32602,
                    f"invalid position edit field: edits[{index}].{key}",
                )
            GatewayParameterValidation._validate_position(edit.get("start"), f"edits[{index}].start")
            GatewayParameterValidation._validate_position(edit.get("end"), f"edits[{index}].end")
            if GatewayParameterValidation._position_tuple(
                edit["start"]
            ) > GatewayParameterValidation._position_tuple(edit["end"]):
                raise JsonRpcError(
                    -32602,
                    f"invalid position edit range: edits[{index}].start is after edits[{index}].end",
                )
            if not isinstance(edit.get("new_text"), str):
                raise JsonRpcError(-32602, f"invalid string param: edits[{index}].new_text")
            GatewayParameterValidation._validate_string_length(
                edit["new_text"],
                f"edits[{index}].new_text",
                TEXT_PARAM_MAX_LENGTH,
            )

    @staticmethod
    def _position_tuple(value: dict[str, Any]) -> tuple[int, int]:
        return (value["row"], value["column"])

    @staticmethod
    def _require_position(params: dict[str, Any], key: str) -> tuple[int, int]:
        value = params.get(key)
        GatewayParameterValidation._validate_position(value, key)
        assert isinstance(value, dict)
        return (value["row"], value["column"])

    @staticmethod
    def _validate_position(value: Any, key: str) -> None:
        if not isinstance(value, dict):
            raise JsonRpcError(-32602, f"invalid position param: {key}")
        extra_keys = set(value) - {"row", "column"}
        if extra_keys:
            field = sorted(extra_keys)[0]
            raise JsonRpcError(-32602, f"invalid position field: {key}.{field}")
        for coordinate in ("row", "column"):
            coordinate_value = value.get(coordinate)
            if (
                not isinstance(coordinate_value, int)
                or isinstance(coordinate_value, bool)
                or coordinate_value < 0
            ):
                raise JsonRpcError(
                    -32602,
                    f"invalid non-negative int param: {key}.{coordinate}",
                )

    @staticmethod
    def _reject_unexpected_params(
        params: dict[str, Any], allowed_keys: tuple[str, ...]
    ) -> None:
        reject_unexpected_params(params, allowed_keys)

    @staticmethod
    def _encode_json_param(value: Any, key: str) -> str:
        GatewayParameterValidation._validate_json_param(value, key)
        try:
            return json.dumps(value, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as exc:
            raise JsonRpcError(-32602, f"invalid JSON-compatible param: {key}") from exc

    @staticmethod
    def _validate_json_param(value: Any, path: str) -> None:
        if value is None or isinstance(value, (bool, str)):
            return
        if isinstance(value, int) and not isinstance(value, bool):
            return
        if isinstance(value, float):
            if math.isfinite(value):
                return
            raise JsonRpcError(-32602, f"invalid finite number param: {path}")
        if isinstance(value, list):
            for index, item in enumerate(value):
                GatewayParameterValidation._validate_json_param(item, f"{path}[{index}]")
            return
        if isinstance(value, dict):
            for item_key, item_value in value.items():
                if not isinstance(item_key, str):
                    raise JsonRpcError(-32602, f"invalid string object key param: {path}")
                GatewayParameterValidation._validate_json_param(item_value, f"{path}.{item_key}")
            return
        raise JsonRpcError(-32602, f"invalid JSON-compatible param: {path}")
