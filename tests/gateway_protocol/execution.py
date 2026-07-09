from __future__ import annotations

from tests.gateway_protocol.helpers import GatewayProtocolTestCase


class GatewayExecutionTests(GatewayProtocolTestCase):
    def test_core_invalid_query_maps_to_invalid_params(self) -> None:
        response = self.call_gateway(
            self.make_live_gateway(),
            "arborist/execute_tree_query",
            {
                "file_path": "tests/fixtures/sample.py",
                "query": "(function_definition @",
            },
            request_id=18,
        )

        self.assert_jsonrpc_error(response, request_id=18, code=-32602, contains="query")

    def test_rejects_reversed_buffer_edit_range(self) -> None:
        class StubCore:
            def apply_buffer_edit_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/apply_buffer_edit",
            {
                "file_path": "tests/fixtures/sample.py",
                "start_byte": 10,
                "old_end_byte": 2,
                "new_text": "x",
            },
            request_id=19,
        )

        self.assert_jsonrpc_error(
            response, request_id=19, code=-32602, contains="start_byte"
        )

    def test_rejects_negative_buffer_edit_offsets(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "arborist/apply_buffer_edit",
            {
                "file_path": "tests/fixtures/sample.py",
                "start_byte": -1,
                "old_end_byte": 2,
                "new_text": "x",
            },
            request_id=27,
        )

        self.assert_jsonrpc_error(
            response, request_id=27, code=-32602, contains="start_byte"
        )

    def test_passes_typed_optional_defaults_to_core(self) -> None:
        class StubCore:
            def get_semantic_skeleton_json(
                self,
                file_path: str,
                source: str | None,
                depth_limit: int,
                expand_nodes: list[str] | None,
            ) -> str:
                self.args = (file_path, source, depth_limit, expand_nodes)
                return "{}"

        core = StubCore()
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "arborist/get_semantic_skeleton",
                {"file_path": "sample.py"},
                request_id=20,
            ),
            request_id=20,
        )

        self.assertEqual(result, {})
        self.assertEqual(core.args, ("sample.py", None, 2, None))

    def test_get_semantic_skeleton_accepts_unsaved_source(self) -> None:
        with self.temp_workspace() as workspace:
            file_path = workspace.joinpath("sample.py")
            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/get_semantic_skeleton",
                    {
                        "file_path": str(file_path),
                        "source": "def top_level() -> int:\n    return 1\n",
                        "depth_limit": 2,
                    },
                    request_id=45,
                ),
                request_id=45,
            )

            assert isinstance(result, dict)
            self.assertFalse(file_path.exists())
            self.assertIn("top_level", result["available_paths"])
            self.assertTrue(
                any(
                    symbol["semantic_path"] == "top_level"
                    for symbol in result["available_symbols"]
                )
            )

    def test_source_backed_requests_return_normalized_file_paths(self) -> None:
        with self.temp_workspace() as workspace:
            nested = workspace.joinpath("child")
            nested.mkdir()
            file_path = workspace.joinpath("sample.py")
            alias_path = nested.joinpath("..", "sample.py")
            expected_file = str(file_path).replace("\\", "/")
            gateway = self.make_live_gateway()

            skeleton = self.assert_jsonrpc_ok(
                self.call_gateway(
                    gateway,
                    "arborist/get_semantic_skeleton",
                    {
                        "file_path": str(alias_path),
                        "source": "def top_level() -> int:\n    return 1\n",
                    },
                    request_id=46,
                ),
                request_id=46,
            )
            patch = self.assert_jsonrpc_ok(
                self.call_gateway(
                    gateway,
                    "arborist/patch_ast_node",
                    {
                        "file_path": str(alias_path),
                        "source": "def top_level() -> int:\n    return 1\n",
                        "semantic_path": "top_level",
                        "new_code": "def top_level() -> int:\n    return 2\n",
                    },
                    request_id=47,
                ),
                request_id=47,
            )

            assert isinstance(skeleton, dict)
            assert isinstance(patch, dict)
            self.assertEqual(skeleton["file"], expected_file)
            self.assertEqual(patch["file"], expected_file)
            self.assertFalse(file_path.exists())

    def test_execute_tree_query_source_returns_normalized_c_owner_path(self) -> None:
        with self.temp_workspace() as workspace:
            nested = workspace.joinpath("child")
            nested.mkdir()
            file_path = workspace.joinpath("sample.c")
            alias_path = nested.joinpath("..", "sample.c")
            expected_file = str(file_path).replace("\\", "/")
            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/execute_tree_query",
                    {
                        "file_path": str(alias_path),
                        "source": "static int orchestrate(int value) { return value + 1; }\n",
                        "query": (
                            "(function_definition declarator: "
                            "(function_declarator declarator: (identifier) @name))"
                        ),
                    },
                    request_id=48,
                ),
                request_id=48,
            )

            assert isinstance(result, list)
            self.assertEqual(len(result), 1)
            self.assertEqual(result[0]["owner_symbol_id"], f"{expected_file}::orchestrate")
            self.assertFalse(file_path.exists())

    def test_allows_null_nullable_string_params(self) -> None:
        class StubCore:
            def patch_ast_node_json(
                self,
                file_path: str,
                semantic_path: str,
                new_code: str,
                source: str | None,
                bypass_reason: str | None,
            ) -> str:
                self.args = (file_path, semantic_path, new_code, source, bypass_reason)
                return "{}"

        core = StubCore()
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "arborist/patch_ast_node",
                {
                    "file_path": "sample.py",
                    "semantic_path": "top_level",
                    "new_code": "def top_level():\n    return 1\n",
                    "source": None,
                    "bypass_reason": None,
                },
                request_id=39,
            ),
            request_id=39,
        )

        self.assertEqual(result, {})
        self.assertEqual(
            core.args,
            (
                "sample.py",
                "top_level",
                "def top_level():\n    return 1\n",
                None,
                None,
            ),
        )

    def test_patch_ast_node_accepts_unsaved_source_without_writing_disk(self) -> None:
        with self.temp_workspace() as workspace:
            file_path = workspace.joinpath("sample.py")
            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/patch_ast_node",
                    {
                        "file_path": str(file_path),
                        "source": "def top_level() -> int:\n    return 1\n",
                        "semantic_path": "top_level",
                        "new_code": "def top_level() -> int:\n    return 2\n",
                    },
                    request_id=44,
                ),
                request_id=44,
            )

            assert isinstance(result, dict)
            self.assertFalse(file_path.exists())
            self.assertTrue(result["applied"])
            self.assertIn("return 2", result["updated_source"])

    def test_patch_ast_node_at_position_accepts_unsaved_source_without_writing_disk(
        self,
    ) -> None:
        with self.temp_workspace() as workspace:
            file_path = workspace.joinpath("sample.py")
            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/patch_ast_node_at_position",
                    {
                        "file_path": str(file_path),
                        "source": (
                            "def decorator(func):\n"
                            "    return func\n\n"
                            "@decorator\n"
                            "def helper() -> int:\n"
                            "    return 1\n"
                        ),
                        "position": {"row": 3, "column": 1},
                        "new_code": "def helper() -> int:\n    return 2\n",
                    },
                    request_id=102,
                ),
                request_id=102,
            )

            assert isinstance(result, dict)
            self.assertFalse(file_path.exists())
            self.assertTrue(result["applied"])
            self.assertEqual(result["resolved_path"], "helper")
            self.assertIn("return 2", result["updated_source"])

    def test_trace_context_at_position_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            caller = workspace.joinpath("caller.py")
            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_trace_context_at_position",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return value + 1\n"
                        ),
                        "position": {"row": 3, "column": 5},
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "direction": "both",
                    },
                    request_id=103,
                ),
                request_id=103,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertTrue(result["patch"]["applied"])
            self.assertEqual(result["patch"]["resolved_path"], "orchestrate")
            self.assertEqual(result["trace_target"], "orchestrate")
            self.assertIsNone(result["trace_error"])
            self.assertTrue(result["trace_validation"]["allowed"])
            self.assertTrue(
                any(symbol["semantic_path"] == "helper" for symbol in result["trace"]["callees"])
            )

    def test_rejects_blank_expand_node_selectors(self) -> None:
        class StubCore:
            def get_semantic_skeleton_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway(StubCore())

        for selector in ("", " \t"):
            with self.subTest(selector=selector):
                response = self.call_gateway(
                    gateway,
                    "arborist/get_semantic_skeleton",
                    {"file_path": "sample.py", "expand_nodes": [selector]},
                    request_id=36,
                )
                self.assert_jsonrpc_error(
                    response, request_id=36, code=-32602, contains="expand_nodes"
                )

    def test_passes_valid_position_edits_to_core(self) -> None:
        class StubCore:
            def apply_position_edits_json(self, file_path: str, edits_json: str) -> str:
                self.args = (file_path, edits_json)
                return "{}"

        core = StubCore()
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "arborist/did_change",
                {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": 0, "column": 0},
                            "end": {"row": 0, "column": 0},
                            "new_text": "x",
                        }
                    ],
                },
                request_id=30,
            ),
            request_id=30,
        )

        self.assertEqual(result, {})
        self.assertEqual(core.args[0], "sample.py")
        self.assertIn('"new_text": "x"', core.args[1])

    def test_rejects_non_json_serializable_patch_object_as_invalid_params(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "arborist/replay_patch_evidence_against_trace",
            {"patch": {"binding_decisions": {1, 2}}, "trace": {}},
            request_id=21,
        )

        self.assert_jsonrpc_error(response, request_id=21, code=-32602, contains="patch")

    def test_rejects_non_finite_patch_object_as_invalid_params(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "arborist/replay_patch_evidence_against_trace",
            {"patch": {"confidence": float("inf")}, "trace": {}},
            request_id=26,
        )

        self.assert_jsonrpc_error(response, request_id=26, code=-32602, contains="patch")

    def test_rejects_non_string_patch_object_keys_as_invalid_params(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "arborist/replay_patch_evidence_against_trace",
            {"patch": {"file": "sample.py", 1: "coerces-to-string"}, "trace": {}},
            request_id=50,
        )

        self.assert_jsonrpc_error(response, request_id=50, code=-32602, contains="patch")
