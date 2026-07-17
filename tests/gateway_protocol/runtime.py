from __future__ import annotations

import io
from unittest import mock

from arborist_mcp import gateway as gateway_module

from tests.gateway_protocol.helpers import GatewayProtocolTestCase, make_recording_json_core

SUITE_NAME = "gateway-runtime"
REQUIRES_EXTENSION = True
COVERED_TOOLS = (
    "arborist/batch",
    "arborist/execute_tree_query",
    "arborist/get_semantic_skeleton",
    "arborist/list_symbol_indexes",
)


class GatewayRuntimeTests(GatewayProtocolTestCase):
    def test_live_initialize_reports_cpp_support(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(self.make_live_gateway(), "initialize", {}, request_id=0),
            request_id=0,
        )

        assert isinstance(result, dict)
        self.assertEqual(result["supportedLanguages"], ["python", "c", "cpp"])

    def test_initialize_still_reports_tools(self) -> None:
        class StubCore:
            def supported_languages(self) -> list[str]:
                return ["python", "c"]

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(StubCore()),
                "initialize",
                {},
                request_id=1,
            ),
            request_id=1,
        )

        assert isinstance(result, dict)
        self.assertEqual(result["serverInfo"]["version"], gateway_module.__version__)
        self.assertEqual(result["supportedLanguages"], ["python", "c"])
        self.assertEqual(
            result["capabilities"]["tools"],
            list(gateway_module.TOOL_NAMES),
        )
        self.assertEqual(
            result["capabilities"]["resources"],
            gateway_module.build_resource_catalog(),
        )

    def test_mcp_initialize_reports_standard_capabilities(self) -> None:
        class StubCore:
            def supported_languages(self) -> list[str]:
                return ["python", "c"]

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(StubCore()),
                "initialize",
                {
                    "protocolVersion": gateway_module.MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "unit-test", "version": "1.0"},
                },
                request_id=101,
            ),
            request_id=101,
        )

        assert isinstance(result, dict)
        self.assertEqual(result["protocolVersion"], gateway_module.MCP_PROTOCOL_VERSION)
        self.assertEqual(result["serverInfo"]["name"], "arborist-mcp")
        self.assertEqual(result["serverInfo"]["version"], gateway_module.__version__)
        self.assertEqual(
            result["capabilities"],
            {
                "tools": {"listChanged": False},
                "resources": {"subscribe": False, "listChanged": False},
            },
        )
        self.assertEqual(result["supportedLanguages"], ["python", "c"])

    def test_mcp_initialize_returns_supported_protocol_version(self) -> None:
        class StubCore:
            def supported_languages(self) -> list[str]:
                return ["python", "c"]

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(StubCore()),
                "initialize",
                {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {"name": "unit-test"},
                },
                request_id=111,
            ),
            request_id=111,
        )

        assert isinstance(result, dict)
        self.assertEqual(result["protocolVersion"], gateway_module.MCP_PROTOCOL_VERSION)

    def test_mcp_initialized_notification_is_noop(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "notifications/initialized",
                {},
                request_id=112,
            ),
            request_id=112,
        )

        self.assertEqual(result, {})

    def test_tools_list_returns_complete_tool_schemas(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(self.make_gateway(), "tools/list", {}, request_id=102),
            request_id=102,
        )

        assert isinstance(result, dict)
        tools = result["tools"]
        assert isinstance(tools, list)
        self.assertEqual(len(tools), len(gateway_module.TOOL_NAMES))
        by_name = {tool["name"]: tool for tool in tools}
        self.assertEqual(set(by_name), set(gateway_module.TOOL_NAMES))
        self.assertEqual(
            [spec.name for spec in gateway_module.TOOL_SPECS if spec.result_schema == "object"],
            [],
        )
        batch = by_name["arborist/batch"]
        self.assertEqual(batch["metadata"]["category"], "read")
        self.assertTrue(batch["annotations"]["readOnlyHint"])
        self.assertFalse(batch["metadata"]["mutatesState"])
        self.assertEqual(batch["inputSchema"]["required"], ["calls"])
        self.assertEqual(
            batch["inputSchema"]["properties"]["calls"]["maxItems"],
            gateway_module.MAX_BATCH_CALLS,
        )
        self.assertEqual(batch["outputSchema"]["properties"]["result"]["type"], "array")
        batch_item_schema = batch["outputSchema"]["properties"]["result"]["items"]
        batch_inner_result_schema = batch_item_schema["properties"]["result"]
        self.assertIn("anyOf", batch_inner_result_schema)
        self.assertIn(
            gateway_module.SEMANTIC_SKELETON_RESULT_SCHEMA,
            batch_inner_result_schema["anyOf"],
        )
        self.assertIn(gateway_module.SYMBOL_LIST_RESULT_SCHEMA, batch_inner_result_schema["anyOf"])
        self.assertIn(
            gateway_module.SYMBOL_INDEX_HEALTH_RESULT_SCHEMA,
            batch_inner_result_schema["anyOf"],
        )
        self.assertNotIn(
            gateway_module.PATCH_AST_NODE_RESULT_SCHEMA,
            batch_inner_result_schema["anyOf"],
        )
        skeleton = by_name["arborist/get_semantic_skeleton"]
        self.assertEqual(skeleton["metadata"]["category"], "read")
        self.assertEqual(skeleton["inputSchema"]["required"], ["file_path"])
        self.assertEqual(skeleton["outputSchema"]["required"], ["result"])
        skeleton_result = skeleton["outputSchema"]["properties"]["result"]
        self.assertEqual(skeleton_result["type"], "object")
        self.assertEqual(skeleton_result["additionalProperties"], False)
        self.assertEqual(
            skeleton_result["required"],
            ["file", "skeleton", "available_paths", "available_symbols"],
        )
        self.assertEqual(skeleton["inputSchema"]["properties"]["depth_limit"]["default"], 2)
        self.assertEqual(
            skeleton["inputSchema"]["properties"]["source"]["maxLength"],
            gateway_module.TEXT_PARAM_MAX_LENGTH,
        )
        self.assertIn(
            "Tree-sitter C++ grammar",
            skeleton["inputSchema"]["properties"]["file_path"]["description"],
        )
        list_indexes = by_name["arborist/list_symbol_indexes"]
        self.assertEqual(list_indexes["outputSchema"]["properties"]["result"]["type"], "array")
        rebuild_index = by_name["arborist/rebuild_symbol_index"]
        self.assertNotIn("max_files", rebuild_index["inputSchema"]["required"])
        self.assertEqual(
            rebuild_index["inputSchema"]["properties"]["max_files"]["default"], 20000
        )
        self.assertEqual(rebuild_index["inputSchema"]["properties"]["max_files"]["minimum"], 1)
        self.assertEqual(
            rebuild_index["inputSchema"]["properties"]["max_files"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_FILES,
        )
        self.assertNotIn("max_file_bytes", rebuild_index["inputSchema"]["required"])
        self.assertEqual(
            rebuild_index["inputSchema"]["properties"]["max_file_bytes"]["minimum"], 1
        )
        self.assertEqual(
            rebuild_index["inputSchema"]["properties"]["max_file_bytes"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_FILE_BYTES,
        )
        self.assertEqual(
            rebuild_index["inputSchema"]["properties"]["timeout_ms"]["minimum"],
            1,
        )
        self.assertEqual(
            rebuild_index["inputSchema"]["properties"]["timeout_ms"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        )
        refresh_index = by_name["arborist/refresh_symbol_index"]
        self.assertEqual(refresh_index["metadata"]["category"], "index")
        self.assertTrue(refresh_index["metadata"]["mutatesState"])
        self.assertFalse(refresh_index["annotations"]["readOnlyHint"])
        self.assertFalse(refresh_index["annotations"]["destructiveHint"])
        self.assertEqual(
            refresh_index["inputSchema"]["properties"]["max_files"]["default"], 20000
        )
        self.assertEqual(
            refresh_index["outputSchema"]["properties"]["result"],
            rebuild_index["outputSchema"]["properties"]["result"],
        )
        refresh_registered = by_name["arborist/refresh_registered_symbol_indexes"]
        self.assertEqual(refresh_registered["metadata"]["category"], "index")
        self.assertTrue(refresh_registered["metadata"]["mutatesState"])
        self.assertEqual(
            refresh_registered["outputSchema"]["properties"]["result"]["type"],
            "array",
        )
        self.assertEqual(
            refresh_registered["outputSchema"]["properties"]["result"]["items"],
            rebuild_index["outputSchema"]["properties"]["result"],
        )
        self.assertNotIn("timeout_ms", refresh_registered["inputSchema"]["required"])
        virtual_snapshot = by_name["arborist/read_virtual_file"]["outputSchema"]["properties"][
            "result"
        ]
        self.assertEqual(virtual_snapshot["additionalProperties"], False)
        self.assertIn("syntax_error_count", virtual_snapshot["required"])
        virtual_status = by_name["arborist/list_virtual_files"]["outputSchema"]["properties"][
            "result"
        ]["items"]
        self.assertEqual(virtual_status["additionalProperties"], False)
        self.assertEqual(
            virtual_status["required"], ["file", "dirty", "version", "syntax_error_count"]
        )
        virtual_edit = by_name["arborist/did_change"]["outputSchema"]["properties"]["result"]
        self.assertEqual(virtual_edit["additionalProperties"], False)
        self.assertEqual(
            virtual_edit["required"],
            ["file", "source", "dirty", "version", "incremental_parse", "validation"],
        )
        self.assertEqual(
            by_name["arborist/apply_buffer_edit"]["outputSchema"]["properties"]["result"],
            virtual_edit,
        )
        self.assertEqual(
            by_name["arborist/apply_buffer_edit"]["inputSchema"]["properties"]["new_text"][
                "maxLength"
            ],
            gateway_module.TEXT_PARAM_MAX_LENGTH,
        )
        self.assertEqual(
            by_name["arborist/did_change"]["inputSchema"]["properties"]["edits"]["items"][
                "properties"
            ]["new_text"]["maxLength"],
            gateway_module.TEXT_PARAM_MAX_LENGTH,
        )
        inspect_index = by_name["arborist/inspect_symbol_index"]
        self.assertTrue(inspect_index["annotations"]["readOnlyHint"])
        self.assertFalse(inspect_index["metadata"]["mutatesState"])
        self.assertEqual(
            inspect_index["inputSchema"]["properties"]["timeout_ms"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        )
        inspect_result = inspect_index["outputSchema"]["properties"]["result"]
        self.assertEqual(inspect_result["type"], "object")
        self.assertIn("response_schema_version", inspect_result["required"])
        self.assertIn("ok", inspect_result["required"])
        self.assertIn("fresh_file_count", inspect_result["required"])
        self.assertEqual(inspect_result["properties"]["stale_files"]["type"], "array")
        self.assertEqual(inspect_result["properties"]["missing_files"]["type"], "array")
        self.assertEqual(inspect_result["properties"]["unreadable_files"]["type"], "array")
        self.assertEqual(inspect_result["properties"]["unindexed_files"]["type"], "array")
        self.assertEqual(inspect_result["properties"]["issues"]["type"], "array")
        unregister = by_name["arborist/unregister_symbol_index"]
        self.assertEqual(unregister["outputSchema"]["properties"]["result"]["type"], "boolean")
        patch = by_name["arborist/patch_ast_node"]
        self.assertEqual(patch["metadata"]["category"], "write")
        self.assertTrue(patch["annotations"]["destructiveHint"])
        self.assertEqual(
            patch["inputSchema"]["properties"]["new_code"]["maxLength"],
            gateway_module.TEXT_PARAM_MAX_LENGTH,
        )
        self.assertEqual(
            patch["inputSchema"]["properties"]["bypass_reason"]["maxLength"],
            gateway_module.BYPASS_REASON_MAX_LENGTH,
        )
        patch_result = patch["outputSchema"]["properties"]["result"]
        self.assertEqual(patch_result["additionalProperties"], False)
        self.assertIn("validation", patch_result["required"])
        self.assertEqual(
            patch_result["properties"]["validation"]["properties"]["commit_gate"][
                "additionalProperties"
            ],
            False,
        )
        preview = by_name["arborist/preview_patch_ast_node"]
        preview_result = preview["outputSchema"]["properties"]["result"]
        self.assertEqual(preview_result["required"], ["patch", "unified_diff", "changed"])
        self.assertEqual(preview_result["properties"]["patch"], patch_result)
        replay = by_name["arborist/replay_patch_evidence_against_trace"]["outputSchema"][
            "properties"
        ]["result"]
        self.assertEqual(replay["required"], ["consistent", "matched_items", "blocked_items", "items"])
        self.assertEqual(replay["properties"]["items"]["items"]["additionalProperties"], False)
        trace_validation = by_name["arborist/validate_patch_commit_with_trace"][
            "outputSchema"
        ]["properties"]["result"]
        self.assertIn("replay", trace_validation["required"])
        trace_backed = by_name["arborist/validate_patch_with_trace_context"]["outputSchema"][
            "properties"
        ]["result"]
        self.assertEqual(
            trace_backed["required"],
            [
                "patch",
                "trace_target",
                "trace",
                "trace_validation",
                "impact",
                "trace_error",
            ],
        )
        self.assertEqual(
            trace_backed["properties"]["impact"]["anyOf"][0]["additionalProperties"],
            False,
        )
        graph_backed = by_name["arborist/validate_patch_with_graph_context"]["outputSchema"][
            "properties"
        ]["result"]
        self.assertIn("neighborhood", graph_backed["required"])
        discovery_backed = by_name["arborist/validate_patch_with_discovery_context"][
            "outputSchema"
        ]["properties"]["result"]
        self.assertIn("read", discovery_backed["required"])
        query = by_name["arborist/execute_tree_query"]
        self.assertNotIn("max_captures", query["inputSchema"]["required"])
        self.assertEqual(
            query["inputSchema"]["properties"]["max_captures"]["default"], 10000
        )
        self.assertEqual(query["inputSchema"]["properties"]["max_captures"]["minimum"], 1)
        self.assertEqual(
            query["inputSchema"]["properties"]["max_captures"]["maximum"],
            gateway_module.TREE_QUERY_MAX_CAPTURES,
        )
        self.assertEqual(
            query["inputSchema"]["properties"]["query"]["maxLength"],
            gateway_module.TREE_QUERY_MAX_LENGTH,
        )
        self.assertEqual(
            query["inputSchema"]["properties"]["timeout_ms"]["minimum"],
            1,
        )
        self.assertEqual(
            query["inputSchema"]["properties"]["timeout_ms"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        )
        query_items = query["outputSchema"]["properties"]["result"]["items"]
        self.assertEqual(query_items["additionalProperties"], False)
        self.assertIn("capture_name", query_items["required"])
        self.assertEqual(query_items["properties"]["start_point"]["properties"]["row"]["type"], "integer")
        self.assertEqual(
            query_items["properties"]["owner_symbol_id"]["anyOf"][1]["type"], "null"
        )
        trace_graph = by_name["arborist/trace_symbol_graph"]["outputSchema"]["properties"][
            "result"
        ]
        self.assertEqual(trace_graph["additionalProperties"], False)
        self.assertEqual(
            trace_graph["properties"]["symbol"]["properties"]["dependencies"]["type"], "array"
        )
        self.assertEqual(
            trace_graph["properties"]["evidence_keys"]["required"],
            ["symbol", "callers", "callees"],
        )
        trace_neighborhood = by_name["arborist/trace_symbol_neighborhood"]["outputSchema"][
            "properties"
        ]["result"]
        self.assertEqual(
            by_name["arborist/trace_symbol_neighborhood"]["inputSchema"]["properties"][
                "max_nodes"
            ]["minimum"],
            1,
        )
        self.assertEqual(
            by_name["arborist/trace_symbol_neighborhood"]["inputSchema"]["properties"][
                "max_nodes"
            ]["maximum"],
            gateway_module.MAX_GRAPH_NODES,
        )
        self.assertEqual(
            by_name["arborist/trace_symbol_neighborhood"]["inputSchema"]["properties"][
                "max_depth"
            ]["maximum"],
            gateway_module.MAX_GRAPH_DEPTH,
        )
        self.assertEqual(
            by_name["arborist/trace_symbol_graph"]["inputSchema"]["properties"][
                "timeout_ms"
            ]["minimum"],
            1,
        )
        self.assertEqual(
            by_name["arborist/trace_symbol_neighborhood_at_position"]["inputSchema"][
                "properties"
            ]["timeout_ms"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        )
        self.assertIn("nodes", trace_neighborhood["required"])
        self.assertEqual(
            trace_neighborhood["properties"]["nodes"]["items"]["properties"]["depth"]["type"],
            "integer",
        )
        read_symbol = by_name["arborist/read_symbol"]["outputSchema"]["properties"]["result"]
        self.assertEqual(
            read_symbol["required"], ["indexed_files", "symbol", "source", "start_point", "end_point"]
        )
        self.assertEqual(read_symbol["properties"]["symbol"]["additionalProperties"], False)
        list_symbols = by_name["arborist/list_symbols"]["outputSchema"]["properties"]["result"]
        self.assertEqual(
            by_name["arborist/list_symbols"]["inputSchema"]["properties"]["limit"]["maximum"],
            gateway_module.MAX_SYMBOL_LIMIT,
        )
        self.assertEqual(
            list_symbols["required"], ["indexed_files", "total_symbols", "truncated", "symbols"]
        )
        search_symbols = by_name["arborist/search_symbols"]["outputSchema"]["properties"][
            "result"
        ]
        self.assertEqual(search_symbols["properties"]["match_details"]["type"], "array")
        search_context = by_name["arborist/search_symbols_discovery_context"]["outputSchema"][
            "properties"
        ]["result"]
        self.assertEqual(search_context["required"], ["search", "reads", "contexts"])

    def test_resources_list_exposes_tool_catalog(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(self.make_gateway(), "resources/list", {}, request_id=57),
            request_id=57,
        )

        self.assertEqual(result, {"resources": gateway_module.build_resource_catalog()})

    def test_resources_read_returns_tool_catalog_snapshot(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "resources/read",
                {"uri": gateway_module.TOOL_CATALOG_RESOURCE_URI},
                request_id=58,
            ),
            request_id=58,
        )

        contents = result["contents"]
        self.assertEqual(len(contents), 1)
        self.assertEqual(contents[0]["uri"], gateway_module.TOOL_CATALOG_RESOURCE_URI)
        self.assertEqual(contents[0]["mimeType"], "application/json")
        catalog = gateway_module.json.loads(contents[0]["text"])
        self.assertEqual(catalog, gateway_module.build_tool_catalog())

    def test_resources_read_rejects_unknown_resource(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "resources/read",
            {"uri": "arborist://missing"},
            request_id=59,
        )

        self.assert_jsonrpc_error(
            response, request_id=59, code=-32602, contains="unknown resource"
        )

    def test_tools_call_invokes_read_tool(self) -> None:
        core = make_recording_json_core(get_semantic_skeleton_json={"kind": "module"})

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "tools/call",
                {
                    "name": "arborist/get_semantic_skeleton",
                    "arguments": {"file_path": "sample.py"},
                },
                request_id=103,
            ),
            request_id=103,
        )

        assert isinstance(result, dict)
        self.assertFalse(result["isError"])
        self.assertEqual(result["structuredContent"]["result"], {"kind": "module"})
        self.assertEqual(core.calls_for("get_semantic_skeleton_json"), [("sample.py", None, 2, None)])

    def test_tools_call_invokes_write_tool(self) -> None:
        core = make_recording_json_core(patch_ast_node_json={"patched": True})

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "tools/call",
                {
                    "name": "arborist/patch_ast_node",
                    "arguments": {
                        "file_path": "sample.py",
                        "semantic_path": "top_level",
                        "new_code": "def top_level():\n    return 1\n",
                    },
                },
                request_id=104,
            ),
            request_id=104,
        )

        assert isinstance(result, dict)
        self.assertFalse(result["isError"])
        self.assertEqual(result["structuredContent"]["result"], {"patched": True})
        self.assertEqual(
            core.calls_for("patch_ast_node_json"),
            [("sample.py", "top_level", "def top_level():\n    return 1\n", None, None)],
        )

    def test_tools_call_invokes_index_tool(self) -> None:
        core = make_recording_json_core(register_symbol_index_json={"registered": True})

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "tools/call",
                {
                    "name": "arborist/register_symbol_index",
                    "arguments": {"workspace_root": ".", "db_path": "symbols.db"},
                },
                request_id=105,
            ),
            request_id=105,
        )

        assert isinstance(result, dict)
        self.assertFalse(result["isError"])
        self.assertEqual(result["structuredContent"]["result"], {"registered": True})
        self.assertEqual(core.calls_for("register_symbol_index_json"), [(".", "symbols.db")])

    def test_tools_call_invokes_trace_tool(self) -> None:
        core = make_recording_json_core(trace_symbol_graph_json={"symbol": "top_level"})

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "tools/call",
                {
                    "name": "arborist/trace_symbol_graph",
                    "arguments": {"workspace_root": ".", "symbol_path": "top_level"},
                },
                request_id=106,
            ),
            request_id=106,
        )

        assert isinstance(result, dict)
        self.assertFalse(result["isError"])
        self.assertEqual(result["structuredContent"]["result"], {"symbol": "top_level"})
        self.assertEqual(
            core.calls_for("trace_symbol_graph_json"),
            [(".", "top_level", "both", None, None, None, None)],
        )

    def test_tools_call_invokes_read_only_batch(self) -> None:
        core = make_recording_json_core(
            get_semantic_skeleton_json={"kind": "module"},
            trace_symbol_graph_json={"symbol": "top_level"},
        )

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "tools/call",
                {
                    "name": "arborist/batch",
                    "arguments": {
                        "calls": [
                            {
                                "name": "arborist/get_semantic_skeleton",
                                "arguments": {"file_path": "sample.py"},
                            },
                            {
                                "name": "arborist/trace_symbol_graph",
                                "arguments": {
                                    "workspace_root": ".",
                                    "symbol_path": "top_level",
                                },
                            },
                        ]
                    },
                },
                request_id=113,
            ),
            request_id=113,
        )

        assert isinstance(result, dict)
        self.assertFalse(result["isError"])
        self.assertEqual(
            result["structuredContent"]["result"],
            [
                {
                    "name": "arborist/get_semantic_skeleton",
                    "result": {"kind": "module"},
                },
                {
                    "name": "arborist/trace_symbol_graph",
                    "result": {"symbol": "top_level"},
                },
            ],
        )
        self.assertEqual(core.calls_for("get_semantic_skeleton_json"), [("sample.py", None, 2, None)])
        self.assertEqual(
            core.calls_for("trace_symbol_graph_json"),
            [(".", "top_level", "both", None, None, None, None)],
        )

    def test_batch_rejects_write_tool(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "tools/call",
                {
                    "name": "arborist/batch",
                    "arguments": {
                        "calls": [
                            {
                                "name": "arborist/patch_ast_node",
                                "arguments": {
                                    "file_path": "sample.py",
                                    "semantic_path": "top_level",
                                    "new_code": "def top_level():\n    return 1\n",
                                },
                            }
                        ]
                    },
                },
                request_id=114,
            ),
            request_id=114,
        )

        assert isinstance(result, dict)
        self.assertTrue(result["isError"])
        self.assertIn("batch only supports read-only tools", result["content"][0]["text"])

    def test_batch_rejects_unknown_tool(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "tools/call",
                {
                    "name": "arborist/batch",
                    "arguments": {
                        "calls": [
                            {"name": "arborist/missing", "arguments": {}},
                        ]
                    },
                },
                request_id=115,
            ),
            request_id=115,
        )

        assert isinstance(result, dict)
        self.assertTrue(result["isError"])
        self.assertIn("unknown batch tool", result["content"][0]["text"])

    def test_batch_rejects_nested_batch(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "tools/call",
                {
                    "name": "arborist/batch",
                    "arguments": {
                        "calls": [
                            {"name": "arborist/batch", "arguments": {"calls": []}},
                        ]
                    },
                },
                request_id=116,
            ),
            request_id=116,
        )

        assert isinstance(result, dict)
        self.assertTrue(result["isError"])
        self.assertIn("may not include arborist/batch", result["content"][0]["text"])

    def test_tools_call_rejects_unknown_tool(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "tools/call",
            {"name": "arborist/missing", "arguments": {}},
            request_id=107,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=107,
            code=-32602,
            contains="unknown tool",
        )

    def test_tools_call_reports_missing_tool_argument_as_tool_error(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "tools/call",
                {"name": "arborist/get_semantic_skeleton", "arguments": {}},
                request_id=108,
            ),
            request_id=108,
        )

        assert isinstance(result, dict)
        self.assertTrue(result["isError"])
        self.assertIn("missing required string param: file_path", result["content"][0]["text"])

    def test_tools_call_rejects_non_object_arguments(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "tools/call",
            {"name": "arborist/get_semantic_skeleton", "arguments": []},
            request_id=109,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=109,
            code=-32602,
            contains="arguments must be an object",
        )

    def test_tools_call_reports_argument_type_error_as_tool_error(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "tools/call",
                {
                    "name": "arborist/get_semantic_skeleton",
                    "arguments": {"file_path": "sample.py", "depth_limit": "two"},
                },
                request_id=110,
            ),
            request_id=110,
        )

        assert isinstance(result, dict)
        self.assertTrue(result["isError"])
        self.assertIn("invalid int param: depth_limit", result["content"][0]["text"])

    def test_rejects_nonstandard_json_from_core(self) -> None:
        class StubCore:
            def list_symbol_indexes_json(self) -> str:
                return '[{"workspace_root": NaN}]'

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/list_symbol_indexes",
            {},
            request_id=34,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=34,
            code=-32000,
            contains="invalid JSON from arborist core",
        )
        self.assertIn("non-standard JSON constant", response["error"]["message"])

    def test_rejects_malformed_json_from_core(self) -> None:
        class StubCore:
            def list_symbol_indexes_json(self) -> str:
                return '[{"workspace_root": "."}'

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/list_symbol_indexes",
            {},
            request_id=35,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=35,
            code=-32000,
            contains="invalid JSON from arborist core",
        )

    def test_rejects_duplicate_json_keys_from_core(self) -> None:
        class StubCore:
            def list_symbol_indexes_json(self) -> str:
                return '[{"workspace_root": "a", "workspace_root": "b"}]'

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/list_symbol_indexes",
            {},
            request_id=50,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=50,
            code=-32000,
            contains="invalid JSON from arborist core",
        )
        self.assertIn("duplicate JSON object key", response["error"]["message"])

    def test_rejects_object_core_payload_with_wrong_shape(self) -> None:
        class StubCore:
            def get_semantic_skeleton_json(self, *args: object) -> str:
                return "[]"

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/get_semantic_skeleton",
            {"file_path": "sample.py"},
            request_id=52,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=52,
            code=-32000,
            contains="invalid JSON from arborist core",
        )
        self.assertIn("expected object", response["error"]["message"])

    def test_rejects_list_core_payload_with_wrong_shape(self) -> None:
        class StubCore:
            def list_symbol_indexes_json(self) -> str:
                return "{}"

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/list_symbol_indexes",
            {},
            request_id=53,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=53,
            code=-32000,
            contains="invalid JSON from arborist core",
        )
        self.assertIn("expected array", response["error"]["message"])

    def test_rejects_list_core_payload_with_non_object_items(self) -> None:
        class StubCore:
            def execute_tree_query_json(self, *args: object) -> str:
                return "[null]"

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/execute_tree_query",
            {"file_path": "sample.py", "query": "(module) @module"},
            request_id=54,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=54,
            code=-32000,
            contains="invalid JSON from arborist core",
        )
        self.assertIn("expected object item", response["error"]["message"])

    def test_execute_tree_query_passes_capture_limit_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.args: tuple[object, ...] | None = None

            def execute_tree_query_json(self, *args: object) -> str:
                self.args = args
                return "[]"

        core = StubCore()
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "arborist/execute_tree_query",
                {
                    "file_path": "sample.py",
                    "query": "(module) @module",
                    "max_captures": 7,
                    "timeout_ms": 2500,
                },
                request_id=55,
            ),
            request_id=55,
        )

        self.assertEqual(result, [])
        self.assertEqual(core.args, ("sample.py", "(module) @module", None, 7, 2500))

    def test_execute_tree_query_rejects_invalid_timeout(self) -> None:
        class StubCore:
            def execute_tree_query_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
            with self.subTest(timeout_ms=timeout_ms):
                response = self.call_gateway(
                    self.make_gateway(StubCore()),
                    "arborist/execute_tree_query",
                    {
                        "file_path": "sample.py",
                        "query": "(module) @module",
                        "timeout_ms": timeout_ms,
                    },
                    request_id=58 + timeout_ms,
                )

                self.assert_jsonrpc_error(
                    response,
                    request_id=58 + timeout_ms,
                    code=-32602,
                    contains="timeout_ms",
                )

    def test_execute_tree_query_rejects_zero_capture_limit(self) -> None:
        class StubCore:
            def execute_tree_query_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/execute_tree_query",
            {
                "file_path": "sample.py",
                "query": "(module) @module",
                "max_captures": 0,
            },
            request_id=56,
        )

        self.assert_jsonrpc_error(
            response, request_id=56, code=-32602, contains="max_captures"
        )

    def test_execute_tree_query_rejects_oversized_query_before_core(self) -> None:
        class StubCore:
            def execute_tree_query_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/execute_tree_query",
            {
                "file_path": "sample.py",
                "query": "(" * (gateway_module.TREE_QUERY_MAX_LENGTH + 1),
            },
            request_id=57,
        )

        self.assert_jsonrpc_error(response, request_id=57, code=-32602, contains="query")
        self.assertIn("max length", response["error"]["message"])

    def test_execute_tree_query_preserves_owner_metadata_from_core(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_live_gateway(),
                "arborist/execute_tree_query",
                {
                    "file_path": "tests/fixtures/sample.py",
                    "source": "@logged\ndef top_level(value):\n    return value\n",
                    "query": "(decorator (identifier) @decorator)",
                },
                request_id=23,
            ),
            request_id=23,
        )

        assert isinstance(result, list)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["capture_name"], "decorator")
        self.assertEqual(result[0]["text"], "logged")
        self.assertEqual(result[0]["owner_symbol_id"], "top_level")
        self.assertEqual(result[0]["owner_semantic_path"], "top_level")
        self.assertIsNone(result[0]["owner_scope_path"])

    def test_gateway_initialization_loads_core_lazily(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.loaded = True

        with mock.patch.object(gateway_module, "_load_core_class", return_value=StubCore) as loader:
            gateway = self.make_live_gateway()
            self.assertIsNone(gateway._core)
            loader.assert_not_called()
            self.assertIsInstance(gateway._require_core(), StubCore)
            loader.assert_called_once()

    def test_require_core_handles_new_only_gateway_instance(self) -> None:
        class StubCore:
            pass

        gateway = self.make_gateway()

        with mock.patch.object(gateway_module, "_load_core_class", return_value=StubCore):
            core = gateway._require_core()

        self.assertIsInstance(core, StubCore)
        self.assertIs(gateway._core, core)

    def test_initialize_reports_core_load_failure_as_jsonrpc_error(self) -> None:
        gateway = self.make_live_gateway()

        with mock.patch.object(gateway_module, "_load_core_class", side_effect=ImportError("boom")):
            response = gateway.handle_request(
                {"jsonrpc": "2.0", "id": 24, "method": "initialize", "params": {}}
            )

        self.assert_jsonrpc_error(
            response,
            request_id=24,
            code=-32000,
            contains="failed to load arborist core",
        )

    def test_once_valid_request_with_core_load_failure_prints_error_response(self) -> None:
        with mock.patch.object(gateway_module, "_load_core_class", side_effect=ImportError("boom")):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"jsonrpc":"2.0","id":25,"method":"initialize","params":{}}',
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 25)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to load arborist core", response["error"]["message"])

    def test_once_valid_request_prints_success_response(self) -> None:
        class StubCore:
            def supported_languages(self) -> list[str]:
                return ["python", "c"]

        with mock.patch.object(gateway_module, "_load_core_class", return_value=StubCore):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"jsonrpc":"2.0","id":26,"method":"initialize","params":{}}',
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 26)
        self.assertEqual(response["result"]["serverInfo"]["version"], gateway_module.__version__)
        self.assertEqual(response["result"]["supportedLanguages"], ["python", "c"])
        self.assertEqual(
            response["result"]["capabilities"]["tools"],
            list(gateway_module.TOOL_NAMES),
        )

    def test_stdio_notification_does_not_emit_response(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": None, "result": {"ok": True}}

        fake_gateway = StubGateway()
        stdin = io.StringIO(
            '{"jsonrpc":"2.0","method":"arborist/list_symbol_indexes","params":{}}\n'
        )
        stdout = io.StringIO()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        self.assertEqual(
            fake_gateway.request,
            {
                "jsonrpc": "2.0",
                "method": "arborist/list_symbol_indexes",
                "params": {},
            },
        )
        self.assertEqual(stdout.getvalue(), "")

    def test_once_notification_does_not_print_response(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": None, "result": {"ok": True}}

        fake_gateway = StubGateway()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"jsonrpc":"2.0","method":"arborist/list_symbol_indexes","params":{}}',
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        self.assertEqual(
            fake_gateway.request,
            {
                "jsonrpc": "2.0",
                "method": "arborist/list_symbol_indexes",
                "params": {},
            },
        )
        mock_print.assert_not_called()

    def test_stdio_invalid_json_emits_parse_error_response(self) -> None:
        stdin = io.StringIO("{bad json}\n")
        stdout = io.StringIO()

        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("invalid JSON", response["error"]["message"])

    def test_stdio_duplicate_json_key_emits_parse_error_response(self) -> None:
        stdin = io.StringIO(
            '{"jsonrpc":"2.0","id":1,"id":2,"method":"initialize","params":{}}\n'
        )
        stdout = io.StringIO()

        with mock.patch.object(
            gateway_module.ArboristGateway,
            "__init__",
            side_effect=AssertionError("gateway should not initialize"),
        ):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("duplicate JSON object key", response["error"]["message"])
        self.assertIn("id", response["error"]["message"])

    def test_parse_request_rejects_nested_duplicate_json_key(self) -> None:
        request, response = gateway_module.parse_request_json(
            '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"x":1,"x":2}}'
        )

        self.assertIsNone(request)
        self.assertIsNotNone(response)
        assert response is not None
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("duplicate JSON object key", response["error"]["message"])
        self.assertIn("x", response["error"]["message"])

    def test_stdio_invalid_no_id_request_emits_error_response(self) -> None:
        stdin = io.StringIO('{"method":"arborist/list_symbol_indexes","params":{}}\n')
        stdout = io.StringIO()

        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("jsonrpc", response["error"]["message"])

    def test_stdio_invalid_json_does_not_initialize_gateway(self) -> None:
        stdin = io.StringIO("{bad json}\n")
        stdout = io.StringIO()

        with mock.patch.object(
            gateway_module.ArboristGateway,
            "__init__",
            side_effect=AssertionError("gateway should not initialize"),
        ):
            with mock.patch.object(
                gateway_module,
                "_load_core_class",
                side_effect=AssertionError("core loader should not run"),
            ):
                with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                    exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["error"]["code"], -32700)

    def test_once_invalid_json_prints_parse_error_response(self) -> None:
        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch("pathlib.Path.read_text", return_value="{bad json}"):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("invalid JSON", response["error"]["message"])

    def test_once_invalid_no_id_request_prints_error_response(self) -> None:
        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"method":"arborist/list_symbol_indexes","params":{}}',
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("jsonrpc", response["error"]["message"])

    def test_once_invalid_json_does_not_initialize_gateway(self) -> None:
        with mock.patch.object(
            gateway_module.ArboristGateway,
            "__init__",
            side_effect=AssertionError("gateway should not initialize"),
        ):
            with mock.patch.object(
                gateway_module,
                "_load_core_class",
                side_effect=AssertionError("core loader should not run"),
            ):
                with mock.patch("pathlib.Path.read_text", return_value="{bad json}"):
                    with mock.patch("builtins.print") as mock_print:
                        exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["error"]["code"], -32700)

    def test_once_missing_request_file_reports_cli_error(self) -> None:
        stderr = io.StringIO()

        with mock.patch.object(
            gateway_module.ArboristGateway,
            "__init__",
            side_effect=AssertionError("gateway should not initialize"),
        ):
            with mock.patch.object(
                gateway_module,
                "_load_core_class",
                side_effect=AssertionError("core loader should not run"),
            ):
                with mock.patch(
                    "pathlib.Path.read_text",
                    side_effect=FileNotFoundError("missing request file"),
                ):
                    with mock.patch("sys.stderr", stderr):
                        exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 1)
        self.assertIn("failed to read request file", stderr.getvalue())
        self.assertIn("dummy.json", stderr.getvalue())

    def test_once_invalid_request_encoding_reports_cli_error(self) -> None:
        stderr = io.StringIO()

        with mock.patch(
            "pathlib.Path.read_text",
            side_effect=UnicodeDecodeError("utf-8", b"\xff", 0, 1, "invalid start byte"),
        ):
            with mock.patch("sys.stderr", stderr):
                exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 1)
        self.assertIn("failed to read request file", stderr.getvalue())
        self.assertIn("dummy.json", stderr.getvalue())

    def test_dump_tool_catalog_prints_generated_catalog(self) -> None:
        with mock.patch("builtins.print") as mock_print:
            exit_code = gateway_module.main(["--dump-tool-catalog"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        payload = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(len(payload), len(gateway_module.TOOL_NAMES))
        by_name = {tool["name"]: tool for tool in payload}
        self.assertIn("arborist/get_semantic_skeleton", by_name)
        self.assertEqual(
            by_name["arborist/list_symbol_indexes"]["outputSchema"]["properties"]["result"]["type"],
            "array",
        )

    def test_stdio_broken_pipe_exits_cleanly(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": 1, "result": {"ok": True}}

        class BrokenStdout(io.StringIO):
            def write(self, text: str) -> int:
                raise BrokenPipeError("pipe closed")

        fake_gateway = StubGateway()
        stdin = io.StringIO('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n')
        stdout = BrokenStdout()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)

    def test_stdio_nonstandard_response_value_emits_internal_error(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": 31, "result": {"value": float("nan")}}

        fake_gateway = StubGateway()
        stdin = io.StringIO('{"jsonrpc":"2.0","id":31,"method":"initialize","params":{}}\n')
        stdout = io.StringIO()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 31)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to serialize response", response["error"]["message"])

    def test_stdio_unserializable_response_id_falls_back_to_null(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": object(), "result": {"value": object()}}

        fake_gateway = StubGateway()
        stdin = io.StringIO('{"jsonrpc":"2.0","id":33,"method":"initialize","params":{}}\n')
        stdout = io.StringIO()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to serialize response", response["error"]["message"])

    def test_once_broken_pipe_exits_cleanly(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": 1, "result": {"ok": True}}

        fake_gateway = StubGateway()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}',
            ):
                with mock.patch("builtins.print", side_effect=BrokenPipeError("pipe closed")):
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)

    def test_once_nonstandard_response_value_prints_internal_error(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": 32, "result": {"value": float("inf")}}

        fake_gateway = StubGateway()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"jsonrpc":"2.0","id":32,"method":"initialize","params":{}}',
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 32)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to serialize response", response["error"]["message"])

    def test_stdio_rejects_nan_as_parse_error(self) -> None:
        stdin = io.StringIO(
            '{"jsonrpc":"2.0","id":NaN,"method":"arborist/list_symbol_indexes","params":{}}\n'
        )
        stdout = io.StringIO()

        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("non-standard JSON constant", response["error"]["message"])

    def test_once_rejects_infinity_as_parse_error(self) -> None:
        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"jsonrpc":"2.0","id":Infinity,"method":"initialize","params":{}}',
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("non-standard JSON constant", response["error"]["message"])
