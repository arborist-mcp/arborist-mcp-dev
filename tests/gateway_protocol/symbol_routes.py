from __future__ import annotations

import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import arborist_mcp
from arborist_mcp import gateway as gateway_module
from arborist_mcp import _version as version_module
from arborist_mcp.gateway import ArboristGateway


class GatewaySymbolRouteTests(unittest.TestCase):
    def test_trace_context_returns_trace_error_when_patch_gate_rejects(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            workspace = Path(temp_dir)
            caller = workspace.joinpath("caller.py")
            caller.write_text(
                "def orchestrate(value: int) -> int:\n    return value + 1\n",
                encoding="utf-8",
            )

            gateway = ArboristGateway()
            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 41,
                    "method": "arborist/validate_patch_with_trace_context",
                    "params": {
                        "workspace_root": str(workspace),
                        "file_path": str(caller),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return missing_helper(value)\n"
                        ),
                        "direction": "both",
                    },
                }
            )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 41)
        self.assertNotIn("error", response)
        self.assertFalse(response["result"]["patch"]["applied"])
        self.assertEqual(
            response["result"]["trace_target"],
            response["result"]["patch"]["resolved_symbol_id"],
        )
        self.assertIsNone(response["result"]["trace"])
        self.assertIsNone(response["result"]["trace_validation"])
        self.assertEqual(
            response["result"]["trace_error"],
            "trace skipped because patch validation rejected the patch",
        )

    def test_search_symbols_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def search_symbols_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"query":"helper","indexed_files":2,"total_matches":1,'
                    '"truncated":false,"matches":['
                    '{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}'
                    '],"match_details":['
                    '{"symbol_id":"helper","score":1000,"matched_fields":["base_name","semantic_path"]}'
                    "]}"
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 57,
                "method": "arborist/search_symbols",
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 57)
        self.assertEqual(response["result"]["query"], "helper")
        self.assertEqual(response["result"]["total_matches"], 1)
        self.assertFalse(response["result"]["truncated"])
        self.assertEqual(response["result"]["matches"][0]["semantic_path"], "helper")
        self.assertEqual(response["result"]["match_details"][0]["symbol_id"], "helper")
        self.assertEqual(response["result"]["match_details"][0]["score"], 1000)
        self.assertEqual(
            gateway._core.calls,
            [(".", "helper", 5, "symbols.db", "graph", "function_definition")],
        )

    def test_search_symbols_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def search_symbols_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"search":{"query":"helper","indexed_files":2,"total_matches":1,'
                    '"truncated":false,"matches":['
                    '{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}'
                    '],"match_details":['
                    '{"symbol_id":"helper","score":1000,"matched_fields":["base_name","semantic_path"]}'
                    ']},'
                    '"reads":['
                    '{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}}]}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 77,
                "method": "arborist/search_symbols_context",
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 77)
        self.assertEqual(response["result"]["search"]["query"], "helper")
        self.assertEqual(response["result"]["search"]["total_matches"], 1)
        self.assertEqual(response["result"]["reads"][0]["symbol"]["semantic_path"], "helper")
        self.assertIn("def helper()", response["result"]["reads"][0]["source"])
        self.assertEqual(
            gateway._core.calls,
            [(".", "helper", 5, "symbols.db", "graph", "function_definition")],
        )

    def test_search_symbols_neighborhood_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def search_symbols_neighborhood_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"search":{"query":"helper","indexed_files":2,"total_matches":1,'
                    '"truncated":false,"matches":['
                    '{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}'
                    '],"match_details":['
                    '{"symbol_id":"helper","score":1000,"matched_fields":["base_name","semantic_path"]}'
                    ']},'
                    '"contexts":['
                    '{"neighborhood":{"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"helper|sample.py|function_definition|trace_root|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":[],"references":["orchestrate"]},'
                    '"direction":"callers","max_depth":2,"max_nodes":10,"truncated":false,'
                    '"indexed_files":2,"nodes":['
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":0}'
                    '],"edges":[]},'
                    '"reads":['
                    '{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}}]}]}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 78,
                "method": "arborist/search_symbols_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 78)
        self.assertEqual(response["result"]["search"]["query"], "helper")
        self.assertEqual(response["result"]["search"]["total_matches"], 1)
        self.assertEqual(
            response["result"]["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
            "helper",
        )
        self.assertIn(
            "def helper()",
            response["result"]["contexts"][0]["reads"][0]["source"],
        )
        self.assertEqual(
            gateway._core.calls,
            [(".", "helper", 5, "callers", 2, 10, "symbols.db", "graph", "function_definition")],
        )

    def test_search_symbols_discovery_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def search_symbols_discovery_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"search":{"query":"helper","indexed_files":2,"total_matches":1,'
                    '"truncated":false,"matches":['
                    '{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}'
                    '],"match_details":['
                    '{"symbol_id":"helper","score":1000,"matched_fields":["base_name","semantic_path"]}'
                    ']},'
                    '"reads":['
                    '{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}}],'
                    '"contexts":['
                    '{"neighborhood":{"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"helper|sample.py|function_definition|trace_root|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":[],"references":["orchestrate"]},'
                    '"direction":"callers","max_depth":2,"max_nodes":10,"truncated":false,'
                    '"indexed_files":2,"nodes":['
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":0}'
                    '],"edges":[]},'
                    '"reads":['
                    '{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}}]}]}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 86,
                "method": "arborist/search_symbols_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 86)
        self.assertEqual(response["result"]["search"]["query"], "helper")
        self.assertEqual(response["result"]["reads"][0]["symbol"]["semantic_path"], "helper")
        self.assertEqual(
            response["result"]["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
            "helper",
        )
        self.assertEqual(
            gateway._core.calls,
            [(".", "helper", 5, "callers", 2, 10, "symbols.db", "graph", "function_definition")],
        )

    def test_list_symbols_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def list_symbols_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"indexed_files":2,"total_symbols":1,"truncated":false,"symbols":['
                    '{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}'
                    "]}"
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 60,
                "method": "arborist/list_symbols",
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 60)
        self.assertEqual(response["result"]["total_symbols"], 1)
        self.assertFalse(response["result"]["truncated"])
        self.assertEqual(response["result"]["symbols"][0]["semantic_path"], "helper")
        self.assertEqual(
            gateway._core.calls,
            [(".", 25, "symbols.db", "graph", "function_definition")],
        )

    def test_list_symbols_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def list_symbols_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"list":{"indexed_files":2,"total_symbols":1,"truncated":false,"symbols":['
                    '{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}'
                    ']},'
                    '"reads":['
                    '{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}}]}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 61,
                "method": "arborist/list_symbols_context",
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 61)
        self.assertEqual(response["result"]["list"]["total_symbols"], 1)
        self.assertFalse(response["result"]["list"]["truncated"])
        self.assertEqual(response["result"]["list"]["symbols"][0]["semantic_path"], "helper")
        self.assertEqual(response["result"]["reads"][0]["symbol"]["semantic_path"], "helper")
        self.assertIn("def helper()", response["result"]["reads"][0]["source"])
        self.assertEqual(
            gateway._core.calls,
            [(".", 25, "symbols.db", "graph", "function_definition")],
        )

    def test_list_symbols_neighborhood_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def list_symbols_neighborhood_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"list":{"indexed_files":2,"total_symbols":1,"truncated":false,"symbols":['
                    '{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}'
                    ']},'
                    '"contexts":['
                    '{"neighborhood":{"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"helper|sample.py|function_definition|trace_root|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":[],"references":["orchestrate"]},'
                    '"direction":"callers","max_depth":2,"max_nodes":10,"truncated":false,'
                    '"indexed_files":2,"nodes":['
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":0}'
                    '],"edges":[]},'
                    '"reads":['
                    '{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}}]}]}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 81,
                "method": "arborist/list_symbols_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 81)
        self.assertEqual(response["result"]["list"]["total_symbols"], 1)
        self.assertEqual(
            response["result"]["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
            "helper",
        )
        self.assertIn(
            "def helper()",
            response["result"]["contexts"][0]["reads"][0]["source"],
        )
        self.assertEqual(
            gateway._core.calls,
            [(".", 25, "callers", 2, 10, "symbols.db", "graph", "function_definition")],
        )

    def test_list_symbols_discovery_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def list_symbols_discovery_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"list":{"indexed_files":2,"total_symbols":1,"truncated":false,"symbols":['
                    '{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}]},'
                    '"reads":['
                    '{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}}],'
                    '"contexts":['
                    '{"neighborhood":{"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"helper|sample.py|function_definition|trace_root|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":[],"references":["orchestrate"]},'
                    '"direction":"callers","max_depth":2,"max_nodes":10,"truncated":false,'
                    '"indexed_files":2,"nodes":['
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":0}'
                    '],"edges":[]},'
                    '"reads":['
                    '{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}}]}]}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 87,
                "method": "arborist/list_symbols_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 87)
        self.assertEqual(response["result"]["list"]["total_symbols"], 1)
        self.assertEqual(response["result"]["reads"][0]["symbol"]["semantic_path"], "helper")
        self.assertEqual(
            response["result"]["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
            "helper",
        )
        self.assertEqual(
            gateway._core.calls,
            [(".", 25, "callers", 2, 10, "symbols.db", "graph", "function_definition")],
        )

    def test_read_symbol_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def read_symbol_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"indexed_files":2,"symbol":{'
                    '"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},'
                    '"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},'
                    '"end_point":{"row":1,"column":12}}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 61,
                "method": "arborist/read_symbol",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "index_db_path": "symbols.db",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 61)
        self.assertEqual(response["result"]["symbol"]["semantic_path"], "helper")
        self.assertIn("def helper()", response["result"]["source"])
        self.assertEqual(gateway._core.calls, [(".", "helper", "symbols.db")])

    def test_read_symbol_at_position_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def read_symbol_at_position_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"indexed_files":2,"symbol":{'
                    '"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"graph_b.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|graph_b.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},'
                    '"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},'
                    '"end_point":{"row":1,"column":12}}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 62,
                "method": "arborist/read_symbol_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "index_db_path": "symbols.db",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 62)
        self.assertEqual(response["result"]["symbol"]["semantic_path"], "helper")
        self.assertIn("def helper()", response["result"]["source"])
        self.assertEqual(gateway._core.calls, [(".", "graph_b.py", 0, 5, "symbols.db")])

    def test_trace_symbol_neighborhood_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def trace_symbol_neighborhood_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"helper|sample.py|function_definition|trace_root|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":[],"references":["orchestrate"]},'
                    '"direction":"callers","max_depth":2,"max_nodes":10,"truncated":false,'
                    '"indexed_files":2,"nodes":['
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":0},'
                    '{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_caller|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":1}'
                    '],"edges":[{"from_symbol_id":"orchestrate","to_symbol_id":"helper"}]}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 66,
                "method": "arborist/trace_symbol_neighborhood",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 66)
        self.assertEqual(response["result"]["symbol"]["semantic_path"], "helper")
        self.assertEqual(response["result"]["direction"], "callers")
        self.assertEqual(response["result"]["nodes"][1]["symbol"]["semantic_path"], "orchestrate")
        self.assertEqual(response["result"]["edges"][0]["to_symbol_id"], "helper")
        self.assertEqual(
            gateway._core.calls,
            [(".", "helper", "callers", 2, 10, "symbols.db")],
        )

    def test_read_symbol_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def read_symbol_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"read":{"indexed_files":2,"symbol":{'
                    '"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},'
                    '"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},'
                    '"end_point":{"row":1,"column":12}},'
                    '"trace":{"symbol":{'
                    '"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"helper|sample.py|function_definition|trace_root|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":[],"references":["orchestrate"]},'
                    '"callers":[{'
                    '"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_caller|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}],'
                    '"callees":[],"evidence_keys":{'
                    '"symbol":"helper|sample.py|function_definition|trace_root|0..10|",'
                    '"callers":["orchestrate|caller.py|function_definition|trace_caller|0..20|"],'
                    '"callees":[]},'
                    '"indexed_files":2}}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 63,
                "method": "arborist/read_symbol_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 63)
        self.assertEqual(response["result"]["read"]["symbol"]["semantic_path"], "helper")
        self.assertIn("def helper()", response["result"]["read"]["source"])
        self.assertEqual(response["result"]["trace"]["symbol"]["semantic_path"], "helper")
        self.assertEqual(response["result"]["trace"]["callers"][0]["semantic_path"], "orchestrate")
        self.assertEqual(
            gateway._core.calls,
            [(".", "helper", "callers", "symbols.db")],
        )

    def test_read_symbol_context_at_position_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def read_symbol_context_at_position_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"read":{"indexed_files":2,"symbol":{'
                    '"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"graph_b.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|graph_b.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},'
                    '"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},'
                    '"end_point":{"row":1,"column":12}},'
                    '"trace":{"symbol":{'
                    '"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"graph_b.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"helper|graph_b.py|function_definition|trace_root|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":[],"references":["orchestrate"]},'
                    '"callers":[{"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"graph_a.py","node_kind":"function_definition",'
                    '"origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|graph_a.py|function_definition|trace_caller|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}],"callees":[],"evidence_keys":{'
                    '"symbol":"helper|graph_b.py|function_definition|trace_root|0..10|",'
                    '"callers":["orchestrate|graph_a.py|function_definition|trace_caller|0..20|"],'
                    '"callees":[]},"indexed_files":2}}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 64,
                "method": "arborist/read_symbol_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 64)
        self.assertEqual(response["result"]["read"]["symbol"]["semantic_path"], "helper")
        self.assertEqual(response["result"]["trace"]["callers"][0]["semantic_path"], "orchestrate")
        self.assertEqual(
            gateway._core.calls,
            [(".", "graph_b.py", 0, 5, "callers", "symbols.db")],
        )

    def test_read_symbol_neighborhood_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def read_symbol_neighborhood_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"neighborhood":{"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"helper|sample.py|function_definition|trace_root|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":[],"references":["orchestrate"]},'
                    '"direction":"callers","max_depth":2,"max_nodes":10,"truncated":false,'
                    '"indexed_files":2,"nodes":['
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":0},'
                    '{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_caller|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":1}'
                    '],"edges":[{"from_symbol_id":"orchestrate","to_symbol_id":"helper"}]},'
                    '"reads":['
                    '{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}},'
                    '{"indexed_files":2,"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate",'
                    '"scope_path":null,"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_caller|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def orchestrate() -> int:\\n    return helper()\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":18}}]}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 72,
                "method": "arborist/read_symbol_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 72)
        self.assertEqual(
            response["result"]["neighborhood"]["symbol"]["semantic_path"],
            "helper",
        )
        self.assertEqual(len(response["result"]["reads"]), 2)
        self.assertEqual(response["result"]["reads"][1]["symbol"]["semantic_path"], "orchestrate")
        self.assertIn("def helper()", response["result"]["reads"][0]["source"])
        self.assertEqual(
            gateway._core.calls,
            [(".", "helper", "callers", 2, 10, "symbols.db")],
        )

    def test_read_symbol_neighborhood_context_at_position_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def read_symbol_neighborhood_context_at_position_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"neighborhood":{"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"graph_b.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"helper|graph_b.py|function_definition|trace_root|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":[],"references":["orchestrate"]},'
                    '"direction":"callers","max_depth":2,"max_nodes":10,"truncated":false,'
                    '"indexed_files":2,"nodes":['
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"graph_b.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|graph_b.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":0},'
                    '{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"graph_a.py","node_kind":"function_definition",'
                    '"origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|graph_a.py|function_definition|trace_caller|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":1}],'
                    '"edges":[{"from_symbol_id":"orchestrate","to_symbol_id":"helper"}]},'
                    '"reads":['
                    '{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"graph_b.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|graph_b.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}},'
                    '{"indexed_files":2,"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate",'
                    '"scope_path":null,"file_path":"graph_a.py","node_kind":"function_definition",'
                    '"origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|graph_a.py|function_definition|trace_caller|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def orchestrate() -> int:\\n    return helper()\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":18}}]}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 73,
                "method": "arborist/read_symbol_neighborhood_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 73)
        self.assertEqual(
            response["result"]["neighborhood"]["symbol"]["semantic_path"],
            "helper",
        )
        self.assertEqual(response["result"]["reads"][1]["symbol"]["semantic_path"], "orchestrate")
        self.assertEqual(
            gateway._core.calls,
            [(".", "graph_b.py", 0, 5, "callers", 2, 10, "symbols.db")],
        )

    def test_read_symbol_discovery_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def read_symbol_discovery_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"read":{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition","origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|","byte_range":[0,10],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null},'
                    '"source":"def helper() -> int:\\n    return 1\\n","start_point":{"row":0,"column":0},'
                    '"end_point":{"row":1,"column":12}},'
                    '"trace":{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition","origin_type":"trace_root",'
                    '"evidence_key":"helper|sample.py|function_definition|trace_root|0..10|","byte_range":[0,10],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null,'
                    '"dependencies":[],"references":["orchestrate"]},'
                    '"callers":[{"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"caller.py","node_kind":"function_definition","origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_caller|0..20|","byte_range":[0,20],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null}],'
                    '"callees":[],"evidence_keys":{"symbol":"helper|sample.py|function_definition|trace_root|0..10|",'
                    '"callers":["orchestrate|caller.py|function_definition|trace_caller|0..20|"],"callees":[]},'
                    '"indexed_files":2},'
                    '"neighborhood_context":{"neighborhood":{"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"sample.py","node_kind":"function_definition","origin_type":"trace_root",'
                    '"evidence_key":"helper|sample.py|function_definition|trace_root|0..10|","byte_range":[0,10],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null,"dependencies":[],'
                    '"references":["orchestrate"]},"direction":"callers","max_depth":2,"max_nodes":10,'
                    '"truncated":false,"indexed_files":2,"nodes":['
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,"file_path":"sample.py",'
                    '"node_kind":"function_definition","origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|","byte_range":[0,10],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null},"depth":0},'
                    '{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"caller.py","node_kind":"function_definition","origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_caller|0..20|","byte_range":[0,20],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null},"depth":1}],'
                    '"edges":[{"from_symbol_id":"orchestrate","to_symbol_id":"helper"}]},'
                    '"reads":['
                    '{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"sample.py","node_kind":"function_definition","origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|sample.py|function_definition|workspace_symbol|0..10|","byte_range":[0,10],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null},'
                    '"source":"def helper() -> int:\\n    return 1\\n","start_point":{"row":0,"column":0},'
                    '"end_point":{"row":1,"column":12}},'
                    '{"indexed_files":2,"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"caller.py","node_kind":"function_definition","origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_caller|0..20|","byte_range":[0,20],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null},'
                    '"source":"def orchestrate() -> int:\\n    return helper()\\n","start_point":{"row":0,"column":0},'
                    '"end_point":{"row":1,"column":18}}]}}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 74,
                "method": "arborist/read_symbol_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 74)
        self.assertEqual(response["result"]["read"]["symbol"]["semantic_path"], "helper")
        self.assertEqual(response["result"]["trace"]["symbol"]["semantic_path"], "helper")
        self.assertEqual(
            response["result"]["neighborhood_context"]["reads"][1]["symbol"]["semantic_path"],
            "orchestrate",
        )
        self.assertEqual(
            gateway._core.calls,
            [(".", "helper", "callers", 2, 10, "symbols.db")],
        )

    def test_read_symbol_discovery_context_at_position_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def read_symbol_discovery_context_at_position_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"read":{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"graph_b.py","node_kind":"function_definition","origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|graph_b.py|function_definition|workspace_symbol|0..10|","byte_range":[0,10],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null},'
                    '"source":"def helper() -> int:\\n    return 1\\n","start_point":{"row":0,"column":0},'
                    '"end_point":{"row":1,"column":12}},'
                    '"trace":{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"graph_b.py","node_kind":"function_definition","origin_type":"trace_root",'
                    '"evidence_key":"helper|graph_b.py|function_definition|trace_root|0..10|","byte_range":[0,10],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null,'
                    '"dependencies":[],"references":["orchestrate"]},"callers":[{"symbol_id":"orchestrate",'
                    '"semantic_path":"orchestrate","scope_path":null,"file_path":"graph_a.py",'
                    '"node_kind":"function_definition","origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|graph_a.py|function_definition|trace_caller|0..20|","byte_range":[0,20],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null}],"callees":[],'
                    '"evidence_keys":{"symbol":"helper|graph_b.py|function_definition|trace_root|0..10|",'
                    '"callers":["orchestrate|graph_a.py|function_definition|trace_caller|0..20|"],"callees":[]},'
                    '"indexed_files":2},"neighborhood_context":{"neighborhood":{"symbol":{"symbol_id":"helper",'
                    '"semantic_path":"helper","scope_path":null,"file_path":"graph_b.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root","evidence_key":"helper|graph_b.py|function_definition|trace_root|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,"docstring":null,'
                    '"dependencies":[],"references":["orchestrate"]},"direction":"callers","max_depth":2,'
                    '"max_nodes":10,"truncated":false,"indexed_files":2,"nodes":[{"symbol":{"symbol_id":"helper",'
                    '"semantic_path":"helper","scope_path":null,"file_path":"graph_b.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol","evidence_key":"helper|graph_b.py|function_definition|workspace_symbol|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,"docstring":null},"depth":0},'
                    '{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"graph_a.py","node_kind":"function_definition","origin_type":"trace_caller",'
                    '"evidence_key":"orchestrate|graph_a.py|function_definition|trace_caller|0..20|","byte_range":[0,20],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null},"depth":1}],'
                    '"edges":[{"from_symbol_id":"orchestrate","to_symbol_id":"helper"}]},'
                    '"reads":[{"indexed_files":2,"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"graph_b.py","node_kind":"function_definition","origin_type":"workspace_symbol",'
                    '"evidence_key":"helper|graph_b.py|function_definition|workspace_symbol|0..10|","byte_range":[0,10],'
                    '"signature":null,"parameters":[],"return_type":null,"docstring":null},'
                    '"source":"def helper() -> int:\\n    return 1\\n","start_point":{"row":0,"column":0},'
                    '"end_point":{"row":1,"column":12}},{"indexed_files":2,"symbol":{"symbol_id":"orchestrate",'
                    '"semantic_path":"orchestrate","scope_path":null,"file_path":"graph_a.py","node_kind":"function_definition",'
                    '"origin_type":"trace_caller","evidence_key":"orchestrate|graph_a.py|function_definition|trace_caller|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,"docstring":null},'
                    '"source":"def orchestrate() -> int:\\n    return helper()\\n","start_point":{"row":0,"column":0},'
                    '"end_point":{"row":1,"column":18}}]}}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 75,
                "method": "arborist/read_symbol_discovery_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 75)
        self.assertEqual(response["result"]["read"]["symbol"]["semantic_path"], "helper")
        self.assertEqual(response["result"]["trace"]["symbol"]["semantic_path"], "helper")
        self.assertEqual(
            response["result"]["neighborhood_context"]["reads"][1]["symbol"]["semantic_path"],
            "orchestrate",
        )
        self.assertEqual(
            gateway._core.calls,
            [(".", "graph_b.py", 0, 5, "callers", 2, 10, "symbols.db")],
        )

    def test_patch_ast_node_at_position_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def patch_ast_node_at_position_json(self, *args: object) -> str:
                self.calls.append(args)
                return "{}"

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 96,
                "method": "arborist/patch_ast_node_at_position",
                "params": {
                    "file_path": "sample.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": "def helper() -> int:\n    return 2\n",
                    "source": "def helper() -> int:\n    return 1\n",
                    "bypass_reason": "known-safe",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 96)
        self.assertEqual(response["result"], {})
        self.assertEqual(
            gateway._core.calls,
            [(
                "sample.py",
                3,
                1,
                "def helper() -> int:\n    return 2\n",
                "def helper() -> int:\n    return 1\n",
                "known-safe",
            )],
        )

    def test_patch_virtual_ast_node_at_position_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def patch_virtual_ast_node_at_position_json(self, *args: object) -> str:
                self.calls.append(args)
                return "{}"

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 97,
                "method": "arborist/patch_virtual_ast_node_at_position",
                "params": {
                    "file_path": "sample.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": "def helper() -> int:\n    return 2\n",
                    "bypass_reason": "known-safe",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 97)
        self.assertEqual(response["result"], {})
        self.assertEqual(
            gateway._core.calls,
            [(
                "sample.py",
                3,
                1,
                "def helper() -> int:\n    return 2\n",
                "known-safe",
            )],
        )

    def test_graph_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def validate_patch_with_graph_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"patch":{"file":"caller.py","target_path":"orchestrate",'
                    '"resolved_path":"orchestrate","resolved_symbol_id":"orchestrate",'
                    '"applied":true,"bypass_applied":false,'
                    '"updated_source":"def orchestrate(value: int) -> int:\\n    return helper(value)\\n",'
                    '"validation":{"syntax_errors":[],"unresolved_identifiers":[],'
                    '"resolved_identifiers":[],"ambiguous_identifiers":[],"binding_decisions":[],'
                    '"commit_gate":{"status":"allowed","allowed":true,"reason":"ok",'
                    '"bypass_reason":null,"blocking_decisions":[],"evidence_invariants":[],'
                    '"syntax_error_count":0}}},'
                    '"trace_target":"orchestrate",'
                    '"trace":{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate",'
                    '"scope_path":null,"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_root|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":["helper"],"references":["entrypoint"]},'
                    '"callers":[{"symbol_id":"entrypoint","semantic_path":"entrypoint","scope_path":null,'
                    '"file_path":"entry.py","node_kind":"function_definition",'
                    '"origin_type":"trace_caller",'
                    '"evidence_key":"entrypoint|entry.py|function_definition|trace_caller|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}],"callees":[{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"helper.py","node_kind":"function_definition",'
                    '"origin_type":"trace_callee",'
                    '"evidence_key":"helper|helper.py|function_definition|trace_callee|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}],"evidence_keys":{"symbol":"orchestrate|caller.py|function_definition|trace_root|0..20|",'
                    '"callers":["entrypoint|entry.py|function_definition|trace_caller|0..20|"],'
                    '"callees":["helper|helper.py|function_definition|trace_callee|0..10|"]},'
                    '"indexed_files":3},'
                    '"neighborhood":{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate",'
                    '"scope_path":null,"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_root|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":["helper"],"references":["entrypoint"]},'
                    '"direction":"both","max_depth":2,"max_nodes":10,"truncated":false,'
                    '"indexed_files":3,"nodes":['
                    '{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|workspace_symbol|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":0},'
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"helper.py","node_kind":"function_definition",'
                    '"origin_type":"trace_callee",'
                    '"evidence_key":"helper|helper.py|function_definition|trace_callee|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":1}],'
                    '"edges":[{"from_symbol_id":"orchestrate","to_symbol_id":"helper"}]},'
                    '"trace_validation":{"allowed":true,"status":"allowed","reason":"ok",'
                    '"patch_gate_status":"allowed","replay_status":"matched",'
                    '"replay":{"consistent":true,"matched_items":0,"blocked_items":0,"items":[]}},'
                    '"trace_error":null}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 70,
                "method": "arborist/validate_patch_with_graph_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 70)
        self.assertTrue(response["result"]["patch"]["applied"])
        self.assertEqual(response["result"]["trace"]["symbol"]["semantic_path"], "orchestrate")
        self.assertEqual(
            response["result"]["neighborhood"]["nodes"][1]["symbol"]["semantic_path"],
            "helper",
        )
        self.assertTrue(response["result"]["trace_validation"]["allowed"])
        self.assertEqual(
            gateway._core.calls,
            [(
                ".",
                "caller.py",
                "orchestrate",
                "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                None,
                None,
                "both",
                2,
                10,
            )],
        )

    def test_trace_context_at_position_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def validate_patch_with_trace_context_at_position_json(
                self, *args: object
            ) -> str:
                self.calls.append(args)
                return "{}"

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 98,
                "method": "arborist/validate_patch_with_trace_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                    "source": "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "bypass_reason": "known-safe",
                    "direction": "callers",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 98)
        self.assertEqual(response["result"], {})
        self.assertEqual(
            gateway._core.calls,
            [(
                ".",
                "caller.py",
                3,
                1,
                "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                "def orchestrate(value: int) -> int:\n    return value + 1\n",
                "known-safe",
                "callers",
            )],
        )

    def test_graph_context_at_position_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def validate_patch_with_graph_context_at_position_json(
                self, *args: object
            ) -> str:
                self.calls.append(args)
                return "{}"

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 99,
                "method": "arborist/validate_patch_with_graph_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                    "source": "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "bypass_reason": "known-safe",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 99)
        self.assertEqual(response["result"], {})
        self.assertEqual(
            gateway._core.calls,
            [(
                ".",
                "caller.py",
                3,
                1,
                "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                "def orchestrate(value: int) -> int:\n    return value + 1\n",
                "known-safe",
                "callers",
                2,
                10,
            )],
        )

    def test_neighborhood_context_at_position_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def validate_patch_with_neighborhood_context_at_position_json(
                self, *args: object
            ) -> str:
                self.calls.append(args)
                return "{}"

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 100,
                "method": "arborist/validate_patch_with_neighborhood_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                    "source": "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "bypass_reason": "known-safe",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 100)
        self.assertEqual(response["result"], {})
        self.assertEqual(
            gateway._core.calls,
            [(
                ".",
                "caller.py",
                3,
                1,
                "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                "def orchestrate(value: int) -> int:\n    return value + 1\n",
                "known-safe",
                "callers",
                2,
                10,
            )],
        )

    def test_discovery_context_at_position_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def validate_patch_with_discovery_context_at_position_json(
                self, *args: object
            ) -> str:
                self.calls.append(args)
                return "{}"

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 101,
                "method": "arborist/validate_patch_with_discovery_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                    "source": "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "bypass_reason": "known-safe",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 101)
        self.assertEqual(response["result"], {})
        self.assertEqual(
            gateway._core.calls,
            [(
                ".",
                "caller.py",
                3,
                1,
                "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                "def orchestrate(value: int) -> int:\n    return value + 1\n",
                "known-safe",
                "callers",
                2,
                10,
            )],
        )

    def test_neighborhood_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def validate_patch_with_neighborhood_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"patch":{"file":"caller.py","target_path":"orchestrate",'
                    '"resolved_path":"orchestrate","resolved_symbol_id":"orchestrate",'
                    '"applied":true,"bypass_applied":false,'
                    '"updated_source":"def orchestrate(value: int) -> int:\\n    return helper(value)\\n",'
                    '"validation":{"syntax_errors":[],"unresolved_identifiers":[],'
                    '"resolved_identifiers":[],"ambiguous_identifiers":[],"binding_decisions":[],'
                    '"commit_gate":{"status":"allowed","allowed":true,"reason":"ok",'
                    '"bypass_reason":null,"blocking_decisions":[],"evidence_invariants":[],'
                    '"syntax_error_count":0}}},'
                    '"trace_target":"orchestrate",'
                    '"trace":{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate",'
                    '"scope_path":null,"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_root|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":["helper"],"references":["entrypoint"]},'
                    '"callers":[{"symbol_id":"entrypoint","semantic_path":"entrypoint","scope_path":null,'
                    '"file_path":"entry.py","node_kind":"function_definition",'
                    '"origin_type":"trace_caller",'
                    '"evidence_key":"entrypoint|entry.py|function_definition|trace_caller|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}],"callees":[{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"helper.py","node_kind":"function_definition",'
                    '"origin_type":"trace_callee",'
                    '"evidence_key":"helper|helper.py|function_definition|trace_callee|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}],"evidence_keys":{"symbol":"orchestrate|caller.py|function_definition|trace_root|0..20|",'
                    '"callers":["entrypoint|entry.py|function_definition|trace_caller|0..20|"],'
                    '"callees":["helper|helper.py|function_definition|trace_callee|0..10|"]},'
                    '"indexed_files":3},'
                    '"neighborhood_context":{"neighborhood":{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate",'
                    '"scope_path":null,"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_root|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":["helper"],"references":["entrypoint"]},'
                    '"direction":"both","max_depth":2,"max_nodes":10,"truncated":false,'
                    '"indexed_files":3,"nodes":['
                    '{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|workspace_symbol|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":0},'
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"helper.py","node_kind":"function_definition",'
                    '"origin_type":"trace_callee",'
                    '"evidence_key":"helper|helper.py|function_definition|trace_callee|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":1}],"edges":[{"from_symbol_id":"orchestrate","to_symbol_id":"helper"}]},'
                    '"reads":['
                    '{"indexed_files":3,"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate",'
                    '"scope_path":null,"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|workspace_symbol|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def orchestrate(value: int) -> int:\\n    return helper(value)\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":24}},'
                    '{"indexed_files":3,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"helper.py","node_kind":"function_definition",'
                    '"origin_type":"trace_callee",'
                    '"evidence_key":"helper|helper.py|function_definition|trace_callee|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}}]},'
                    '"trace_validation":{"allowed":true,"status":"allowed","reason":"ok",'
                    '"patch_gate_status":"allowed","replay_status":"matched",'
                    '"replay":{"consistent":true,"matched_items":0,"blocked_items":0,"items":[]}},'
                    '"trace_error":null}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 75,
                "method": "arborist/validate_patch_with_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 75)
        self.assertTrue(response["result"]["patch"]["applied"])
        self.assertEqual(
            response["result"]["trace"]["symbol"]["semantic_path"],
            "orchestrate",
        )
        self.assertEqual(
            response["result"]["neighborhood_context"]["neighborhood"]["nodes"][1]["symbol"]["semantic_path"],
            "helper",
        )
        self.assertEqual(
            response["result"]["neighborhood_context"]["reads"][1]["symbol"]["semantic_path"],
            "helper",
        )
        self.assertTrue(response["result"]["trace_validation"]["allowed"])
        self.assertEqual(
            gateway._core.calls,
            [(
                ".",
                "caller.py",
                "orchestrate",
                "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                None,
                None,
                "both",
                2,
                10,
            )],
        )

    def test_discovery_context_routes_params_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.calls: list[tuple[object, ...]] = []

            def validate_patch_with_discovery_context_json(self, *args: object) -> str:
                self.calls.append(args)
                return (
                    '{"patch":{"file":"caller.py","target_path":"orchestrate",'
                    '"resolved_path":"orchestrate","resolved_symbol_id":"orchestrate",'
                    '"applied":true,"bypass_applied":false,'
                    '"updated_source":"def orchestrate(value: int) -> int:\\n    return helper(value)\\n",'
                    '"validation":{"syntax_errors":[],"unresolved_identifiers":[],'
                    '"resolved_identifiers":[],"ambiguous_identifiers":[],"binding_decisions":[],'
                    '"commit_gate":{"status":"allowed","allowed":true,"reason":"ok",'
                    '"bypass_reason":null,"blocking_decisions":[],"evidence_invariants":[],'
                    '"syntax_error_count":0}}},'
                    '"trace_target":"orchestrate",'
                    '"trace":{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate",'
                    '"scope_path":null,"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_root|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":["helper"],"references":["entrypoint"]},'
                    '"callers":[{"symbol_id":"entrypoint","semantic_path":"entrypoint","scope_path":null,'
                    '"file_path":"entry.py","node_kind":"function_definition",'
                    '"origin_type":"trace_caller",'
                    '"evidence_key":"entrypoint|entry.py|function_definition|trace_caller|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}],"callees":[{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"helper.py","node_kind":"function_definition",'
                    '"origin_type":"trace_callee",'
                    '"evidence_key":"helper|helper.py|function_definition|trace_callee|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null}],"evidence_keys":{"symbol":"orchestrate|caller.py|function_definition|trace_root|0..20|",'
                    '"callers":["entrypoint|entry.py|function_definition|trace_caller|0..20|"],'
                    '"callees":["helper|helper.py|function_definition|trace_callee|0..10|"]},'
                    '"indexed_files":3},'
                    '"read":{"indexed_files":3,"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate",'
                    '"scope_path":null,"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|workspace_symbol|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def orchestrate(value: int) -> int:\\n    return helper(value)\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":24}},'
                    '"neighborhood_context":{"neighborhood":{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate",'
                    '"scope_path":null,"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"trace_root",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|trace_root|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null,"dependencies":["helper"],"references":["entrypoint"]},'
                    '"direction":"both","max_depth":2,"max_nodes":10,"truncated":false,'
                    '"indexed_files":3,"nodes":['
                    '{"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate","scope_path":null,'
                    '"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|workspace_symbol|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":0},'
                    '{"symbol":{"symbol_id":"helper","semantic_path":"helper","scope_path":null,'
                    '"file_path":"helper.py","node_kind":"function_definition",'
                    '"origin_type":"trace_callee",'
                    '"evidence_key":"helper|helper.py|function_definition|trace_callee|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"depth":1}],"edges":[{"from_symbol_id":"orchestrate","to_symbol_id":"helper"}]},'
                    '"reads":['
                    '{"indexed_files":3,"symbol":{"symbol_id":"orchestrate","semantic_path":"orchestrate",'
                    '"scope_path":null,"file_path":"caller.py","node_kind":"function_definition",'
                    '"origin_type":"workspace_symbol",'
                    '"evidence_key":"orchestrate|caller.py|function_definition|workspace_symbol|0..20|",'
                    '"byte_range":[0,20],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def orchestrate(value: int) -> int:\\n    return helper(value)\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":24}},'
                    '{"indexed_files":3,"symbol":{"symbol_id":"helper","semantic_path":"helper",'
                    '"scope_path":null,"file_path":"helper.py","node_kind":"function_definition",'
                    '"origin_type":"trace_callee",'
                    '"evidence_key":"helper|helper.py|function_definition|trace_callee|0..10|",'
                    '"byte_range":[0,10],"signature":null,"parameters":[],"return_type":null,'
                    '"docstring":null},"source":"def helper() -> int:\\n    return 1\\n",'
                    '"start_point":{"row":0,"column":0},"end_point":{"row":1,"column":12}}]},'
                    '"trace_validation":{"allowed":true,"status":"allowed","reason":"ok",'
                    '"patch_gate_status":"allowed","replay_status":"matched",'
                    '"replay":{"consistent":true,"matched_items":0,"blocked_items":0,"items":[]}},'
                    '"trace_error":null}'
                )

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 79,
                "method": "arborist/validate_patch_with_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 79)
        self.assertTrue(response["result"]["patch"]["applied"])
        self.assertEqual(response["result"]["trace"]["symbol"]["semantic_path"], "orchestrate")
        self.assertEqual(response["result"]["read"]["symbol"]["semantic_path"], "orchestrate")
        self.assertEqual(
            response["result"]["neighborhood_context"]["reads"][1]["symbol"]["semantic_path"],
            "helper",
        )
        self.assertTrue(response["result"]["trace_validation"]["allowed"])
        self.assertEqual(
            gateway._core.calls,
            [(
                ".",
                "caller.py",
                "orchestrate",
                "def orchestrate(value: int) -> int:\n    return helper(value)\n",
                None,
                None,
                "both",
                2,
                10,
            )],
        )

    def test_trace_context_returns_trace_error_when_patch_has_syntax_errors(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            workspace = Path(temp_dir)
            caller = workspace.joinpath("caller.py")
            caller.write_text(
                "def orchestrate(value: int) -> int:\n    return value + 1\n",
                encoding="utf-8",
            )

            gateway = ArboristGateway()
            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 42,
                    "method": "arborist/validate_patch_with_trace_context",
                    "params": {
                        "workspace_root": str(workspace),
                        "file_path": str(caller),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(\n"
                        ),
                        "direction": "both",
                    },
                }
            )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 42)
        self.assertNotIn("error", response)
        self.assertFalse(response["result"]["patch"]["applied"])
        self.assertEqual(
            response["result"]["trace_target"],
            response["result"]["patch"]["resolved_symbol_id"],
        )
        self.assertTrue(response["result"]["patch"]["validation"]["syntax_errors"])
        self.assertIsNone(response["result"]["trace"])
        self.assertIsNone(response["result"]["trace_validation"])
        self.assertEqual(
            response["result"]["trace_error"],
            "trace skipped because patch validation reported syntax errors",
        )

    def test_trace_context_accepts_unsaved_source(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            workspace = Path(temp_dir)
            nested = workspace.joinpath("child")
            helper = workspace.joinpath("helper.py")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            helper.write_text(
                "def helper(value: int) -> int:\n    return value + 1\n",
                encoding="utf-8",
            )
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            gateway = ArboristGateway()
            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 43,
                    "method": "arborist/validate_patch_with_trace_context",
                    "params": {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return value + 1\n"
                        ),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "direction": "both",
                    },
                }
            )

            self.assertEqual(response["jsonrpc"], "2.0")
            self.assertEqual(response["id"], 43)
            self.assertNotIn("error", response)
            self.assertFalse(caller.exists())
            self.assertTrue(response["result"]["patch"]["applied"])
            self.assertEqual(response["result"]["patch"]["file"], expected_file)
            self.assertEqual(
                response["result"]["trace_target"],
                response["result"]["patch"]["resolved_symbol_id"],
            )
            self.assertIsNone(response["result"]["trace_error"])
            self.assertTrue(response["result"]["trace_validation"]["allowed"])
            self.assertEqual(
                response["result"]["trace_validation"]["replay_status"],
                "matched",
            )
            self.assertEqual(
                response["result"]["trace"]["symbol"]["semantic_path"],
                "orchestrate",
            )
            self.assertEqual(response["result"]["trace"]["symbol"]["file_path"], expected_file)
            self.assertTrue(
                any(
                    symbol["semantic_path"] == "helper"
                    for symbol in response["result"]["trace"]["callees"]
                )
            )

    def test_graph_context_accepts_unsaved_source(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            workspace = Path(temp_dir)
            nested = workspace.joinpath("child")
            helper = workspace.joinpath("helper.py")
            caller = workspace.joinpath("caller.py")
            entry = workspace.joinpath("entry.py")
            nested.mkdir()
            helper.write_text(
                "def helper(value: int) -> int:\n    return value + 1\n",
                encoding="utf-8",
            )
            entry.write_text(
                "from caller import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
                encoding="utf-8",
            )
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            gateway = ArboristGateway()
            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 71,
                    "method": "arborist/validate_patch_with_graph_context",
                    "params": {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return value + 1\n"
                        ),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "direction": "both",
                        "max_depth": 2,
                        "max_nodes": 10,
                    },
                }
            )

            self.assertEqual(response["jsonrpc"], "2.0")
            self.assertEqual(response["id"], 71)
            self.assertNotIn("error", response)
            self.assertFalse(caller.exists())
            self.assertTrue(response["result"]["patch"]["applied"])
            self.assertEqual(response["result"]["patch"]["file"], expected_file)
            self.assertIsNone(response["result"]["trace_error"])
            self.assertTrue(response["result"]["trace_validation"]["allowed"])
            self.assertEqual(
                response["result"]["trace"]["symbol"]["semantic_path"],
                "orchestrate",
            )
            self.assertEqual(
                response["result"]["neighborhood"]["symbol"]["semantic_path"],
                "orchestrate",
            )
            self.assertTrue(
                any(
                    node["symbol"]["semantic_path"] == "helper"
                    for node in response["result"]["neighborhood"]["nodes"]
                )
            )

    def test_neighborhood_context_accepts_unsaved_source(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            workspace = Path(temp_dir)
            nested = workspace.joinpath("child")
            helper = workspace.joinpath("helper.py")
            caller = workspace.joinpath("caller.py")
            entry = workspace.joinpath("entry.py")
            nested.mkdir()
            helper.write_text(
                "def helper(value: int) -> int:\n    return value + 1\n",
                encoding="utf-8",
            )
            entry.write_text(
                "from caller import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
                encoding="utf-8",
            )
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            gateway = ArboristGateway()
            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 76,
                    "method": "arborist/validate_patch_with_neighborhood_context",
                    "params": {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return value + 1\n"
                        ),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "direction": "both",
                        "max_depth": 2,
                        "max_nodes": 10,
                    },
                }
            )

            self.assertEqual(response["jsonrpc"], "2.0")
            self.assertEqual(response["id"], 76)
            self.assertNotIn("error", response)
            self.assertFalse(caller.exists())
            self.assertTrue(response["result"]["patch"]["applied"])
            self.assertEqual(response["result"]["patch"]["file"], expected_file)
            self.assertIsNone(response["result"]["trace_error"])
            self.assertTrue(response["result"]["trace_validation"]["allowed"])
            self.assertEqual(
                response["result"]["trace"]["symbol"]["semantic_path"],
                "orchestrate",
            )
            self.assertEqual(
                response["result"]["neighborhood_context"]["neighborhood"]["symbol"]["semantic_path"],
                "orchestrate",
            )
            self.assertTrue(
                any(
                    read["symbol"]["semantic_path"] == "helper"
                    for read in response["result"]["neighborhood_context"]["reads"]
                )
            )

    def test_discovery_context_accepts_unsaved_source(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            workspace = Path(temp_dir)
            nested = workspace.joinpath("child")
            helper = workspace.joinpath("helper.py")
            caller = workspace.joinpath("caller.py")
            entry = workspace.joinpath("entry.py")
            nested.mkdir()
            helper.write_text(
                "def helper(value: int) -> int:\n    return value + 1\n",
                encoding="utf-8",
            )
            entry.write_text(
                "from caller import orchestrate\n\n\ndef entrypoint(value: int) -> int:\n    return orchestrate(value)\n",
                encoding="utf-8",
            )
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            gateway = ArboristGateway()
            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 80,
                    "method": "arborist/validate_patch_with_discovery_context",
                    "params": {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return value + 1\n"
                        ),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "direction": "both",
                        "max_depth": 2,
                        "max_nodes": 10,
                    },
                }
            )

            self.assertEqual(response["jsonrpc"], "2.0")
            self.assertEqual(response["id"], 80)
            self.assertNotIn("error", response)
            self.assertFalse(caller.exists())
            self.assertTrue(response["result"]["patch"]["applied"])
            self.assertEqual(response["result"]["patch"]["file"], expected_file)
            self.assertIsNone(response["result"]["trace_error"])
            self.assertTrue(response["result"]["trace_validation"]["allowed"])
            self.assertEqual(
                response["result"]["trace"]["symbol"]["semantic_path"],
                "orchestrate",
            )
            self.assertEqual(
                response["result"]["read"]["symbol"]["semantic_path"],
                "orchestrate",
            )
            self.assertTrue(
                any(
                    read["symbol"]["semantic_path"] == "helper"
                    for read in response["result"]["neighborhood_context"]["reads"]
                )
            )
