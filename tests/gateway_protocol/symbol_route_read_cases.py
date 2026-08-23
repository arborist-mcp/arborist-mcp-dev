from __future__ import annotations


class GatewaySymbolRouteReadTestsMixin:
    def test_direct_read_timeouts_reach_final_core_parameter(self) -> None:
        helper_read = self.helper_read()
        helper_read_graph = self.helper_read(file_path="graph_b.py")
        helper_trace = self.helper_trace_context()
        helper_trace_graph = self.helper_trace_context(file_path="graph_b.py")
        helper_context = self.helper_neighborhood_context()
        helper_context_graph = self.helper_neighborhood_context(file_path="graph_b.py")
        cases = (
            {
                "core_method": "read_symbol_json",
                "rpc_method": "arborist/read_symbol",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "payload": helper_read,
                "expected_call": (".", "helper", "symbols.db", None, None, 37),
            },
            {
                "core_method": "read_symbol_at_position_json",
                "rpc_method": "arborist/read_symbol_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "payload": helper_read_graph,
                "expected_call": (".", "graph_b.py", 0, 5, None, "symbols.db", 37),
            },
            {
                "core_method": "read_symbol_context_json",
                "rpc_method": "arborist/read_symbol_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "payload": {"read": helper_read, "trace": helper_trace},
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    "symbols.db",
                    None,
                    None,
                    37,
                ),
            },
            {
                "core_method": "read_symbol_context_at_position_json",
                "rpc_method": "arborist/read_symbol_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "payload": {"read": helper_read_graph, "trace": helper_trace_graph},
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    None,
                    "symbols.db",
                    37,
                ),
            },
            {
                "core_method": "read_symbol_discovery_context_json",
                "rpc_method": "arborist/read_symbol_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "payload": {
                    "read": helper_read,
                    "trace": helper_trace,
                    "neighborhood_context": helper_context,
                },
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    37,
                ),
            },
            {
                "core_method": "read_symbol_discovery_context_at_position_json",
                "rpc_method": "arborist/read_symbol_discovery_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "payload": {
                    "read": helper_read_graph,
                    "trace": helper_trace_graph,
                    "neighborhood_context": helper_context_graph,
                },
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    None,
                    "symbols.db",
                    37,
                ),
            },
        )

        for request_id, case in enumerate(cases, start=240):
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload=case["payload"],
                    request_id=request_id,
                    expected_call=case["expected_call"],
                    check_result=lambda result: self.assertIsInstance(result, dict),
                )

    def test_search_routes_params_to_core(self) -> None:
        helper_read = self.helper_read()
        helper_context = self.helper_neighborhood_context()
        source = "def helper(value: int) -> int:\n    return value + 2\n"
        cases = [
            {
                "core_method": "search_symbols_json",
                "rpc_method": "arborist/search_symbols",
                "request_id": 57,
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                    "timeout_ms": 37,
                },
                "payload": self.make_search_result(),
                "expected_call": (
                    ".",
                    "helper",
                    5,
                    "symbols.db",
                    "graph",
                    "function_definition",
                    None,
                    None,
                    37,
                ),
                "check": lambda result: (
                    self.assertEqual(result["query"], "helper"),
                    self.assertEqual(result["total_matches"], 1),
                    self.assertFalse(result["truncated"]),
                    self.assertEqual(result["matches"][0]["semantic_path"], "helper"),
                    self.assertEqual(result["match_details"][0]["score"], 1000),
                ),
            },
            {
                "core_method": "search_symbols_json",
                "rpc_method": "arborist/search_symbols",
                "request_id": 174,
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "file_path": "graph_b.py",
                    "source": source,
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": self.make_search_result(),
                "expected_call": (
                    ".",
                    "helper",
                    5,
                    None,
                    "graph",
                    "function_definition",
                    "graph_b.py",
                    source,
                ),
                "check": lambda result: (
                    self.assertEqual(result["query"], "helper"),
                    self.assertEqual(result["matches"][0]["semantic_path"], "helper"),
                ),
            },
            {
                "core_method": "search_symbols_context_json",
                "rpc_method": "arborist/search_symbols_context",
                "request_id": 77,
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                    "timeout_ms": 37,
                },
                "payload": {
                    "search": self.make_search_result(),
                    "reads": [helper_read],
                },
                "expected_call": (
                    ".",
                    "helper",
                    5,
                    "symbols.db",
                    "graph",
                    "function_definition",
                    None,
                    None,
                    37,
                ),
                "check": lambda result: (
                    self.assertEqual(result["search"]["query"], "helper"),
                    self.assertEqual(result["search"]["total_matches"], 1),
                    self.assertEqual(
                        result["reads"][0]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertIn("def helper()", result["reads"][0]["source"]),
                ),
            },
            {
                "core_method": "search_symbols_neighborhood_context_json",
                "rpc_method": "arborist/search_symbols_neighborhood_context",
                "request_id": 78,
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
                    "timeout_ms": 37,
                },
                "payload": {
                    "search": self.make_search_result(),
                    "contexts": [helper_context],
                },
                "expected_call": (
                    ".",
                    "helper",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph",
                    "function_definition",
                    None,
                    None,
                    37,
                ),
                "check": lambda result: (
                    self.assertEqual(result["search"]["query"], "helper"),
                    self.assertEqual(
                        result["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
                        "helper",
                    ),
                    self.assertIn(
                        "def helper()",
                        result["contexts"][0]["reads"][0]["source"],
                    ),
                ),
            },
            {
                "core_method": "search_symbols_discovery_context_json",
                "rpc_method": "arborist/search_symbols_discovery_context",
                "request_id": 86,
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
                    "timeout_ms": 37,
                },
                "payload": {
                    "search": self.make_search_result(),
                    "reads": [helper_read],
                    "contexts": [helper_context],
                },
                "expected_call": (
                    ".",
                    "helper",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph",
                    "function_definition",
                    None,
                    None,
                    37,
                ),
                "check": lambda result: (
                    self.assertEqual(result["search"]["query"], "helper"),
                    self.assertEqual(
                        result["reads"][0]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertEqual(
                        result["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
                        "helper",
                    ),
                ),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload=case["payload"],
                    request_id=case["request_id"],
                    expected_call=case["expected_call"],
                    check_result=case["check"],
                )

    def test_list_routes_params_to_core(self) -> None:
        helper_read = self.helper_read()
        helper_context = self.helper_neighborhood_context()
        source = "def helper(value: int) -> int:\n    return value + 2\n"
        cases = [
            {
                "core_method": "list_symbols_json",
                "rpc_method": "arborist/list_symbols",
                "request_id": 60,
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                    "timeout_ms": 37,
                },
                "payload": self.make_list_result(),
                "expected_call": (
                    ".",
                    25,
                    "symbols.db",
                    "graph",
                    "function_definition",
                    None,
                    None,
                    37,
                ),
                "check": lambda result: (
                    self.assertEqual(result["total_symbols"], 1),
                    self.assertFalse(result["truncated"]),
                    self.assertEqual(result["symbols"][0]["semantic_path"], "helper"),
                ),
            },
            {
                "core_method": "list_symbols_json",
                "rpc_method": "arborist/list_symbols",
                "request_id": 175,
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "file_path": "graph_b.py",
                    "source": source,
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": self.make_list_result(),
                "expected_call": (
                    ".",
                    25,
                    None,
                    "graph",
                    "function_definition",
                    "graph_b.py",
                    source,
                ),
                "check": lambda result: (
                    self.assertEqual(result["total_symbols"], 1),
                    self.assertEqual(result["symbols"][0]["semantic_path"], "helper"),
                ),
            },
            {
                "core_method": "list_symbols_json",
                "rpc_method": "arborist/list_symbols",
                "request_id": 176,
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "file_path": "graph_b.py",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": self.make_list_result(),
                "expected_call": (
                    ".",
                    25,
                    None,
                    "graph",
                    "function_definition",
                    "graph_b.py",
                    None,
                ),
                "check": lambda result: (
                    self.assertEqual(result["total_symbols"], 1),
                    self.assertEqual(result["symbols"][0]["semantic_path"], "helper"),
                ),
            },
            {
                "core_method": "list_symbols_context_json",
                "rpc_method": "arborist/list_symbols_context",
                "request_id": 61,
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                    "timeout_ms": 37,
                },
                "payload": {
                    "list": self.make_list_result(),
                    "reads": [helper_read],
                },
                "expected_call": (
                    ".",
                    25,
                    "symbols.db",
                    "graph",
                    "function_definition",
                    None,
                    None,
                    37,
                ),
                "check": lambda result: (
                    self.assertEqual(result["list"]["total_symbols"], 1),
                    self.assertEqual(
                        result["list"]["symbols"][0]["semantic_path"], "helper"
                    ),
                    self.assertIn("def helper()", result["reads"][0]["source"]),
                ),
            },
            {
                "core_method": "list_symbols_neighborhood_context_json",
                "rpc_method": "arborist/list_symbols_neighborhood_context",
                "request_id": 81,
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                    "timeout_ms": 37,
                },
                "payload": {
                    "list": self.make_list_result(),
                    "contexts": [helper_context],
                },
                "expected_call": (
                    ".",
                    25,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph",
                    "function_definition",
                    None,
                    None,
                    37,
                ),
                "check": lambda result: (
                    self.assertEqual(result["list"]["total_symbols"], 1),
                    self.assertEqual(
                        result["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
                        "helper",
                    ),
                    self.assertIn(
                        "def helper()",
                        result["contexts"][0]["reads"][0]["source"],
                    ),
                ),
            },
            {
                "core_method": "list_symbols_discovery_context_json",
                "rpc_method": "arborist/list_symbols_discovery_context",
                "request_id": 87,
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                    "timeout_ms": 37,
                },
                "payload": {
                    "list": self.make_list_result(),
                    "reads": [helper_read],
                    "contexts": [helper_context],
                },
                "expected_call": (
                    ".",
                    25,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph",
                    "function_definition",
                    None,
                    None,
                    37,
                ),
                "check": lambda result: (
                    self.assertEqual(result["list"]["total_symbols"], 1),
                    self.assertEqual(
                        result["reads"][0]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertEqual(
                        result["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
                        "helper",
                    ),
                ),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload=case["payload"],
                    request_id=case["request_id"],
                    expected_call=case["expected_call"],
                    check_result=case["check"],
                )

    def test_read_routes_params_to_core(self) -> None:
        helper_read = self.helper_read()
        helper_trace = self.helper_trace_context()
        helper_context = self.helper_neighborhood_context()
        helper_read_graph = self.helper_read(file_path="graph_b.py")
        helper_trace_graph = self.helper_trace_context(file_path="graph_b.py")
        helper_context_graph = self.helper_neighborhood_context(file_path="graph_b.py")
        source = "def helper(value: int) -> int:\n    return value + 2\n"
        cases = [
            {
                "core_method": "read_symbol_json",
                "rpc_method": "arborist/read_symbol",
                "request_id": 176,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "file_path": "graph_b.py",
                    "source": source,
                },
                "payload": helper_read,
                "expected_call": (".", "helper", None, "graph_b.py", source),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertIn("def helper()", result["source"]),
                ),
            },
            {
                "core_method": "read_symbol_json",
                "rpc_method": "arborist/read_symbol",
                "request_id": 61,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "index_db_path": "symbols.db",
                },
                "payload": helper_read,
                "expected_call": (".", "helper", "symbols.db"),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertIn("def helper()", result["source"]),
                ),
            },
            {
                "core_method": "read_symbol_at_position_json",
                "rpc_method": "arborist/read_symbol_at_position",
                "request_id": 62,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "index_db_path": "symbols.db",
                },
                "payload": helper_read_graph,
                "expected_call": (".", "graph_b.py", 0, 5, None, "symbols.db"),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertIn("def helper()", result["source"]),
                ),
            },
            {
                "core_method": "trace_symbol_graph_json",
                "rpc_method": "arborist/trace_symbol_graph",
                "request_id": 177,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "file_path": "graph_b.py",
                    "source": source,
                },
                "payload": helper_trace,
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    None,
                    "graph_b.py",
                    source,
                    None,
                ),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "trace_symbol_graph_json",
                "rpc_method": "arborist/trace_symbol_graph",
                "request_id": 60,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                },
                "payload": helper_trace,
                "expected_call": (".", "helper", "callers", "symbols.db", None, None, None),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "trace_symbol_graph_at_position_json",
                "rpc_method": "arborist/trace_symbol_graph_at_position",
                "request_id": 65,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                },
                "payload": helper_trace_graph,
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    None,
                    "symbols.db",
                    None,
                ),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "trace_symbol_neighborhood_json",
                "rpc_method": "arborist/trace_symbol_neighborhood",
                "request_id": 66,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                "payload": helper_context["neighborhood"],
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    None,
                ),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(result["direction"], "callers"),
                    self.assertEqual(
                        result["nodes"][1]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(result["edges"][0]["to_symbol_id"], "helper"),
                ),
            },
            {
                "core_method": "trace_symbol_neighborhood_at_position_json",
                "rpc_method": "arborist/trace_symbol_neighborhood_at_position",
                "request_id": 67,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                "payload": helper_context_graph["neighborhood"],
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    None,
                    "symbols.db",
                    None,
                ),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(result["direction"], "callers"),
                    self.assertEqual(
                        result["nodes"][1]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(result["edges"][0]["to_symbol_id"], "helper"),
                ),
            },
            {
                "core_method": "read_symbol_context_json",
                "rpc_method": "arborist/read_symbol_context",
                "request_id": 178,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "file_path": "graph_b.py",
                    "source": source,
                },
                "payload": {"read": helper_read, "trace": helper_trace},
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    None,
                    "graph_b.py",
                    source,
                ),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["trace"]["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "read_symbol_context_json",
                "rpc_method": "arborist/read_symbol_context",
                "request_id": 63,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                },
                "payload": {"read": helper_read, "trace": helper_trace},
                "expected_call": (".", "helper", "callers", "symbols.db"),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["trace"]["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "read_symbol_context_at_position_json",
                "rpc_method": "arborist/read_symbol_context_at_position",
                "request_id": 64,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                },
                "payload": {"read": helper_read_graph, "trace": helper_trace_graph},
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    None,
                    "symbols.db",
                ),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["trace"]["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "read_symbol_neighborhood_context_json",
                "rpc_method": "arborist/read_symbol_neighborhood_context",
                "request_id": 179,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "file_path": "graph_b.py",
                    "source": source,
                    "timeout_ms": 37,
                },
                "payload": helper_context,
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    None,
                    "graph_b.py",
                    source,
                    37,
                ),
                "check": lambda result: (
                    self.assertEqual(
                        result["neighborhood"]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertEqual(
                        result["reads"][1]["symbol"]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "read_symbol_neighborhood_context_json",
                "rpc_method": "arborist/read_symbol_neighborhood_context",
                "request_id": 72,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                "payload": helper_context,
                "expected_call": (".", "helper", "callers", 2, 10, "symbols.db"),
                "check": lambda result: (
                    self.assertEqual(
                        result["neighborhood"]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertEqual(
                        result["reads"][1]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertIn("def helper()", result["reads"][0]["source"]),
                ),
            },
            {
                "core_method": "read_symbol_neighborhood_context_json",
                "rpc_method": "arborist/read_symbol_neighborhood_context",
                "request_id": 187,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "payload": helper_context,
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    37,
                ),
                "check": lambda result: self.assertEqual(
                    result["neighborhood"]["symbol"]["semantic_path"], "helper"
                ),
            },
            {
                "core_method": "read_symbol_neighborhood_context_at_position_json",
                "rpc_method": "arborist/read_symbol_neighborhood_context_at_position",
                "request_id": 73,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "payload": helper_context_graph,
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    None,
                    "symbols.db",
                    37,
                ),
                "check": lambda result: (
                    self.assertEqual(
                        result["neighborhood"]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertEqual(
                        result["reads"][1]["symbol"]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "read_symbol_discovery_context_json",
                "rpc_method": "arborist/read_symbol_discovery_context",
                "request_id": 180,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "file_path": "graph_b.py",
                    "source": source,
                },
                "payload": {
                    "read": helper_read,
                    "trace": helper_trace,
                    "neighborhood_context": helper_context,
                },
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    None,
                    "graph_b.py",
                    source,
                ),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(result["trace"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["neighborhood_context"]["reads"][1]["symbol"][
                            "semantic_path"
                        ],
                        "orchestrate",
                    ),
                ),
            },
            {
                "core_method": "read_symbol_discovery_context_json",
                "rpc_method": "arborist/read_symbol_discovery_context",
                "request_id": 74,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                "payload": {
                    "read": helper_read,
                    "trace": helper_trace,
                    "neighborhood_context": helper_context,
                },
                "expected_call": (".", "helper", "callers", 2, 10, "symbols.db"),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(result["trace"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["neighborhood_context"]["reads"][1]["symbol"][
                            "semantic_path"
                        ],
                        "orchestrate",
                    ),
                ),
            },
            {
                "core_method": "read_symbol_discovery_context_at_position_json",
                "rpc_method": "arborist/read_symbol_discovery_context_at_position",
                "request_id": 75,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                "payload": {
                    "read": helper_read_graph,
                    "trace": helper_trace_graph,
                    "neighborhood_context": helper_context_graph,
                },
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    None,
                    "symbols.db",
                ),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(result["trace"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["neighborhood_context"]["reads"][1]["symbol"][
                            "semantic_path"
                        ],
                        "orchestrate",
                    ),
                ),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload=case["payload"],
                    request_id=case["request_id"],
                    expected_call=case["expected_call"],
                    check_result=case["check"],
                )

    def test_position_routes_source_params_to_core(self) -> None:
        helper_read_graph = self.helper_read(file_path="graph_b.py")
        helper_trace_graph = self.helper_trace_context(file_path="graph_b.py")
        helper_context_graph = self.helper_neighborhood_context(file_path="graph_b.py")
        source = "def helper(value: int) -> int:\n    return value + 2\n"
        cases = [
            {
                "core_method": "read_symbol_at_position_json",
                "rpc_method": "arborist/read_symbol_at_position",
                "request_id": 168,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "source": source,
                },
                "payload": helper_read_graph,
                "expected_call": (".", "graph_b.py", 0, 5, source, None),
            },
            {
                "core_method": "trace_symbol_graph_at_position_json",
                "rpc_method": "arborist/trace_symbol_graph_at_position",
                "request_id": 169,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "source": source,
                },
                "payload": helper_trace_graph,
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    source,
                    None,
                    None,
                ),
            },
            {
                "core_method": "trace_symbol_neighborhood_at_position_json",
                "rpc_method": "arborist/trace_symbol_neighborhood_at_position",
                "request_id": 170,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                },
                "payload": helper_context_graph["neighborhood"],
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    source,
                    None,
                    None,
                ),
            },
            {
                "core_method": "read_symbol_context_at_position_json",
                "rpc_method": "arborist/read_symbol_context_at_position",
                "request_id": 171,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "source": source,
                },
                "payload": {"read": helper_read_graph, "trace": helper_trace_graph},
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    source,
                    None,
                ),
            },
            {
                "core_method": "read_symbol_neighborhood_context_at_position_json",
                "rpc_method": "arborist/read_symbol_neighborhood_context_at_position",
                "request_id": 172,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                },
                "payload": helper_context_graph,
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    source,
                    None,
                ),
            },
            {
                "core_method": "read_symbol_discovery_context_at_position_json",
                "rpc_method": "arborist/read_symbol_discovery_context_at_position",
                "request_id": 173,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                },
                "payload": {
                    "read": helper_read_graph,
                    "trace": helper_trace_graph,
                    "neighborhood_context": helper_context_graph,
                },
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    source,
                    None,
                ),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload=case["payload"],
                    request_id=case["request_id"],
                    expected_call=case["expected_call"],
                    check_result=lambda result: self.assertTrue(result),
                )
