from __future__ import annotations

from arborist_mcp import gateway as gateway_module


class GatewayParameterRequestValidationMixin:
    def test_rejects_string_bool_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 11,
                "method": "arborist/list_virtual_files",
                "params": {"dirty_only": "false"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 11)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("dirty_only", response["error"]["message"])

    def test_rejects_string_int_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 13,
                "method": "arborist/get_semantic_skeleton",
                "params": {"file_path": "sample.py", "depth_limit": "2"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 13)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("depth_limit", response["error"]["message"])

    def test_rejects_bool_int_params(self) -> None:
        class StubCore:
            def get_semantic_skeleton_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

            def apply_buffer_edit_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        cases = [
            (
                "arborist/get_semantic_skeleton",
                {"file_path": "sample.py", "depth_limit": True},
                "depth_limit",
            ),
            (
                "arborist/apply_buffer_edit",
                {
                    "file_path": "sample.py",
                    "start_byte": True,
                    "old_end_byte": 1,
                    "new_text": "x",
                },
                "start_byte",
            ),
        ]

        for method, params, expected_message in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 42,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 42)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn(expected_message, response["error"]["message"])

    def test_rejects_negative_optional_int_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 14,
                "method": "arborist/get_semantic_skeleton",
                "params": {"file_path": "sample.py", "depth_limit": -1},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 14)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("depth_limit", response["error"]["message"])

    def test_rejects_negative_search_limit(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 55,
                "method": "arborist/search_symbols",
                "params": {"workspace_root": ".", "query": "helper", "limit": -1},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 55)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("limit", response["error"]["message"])

    def test_rejects_negative_index_max_files(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 56,
                "method": "arborist/rebuild_symbol_index",
                "params": {"workspace_root": ".", "db_path": "symbols.db", "max_files": -1},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 56)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_files", response["error"]["message"])

    def test_rejects_zero_index_max_files(self) -> None:
        class StubCore:
            def refresh_symbol_index_for_file_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway(StubCore())

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 57,
                "method": "arborist/refresh_symbol_index_for_file",
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "file_path": "helper.py",
                    "max_files": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 57)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_files", response["error"]["message"])

    def test_rejects_zero_index_max_file_bytes(self) -> None:
        class StubCore:
            def rebuild_symbol_index_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway(StubCore())

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 58,
                "method": "arborist/rebuild_symbol_index",
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "max_file_bytes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 58)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_file_bytes", response["error"]["message"])

    def test_rejects_excessive_bounded_integer_params_without_calling_core(self) -> None:
        class StubCore:
            def __getattr__(self, name: str) -> object:
                raise AssertionError(f"core method should not be called: {name}")

        cases = [
            (
                "arborist/search_symbols",
                {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": gateway_module.MAX_SYMBOL_LIMIT + 1,
                },
                "limit",
            ),
            (
                "arborist/trace_symbol_neighborhood",
                {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "max_nodes": gateway_module.MAX_GRAPH_NODES + 1,
                },
                "max_nodes",
            ),
            (
                "arborist/trace_symbol_neighborhood",
                {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "max_depth": gateway_module.MAX_GRAPH_DEPTH + 1,
                },
                "max_depth",
            ),
            (
                "arborist/rebuild_symbol_index",
                {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "max_files": gateway_module.MAX_WORKSPACE_SCAN_FILES + 1,
                },
                "max_files",
            ),
            (
                "arborist/rebuild_symbol_index",
                {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "max_file_bytes": gateway_module.MAX_WORKSPACE_SCAN_FILE_BYTES + 1,
                },
                "max_file_bytes",
            ),
            (
                "arborist/rebuild_symbol_index",
                {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "timeout_ms": gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1,
                },
                "timeout_ms",
            ),
            (
                "arborist/refresh_symbol_index",
                {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "max_files": gateway_module.MAX_WORKSPACE_SCAN_FILES + 1,
                },
                "max_files",
            ),
            (
                "arborist/execute_tree_query",
                {
                    "file_path": "sample.py",
                    "query": "(module) @root",
                    "max_captures": gateway_module.TREE_QUERY_MAX_CAPTURES + 1,
                },
                "max_captures",
            ),
            (
                "arborist/execute_tree_query",
                {
                    "file_path": "sample.py",
                    "query": "(module) @root",
                    "timeout_ms": gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1,
                },
                "timeout_ms",
            ),
        ]

        for index, (method, params, expected_param) in enumerate(cases, start=1):
            with self.subTest(method=method, param=expected_param):
                response = self.make_gateway(StubCore()).handle_request(
                    self.request(method, params, request_id=340 + index)
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 340 + index)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn(expected_param, response["error"]["message"])
                self.assertIn("exceeds maximum", response["error"]["message"])

    def test_rejects_oversized_text_params_before_core_call(self) -> None:
        class StubCore:
            def get_semantic_skeleton_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

            def patch_ast_node_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

            def apply_buffer_edit_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

            def apply_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway(StubCore())
        oversized_text = "x" * (gateway_module.TEXT_PARAM_MAX_LENGTH + 1)
        oversized_reason = "x" * (gateway_module.BYPASS_REASON_MAX_LENGTH + 1)
        cases = [
            (
                "arborist/get_semantic_skeleton",
                {"file_path": "sample.py", "source": oversized_text},
                "source",
            ),
            (
                "arborist/patch_ast_node",
                {
                    "file_path": "sample.py",
                    "semantic_path": "top_level",
                    "new_code": oversized_text,
                },
                "new_code",
            ),
            (
                "arborist/patch_ast_node",
                {
                    "file_path": "sample.py",
                    "semantic_path": "top_level",
                    "new_code": "def top_level():\n    return 1\n",
                    "bypass_reason": oversized_reason,
                },
                "bypass_reason",
            ),
            (
                "arborist/apply_buffer_edit",
                {
                    "file_path": "sample.py",
                    "start_byte": 0,
                    "old_end_byte": 0,
                    "new_text": oversized_text,
                },
                "new_text",
            ),
            (
                "arborist/did_change",
                {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": 0, "column": 0},
                            "end": {"row": 0, "column": 0},
                            "new_text": oversized_text,
                        }
                    ],
                },
                "edits[0].new_text",
            ),
        ]

        for method, params, expected_message in cases:
            with self.subTest(method=method, expected_message=expected_message):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 58,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 58)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn(expected_message, response["error"]["message"])
                self.assertIn("max length", response["error"]["message"])

    def test_rejects_non_string_optional_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 15,
                "method": "arborist/trace_symbol_graph",
                "params": {"workspace_root": 123, "symbol_path": "top_level"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 15)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("workspace_root", response["error"]["message"])

    def test_rejects_blank_required_string_params(self) -> None:
        class StubCore:
            def get_semantic_skeleton_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 39,
                "method": "arborist/get_semantic_skeleton",
                "params": {"file_path": "   "},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 39)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("file_path", response["error"]["message"])

    def test_rejects_blank_optional_string_params(self) -> None:
        class StubCore:
            def trace_symbol_graph_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 40,
                "method": "arborist/trace_symbol_graph",
                "params": {"workspace_root": "   ", "symbol_path": "top_level"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 40)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("workspace_root", response["error"]["message"])

    def test_rejects_blank_search_query(self) -> None:
        class StubCore:
            def search_symbols_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 56,
                "method": "arborist/search_symbols",
                "params": {"workspace_root": ".", "query": "   "},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 56)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("query", response["error"]["message"])

    def test_rejects_blank_search_filters(self) -> None:
        class StubCore:
            def search_symbols_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 58,
                "method": "arborist/search_symbols",
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "file_path_contains": "   ",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 58)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("file_path_contains", response["error"]["message"])

    def test_rejects_blank_list_symbols_filters(self) -> None:
        class StubCore:
            def list_symbols_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 59,
                "method": "arborist/list_symbols",
                "params": {"workspace_root": ".", "node_kind": "   "},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 59)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("node_kind", response["error"]["message"])

    def test_rejects_null_string_param_with_default(self) -> None:
        class StubCore:
            def trace_symbol_graph_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 38,
                "method": "arborist/trace_symbol_graph",
                "params": {"workspace_root": None, "symbol_path": "top_level"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 38)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("workspace_root", response["error"]["message"])

