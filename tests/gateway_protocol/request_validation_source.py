from __future__ import annotations

from tests.gateway_protocol.helpers import make_recording_json_core


class GatewaySourceRequestValidationMixin:
    def test_rejects_invalid_read_symbol_at_position_position_as_invalid_params(self) -> None:
        class StubCore:
            def read_symbol_at_position_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        cases = [
            (
                "arborist/read_symbol_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": -1},
                },
                "position.column",
            ),
            (
                "arborist/read_symbol_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "character": 2},
                },
                "position.character",
            ),
            (
                "arborist/read_symbol_neighborhood_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": "1:2",
                },
                "position",
            ),
            (
                "arborist/read_symbol_discovery_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": True, "column": 2},
                },
                "position.row",
            ),
        ]

        for method, params, expected_key in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 86,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 86)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn(expected_key, response["error"]["message"])

    def test_position_entrypoints_allow_source_with_index_db_path(self) -> None:
        source = "def helper() -> int:\n    return 1\n"
        core = make_recording_json_core(
            read_symbol_at_position_json={},
            read_symbol_context_at_position_json={},
            read_symbol_neighborhood_context_at_position_json={},
            read_symbol_discovery_context_at_position_json={},
            trace_symbol_graph_at_position_json={},
            trace_symbol_neighborhood_at_position_json={},
        )
        gateway = self.make_gateway(core)

        cases = [
            (
                "read_symbol_at_position_json",
                "arborist/read_symbol_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "source": source,
                    "index_db_path": "symbols.db",
                },
                (".", "graph_a.py", 1, 2, source, "symbols.db"),
            ),
            (
                "read_symbol_context_at_position_json",
                "arborist/read_symbol_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "direction": "callers",
                    "source": source,
                    "index_db_path": "symbols.db",
                },
                (".", "graph_a.py", 1, 2, "callers", source, "symbols.db"),
            ),
            (
                "read_symbol_neighborhood_context_at_position_json",
                "arborist/read_symbol_neighborhood_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                    "index_db_path": "symbols.db",
                },
                (".", "graph_a.py", 1, 2, "callers", 2, 10, source, "symbols.db"),
            ),
            (
                "read_symbol_discovery_context_at_position_json",
                "arborist/read_symbol_discovery_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                    "index_db_path": "symbols.db",
                },
                (".", "graph_a.py", 1, 2, "callers", 2, 10, source, "symbols.db"),
            ),
            (
                "trace_symbol_graph_at_position_json",
                "arborist/trace_symbol_graph_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "direction": "callers",
                    "source": source,
                    "index_db_path": "symbols.db",
                    "timeout_ms": 5000,
                },
                (".", "graph_a.py", 1, 2, "callers", source, "symbols.db", 5000),
            ),
            (
                "trace_symbol_neighborhood_at_position_json",
                "arborist/trace_symbol_neighborhood_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                    "index_db_path": "symbols.db",
                    "timeout_ms": 5000,
                },
                (".", "graph_a.py", 1, 2, "callers", 2, 10, source, "symbols.db", 5000),
            ),
        ]

        for core_method, rpc_method, params, expected_call in cases:
            with self.subTest(method=rpc_method):
                result = self.assert_jsonrpc_ok(
                    self.call_gateway(gateway, rpc_method, params, request_id=111),
                    request_id=111,
                )
                self.assertEqual(result, {})
                self.assertEqual(core.calls_for(core_method), [expected_call])

    def test_rejects_invalid_patch_at_position_position_as_invalid_params(self) -> None:
        class StubCore:
            def patch_ast_node_at_position_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

            def patch_virtual_ast_node_at_position_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

            def validate_patch_with_trace_context_at_position_json(
                self, *args: object
            ) -> str:
                raise AssertionError("core should not be called")

            def validate_patch_with_graph_context_at_position_json(
                self, *args: object
            ) -> str:
                raise AssertionError("core should not be called")

            def validate_patch_with_neighborhood_context_at_position_json(
                self, *args: object
            ) -> str:
                raise AssertionError("core should not be called")

            def validate_patch_with_discovery_context_at_position_json(
                self, *args: object
            ) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        cases = [
            (
                "arborist/patch_ast_node_at_position",
                {
                    "file_path": "sample.py",
                    "position": {"row": 1, "column": -1},
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position.column",
            ),
            (
                "arborist/patch_virtual_ast_node_at_position",
                {
                    "file_path": "sample.py",
                    "position": {"row": 1, "character": 2},
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position.character",
            ),
            (
                "arborist/validate_patch_with_trace_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": "1:2",
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position",
            ),
            (
                "arborist/validate_patch_with_graph_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": True, "column": 2},
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position.row",
            ),
            (
                "arborist/validate_patch_with_neighborhood_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": 1, "column": -1},
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position.column",
            ),
            (
                "arborist/validate_patch_with_discovery_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": 1, "character": 2},
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position.character",
            ),
        ]

        for method, params, expected_key in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 88,
                        "method": method,
                        "params": params,
                    }
                )

    def test_path_and_workspace_entrypoints_allow_source_with_index_db_path(self) -> None:
        source = "def helper() -> int:\n    return 1\n"
        core = make_recording_json_core(
            read_symbol_json={},
            read_symbol_context_json={},
            read_symbol_neighborhood_context_json={},
            read_symbol_discovery_context_json={},
            trace_symbol_graph_json={},
            trace_symbol_neighborhood_json={},
            search_symbols_json={},
            search_symbols_context_json={},
            search_symbols_neighborhood_context_json={},
            search_symbols_discovery_context_json={},
            list_symbols_json={},
            list_symbols_context_json={},
            list_symbols_neighborhood_context_json={},
            list_symbols_discovery_context_json={},
        )
        gateway = self.make_gateway(core)

        shared = {
            "workspace_root": ".",
            "file_path": "graph_a.py",
            "source": source,
            "index_db_path": "symbols.db",
        }
        cases = [
            (
                "read_symbol_json",
                "arborist/read_symbol",
                {**shared, "symbol_path": "helper"},
                (".", "helper", "symbols.db", "graph_a.py", source),
            ),
            (
                "read_symbol_context_json",
                "arborist/read_symbol_context",
                {**shared, "symbol_path": "helper", "direction": "callers"},
                (".", "helper", "callers", "symbols.db", "graph_a.py", source),
            ),
            (
                "read_symbol_neighborhood_context_json",
                "arborist/read_symbol_neighborhood_context",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph_a.py",
                    source,
                ),
            ),
            (
                "read_symbol_discovery_context_json",
                "arborist/read_symbol_discovery_context",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph_a.py",
                    source,
                ),
            ),
            (
                "trace_symbol_graph_json",
                "arborist/trace_symbol_graph",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "timeout_ms": 5000,
                },
                (".", "helper", "callers", "symbols.db", "graph_a.py", source, 5000),
            ),
            (
                "trace_symbol_neighborhood_json",
                "arborist/trace_symbol_neighborhood",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "timeout_ms": 5000,
                },
                (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph_a.py",
                    source,
                    5000,
                ),
            ),
            (
                "search_symbols_json",
                "arborist/search_symbols",
                {**shared, "query": "helper", "limit": 5},
                (".", "helper", 5, "symbols.db", None, None, "graph_a.py", source),
            ),
            (
                "search_symbols_context_json",
                "arborist/search_symbols_context",
                {**shared, "query": "helper", "limit": 5},
                (".", "helper", 5, "symbols.db", None, None, "graph_a.py", source),
            ),
            (
                "search_symbols_neighborhood_context_json",
                "arborist/search_symbols_neighborhood_context",
                {
                    **shared,
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    "helper",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    "graph_a.py",
                    source,
                ),
            ),
            (
                "search_symbols_discovery_context_json",
                "arborist/search_symbols_discovery_context",
                {
                    **shared,
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    "helper",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    "graph_a.py",
                    source,
                ),
            ),
            (
                "list_symbols_json",
                "arborist/list_symbols",
                {**shared, "limit": 5},
                (".", 5, "symbols.db", None, None, "graph_a.py", source),
            ),
            (
                "list_symbols_context_json",
                "arborist/list_symbols_context",
                {**shared, "limit": 5},
                (".", 5, "symbols.db", None, None, "graph_a.py", source),
            ),
            (
                "list_symbols_neighborhood_context_json",
                "arborist/list_symbols_neighborhood_context",
                {
                    **shared,
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    "graph_a.py",
                    source,
                ),
            ),
            (
                "list_symbols_discovery_context_json",
                "arborist/list_symbols_discovery_context",
                {
                    **shared,
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    "graph_a.py",
                    source,
                ),
            ),
        ]

        for core_method, rpc_method, params, expected_call in cases:
            with self.subTest(method=rpc_method):
                result = self.assert_jsonrpc_ok(
                    self.call_gateway(gateway, rpc_method, params, request_id=112),
                    request_id=112,
                )
                self.assertEqual(result, {})
                self.assertEqual(core.calls_for(core_method), [expected_call])

    def test_patch_context_entrypoints_allow_source_with_index_db_path(self) -> None:
        source = "def orchestrate(value: int) -> int:\n    return value + 1\n"
        new_code = "def orchestrate(value: int) -> int:\n    return helper(value)\n"
        core = make_recording_json_core(
            validate_patch_with_trace_context_json={},
            validate_patch_with_trace_context_at_position_json={},
            validate_patch_with_graph_context_json={},
            validate_patch_with_graph_context_at_position_json={},
            validate_patch_with_neighborhood_context_json={},
            validate_patch_with_neighborhood_context_at_position_json={},
            validate_patch_with_discovery_context_json={},
            validate_patch_with_discovery_context_at_position_json={},
        )
        gateway = self.make_gateway(core)

        cases = [
            (
                "validate_patch_with_trace_context_json",
                "arborist/validate_patch_with_trace_context",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "index_db_path": "symbols.db",
                },
                (".", "caller.py", "orchestrate", new_code, source, None, "both", "symbols.db"),
            ),
            (
                "validate_patch_with_trace_context_at_position_json",
                "arborist/validate_patch_with_trace_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 1, "column": 4},
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "index_db_path": "symbols.db",
                },
                (".", "caller.py", 1, 4, new_code, source, None, "both", "symbols.db"),
            ),
            (
                "validate_patch_with_graph_context_json",
                "arborist/validate_patch_with_graph_context",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (
                    ".",
                    "caller.py",
                    "orchestrate",
                    new_code,
                    source,
                    None,
                    "both",
                    2,
                    10,
                    "symbols.db",
                ),
            ),
            (
                "validate_patch_with_graph_context_at_position_json",
                "arborist/validate_patch_with_graph_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 1, "column": 4},
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (".", "caller.py", 1, 4, new_code, source, None, "both", 2, 10, "symbols.db"),
            ),
            (
                "validate_patch_with_neighborhood_context_json",
                "arborist/validate_patch_with_neighborhood_context",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (
                    ".",
                    "caller.py",
                    "orchestrate",
                    new_code,
                    source,
                    None,
                    "both",
                    2,
                    10,
                    "symbols.db",
                ),
            ),
            (
                "validate_patch_with_neighborhood_context_at_position_json",
                "arborist/validate_patch_with_neighborhood_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 1, "column": 4},
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (".", "caller.py", 1, 4, new_code, source, None, "both", 2, 10, "symbols.db"),
            ),
            (
                "validate_patch_with_discovery_context_json",
                "arborist/validate_patch_with_discovery_context",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (
                    ".",
                    "caller.py",
                    "orchestrate",
                    new_code,
                    source,
                    None,
                    "both",
                    2,
                    10,
                    "symbols.db",
                ),
            ),
            (
                "validate_patch_with_discovery_context_at_position_json",
                "arborist/validate_patch_with_discovery_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 1, "column": 4},
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (".", "caller.py", 1, 4, new_code, source, None, "both", 2, 10, "symbols.db"),
            ),
        ]

        for core_method, rpc_method, params, expected_call in cases:
            with self.subTest(method=rpc_method):
                result = self.assert_jsonrpc_ok(
                    self.call_gateway(gateway, rpc_method, params, request_id=113),
                    request_id=113,
                )
                self.assertEqual(result, {})
                self.assertEqual(core.calls_for(core_method), [expected_call])

    def test_rejects_missing_file_path_for_source_backed_path_and_workspace_entrypoints(self) -> None:
        class StubCore:
            def __getattr__(self, name: str):
                if name.endswith("_json"):
                    def _unexpected(*args: object) -> str:
                        raise AssertionError(f"core should not be called: {name}")

                    return _unexpected
                raise AttributeError(name)

        gateway = self.make_gateway()
        gateway._core = StubCore()

        shared = {
            "workspace_root": ".",
            "source": "def helper() -> int:\n    return 1\n",
        }
        cases = [
            ("arborist/read_symbol", {**shared, "symbol_path": "helper"}),
            (
                "arborist/read_symbol_context",
                {**shared, "symbol_path": "helper", "direction": "callers"},
            ),
            (
                "arborist/read_symbol_neighborhood_context",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            (
                "arborist/read_symbol_discovery_context",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            (
                "arborist/trace_symbol_graph",
                {**shared, "symbol_path": "helper", "direction": "callers"},
            ),
            (
                "arborist/trace_symbol_neighborhood",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            ("arborist/search_symbols", {**shared, "query": "helper", "limit": 5}),
            (
                "arborist/search_symbols_context",
                {**shared, "query": "helper", "limit": 5},
            ),
            (
                "arborist/search_symbols_neighborhood_context",
                {
                    **shared,
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            (
                "arborist/search_symbols_discovery_context",
                {
                    **shared,
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            ("arborist/list_symbols", {**shared, "limit": 5}),
            ("arborist/list_symbols_context", {**shared, "limit": 5}),
            (
                "arborist/list_symbols_neighborhood_context",
                {
                    **shared,
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            (
                "arborist/list_symbols_discovery_context",
                {
                    **shared,
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
        ]

        for method, params in cases:
            with self.subTest(method=method):
                self.assert_invalid_params(
                    method,
                    params,
                    request_id=113,
                    contains="file_path is required when source is provided",
                    gateway=gateway,
                )
