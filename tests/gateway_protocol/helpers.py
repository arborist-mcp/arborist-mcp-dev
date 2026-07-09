from __future__ import annotations

import copy
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator

from arborist_mcp.gateway import ArboristGateway

_UNSET = object()


def make_gateway(core: object = _UNSET) -> ArboristGateway:
    gateway = ArboristGateway.__new__(ArboristGateway)
    if core is not _UNSET:
        gateway._core = core
    return gateway


def make_request(
    method: str,
    params: object | None = None,
    *,
    request_id: object = 1,
    jsonrpc: str = "2.0",
) -> dict[str, object]:
    return {
        "jsonrpc": jsonrpc,
        "id": request_id,
        "method": method,
        "params": {} if params is None else params,
    }


def deep_merge(base: object, updates: object) -> object:
    if isinstance(base, dict) and isinstance(updates, dict):
        merged = copy.deepcopy(base)
        for key, value in updates.items():
            if key in merged:
                merged[key] = deep_merge(merged[key], value)
            else:
                merged[key] = copy.deepcopy(value)
        return merged
    return copy.deepcopy(updates)


@contextmanager
def temp_workspace(files: dict[str, str] | None = None) -> Iterator[Path]:
    with tempfile.TemporaryDirectory() as temp_dir:
        workspace = Path(temp_dir)
        for relative_path, contents in (files or {}).items():
            file_path = workspace.joinpath(relative_path)
            file_path.parent.mkdir(parents=True, exist_ok=True)
            file_path.write_text(contents, encoding="utf-8")
        yield workspace


class GatewayProtocolTestCase(unittest.TestCase):
    def make_gateway(self, core: object = _UNSET) -> ArboristGateway:
        return make_gateway(core)

    def make_live_gateway(self) -> ArboristGateway:
        return ArboristGateway()

    def request(
        self,
        method: str,
        params: object | None = None,
        *,
        request_id: object = 1,
        jsonrpc: str = "2.0",
    ) -> dict[str, object]:
        return make_request(method, params, request_id=request_id, jsonrpc=jsonrpc)

    def call_gateway(
        self,
        gateway: ArboristGateway,
        method: str,
        params: object | None = None,
        *,
        request_id: object = 1,
        jsonrpc: str = "2.0",
    ) -> dict[str, object]:
        return gateway.handle_request(
            self.request(method, params, request_id=request_id, jsonrpc=jsonrpc)
        )

    def assert_jsonrpc_ok(
        self,
        response: dict[str, object],
        *,
        request_id: object,
    ) -> object:
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], request_id)
        self.assertNotIn("error", response)
        return response["result"]

    def assert_jsonrpc_error(
        self,
        response: dict[str, object],
        *,
        request_id: object,
        code: int,
        contains: str | None = None,
    ) -> dict[str, object]:
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], request_id)
        error = response["error"]
        assert isinstance(error, dict)
        self.assertEqual(error["code"], code)
        if contains is not None:
            message = error["message"]
            assert isinstance(message, str)
            self.assertIn(contains, message)
        return error

    @contextmanager
    def temp_workspace(self, files: dict[str, str] | None = None) -> Iterator[Path]:
        with temp_workspace(files) as workspace:
            yield workspace
