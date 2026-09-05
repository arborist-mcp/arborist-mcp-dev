from __future__ import annotations

from unittest import mock

from arborist_mcp import gateway as gateway_module


class GatewayRuntimeCatalogTestsMixin:
    def test_live_initialize_reports_builtin_language_support(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(self.make_live_gateway(), "initialize", {}, request_id=0),
            request_id=0,
        )

        assert isinstance(result, dict)
        self.assertEqual(
            result["supportedLanguages"],
            [
                "python",
                "c",
                "cpp",
                "csharp",
                "javascript",
                "typescript",
                "tsx",
                "rust",
                "go",
                "java",
                "kotlin",
                "lua",
                "php",
                "swift",
            ],
        )

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
        self.assertEqual(
            [
                spec.name
                for spec in gateway_module.TOOL_SPECS
                if "timeout_ms" not in spec.params
            ],
            [],
        )
        self.assertTrue(gateway_module.TOOL_PARAM_SPECS["timeout_ms"].optional)
        batch = by_name["arborist/batch"]
        self.assertEqual(batch["metadata"]["category"], "read")
        self.assertTrue(batch["annotations"]["readOnlyHint"])
        self.assertFalse(batch["metadata"]["mutatesState"])
        self.assertEqual(batch["inputSchema"]["required"], ["calls"])
        self.assertEqual(
            batch["inputSchema"]["properties"]["calls"]["maxItems"],
            gateway_module.MAX_BATCH_CALLS,
        )
        batch_timeout_schema = batch["inputSchema"]["properties"]["timeout_ms"]
        self.assertEqual(batch_timeout_schema["minimum"], 1)
        self.assertEqual(
            batch_timeout_schema["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
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
        self.assertNotIn("timeout_ms", skeleton["inputSchema"]["required"])
        self.assertEqual(
            skeleton["inputSchema"]["properties"]["timeout_ms"]["minimum"],
            1,
        )
        self.assertEqual(
            skeleton["inputSchema"]["properties"]["timeout_ms"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        )
        self.assertIn(
            "Tree-sitter C++ grammar",
            skeleton["inputSchema"]["properties"]["file_path"]["description"],
        )
        list_indexes = by_name["arborist/list_symbol_indexes"]
        self.assertEqual(list_indexes["inputSchema"]["required"], [])
        self.assertIn("timeout_ms", list_indexes["inputSchema"]["properties"])
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
        self.assertEqual(
            by_name["arborist/did_change"]["inputSchema"]["properties"]["edits"]["maxItems"],
            gateway_module.MAX_POSITION_EDITS,
        )
        self.assertEqual(
            by_name["arborist/get_semantic_skeleton"]["inputSchema"]["properties"][
                "expand_nodes"
            ]["maxItems"],
            gateway_module.MAX_SEMANTIC_EXPAND_NODES,
        )
        self.assertEqual(
            by_name["arborist/preview_workspace_position_edits"]["inputSchema"]["properties"][
                "files"
            ]["maxItems"],
            gateway_module.MAX_WORKSPACE_EDIT_PREVIEW_FILES,
        )
        self.assertEqual(
            by_name["arborist/preview_workspace_position_edits"]["inputSchema"]["properties"][
                "files"
            ]["items"]["properties"]["source"]["maxLength"],
            gateway_module.TEXT_PARAM_MAX_LENGTH,
        )
        self.assertEqual(
            by_name["arborist/preview_workspace_position_edits"]["inputSchema"]["properties"][
                "files"
            ]["items"]["properties"]["edits"]["maxItems"],
            gateway_module.MAX_POSITION_EDITS,
        )
        workspace_preview_schema = by_name["arborist/preview_workspace_position_edits"][
            "inputSchema"
        ]
        self.assertNotIn("timeout_ms", workspace_preview_schema["required"])
        self.assertEqual(workspace_preview_schema["properties"]["timeout_ms"]["minimum"], 1)
        self.assertEqual(
            workspace_preview_schema["properties"]["timeout_ms"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        )
        inspect_index = by_name["arborist/inspect_symbol_index"]
        self.assertTrue(inspect_index["annotations"]["readOnlyHint"])
        self.assertFalse(inspect_index["metadata"]["mutatesState"])
        self.assertEqual(
            inspect_index["inputSchema"]["properties"]["timeout_ms"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        )
        migrate_index = by_name["arborist/migrate_symbol_index"]
        self.assertFalse(migrate_index["annotations"]["readOnlyHint"])
        self.assertTrue(migrate_index["metadata"]["mutatesState"])
        migrate_index_schema = migrate_index["inputSchema"]
        self.assertNotIn("timeout_ms", migrate_index_schema["required"])
        self.assertEqual(migrate_index_schema["properties"]["timeout_ms"]["minimum"], 1)
        self.assertEqual(
            migrate_index_schema["properties"]["timeout_ms"]["maximum"],
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
        for patch_name in (
            "arborist/patch_ast_node",
            "arborist/patch_ast_node_at_position",
            "arborist/patch_virtual_ast_node",
            "arborist/patch_virtual_ast_node_at_position",
        ):
            patch_timeout = by_name[patch_name]["inputSchema"]
            self.assertNotIn("timeout_ms", patch_timeout["required"])
            self.assertEqual(patch_timeout["properties"]["timeout_ms"]["minimum"], 1)
            self.assertEqual(
                patch_timeout["properties"]["timeout_ms"]["maximum"],
                gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
            )
        for lifecycle_tool_name in (
            "arborist/did_open",
            "arborist/did_change",
            "arborist/read_virtual_file",
            "arborist/list_virtual_files",
            "arborist/apply_buffer_edit",
            "arborist/commit_virtual_file",
            "arborist/discard_virtual_file",
            "arborist/did_close",
        ):
            lifecycle_timeout = by_name[lifecycle_tool_name]["inputSchema"]
            self.assertNotIn("timeout_ms", lifecycle_timeout["required"])
            self.assertEqual(
                lifecycle_timeout["properties"]["timeout_ms"]["minimum"],
                1,
            )
            self.assertEqual(
                lifecycle_timeout["properties"]["timeout_ms"]["maximum"],
                gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
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
        for preview_name in (
            "arborist/preview_patch_ast_node",
            "arborist/preview_patch_ast_node_at_position",
        ):
            preview_timeout = by_name[preview_name]["inputSchema"]
            self.assertNotIn("timeout_ms", preview_timeout["required"])
            self.assertEqual(preview_timeout["properties"]["timeout_ms"]["minimum"], 1)
            self.assertEqual(
                preview_timeout["properties"]["timeout_ms"]["maximum"],
                gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
            )
        preview_result = preview["outputSchema"]["properties"]["result"]
        self.assertEqual(preview_result["required"], ["patch", "unified_diff", "changed"])
        self.assertEqual(preview_result["properties"]["patch"], patch_result)
        for offline_analysis_name in (
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
            "arborist/export_patch_diagnostics_sarif",
        ):
            offline_analysis_timeout = by_name[offline_analysis_name]["inputSchema"]
            self.assertNotIn("timeout_ms", offline_analysis_timeout["required"])
            self.assertEqual(
                offline_analysis_timeout["properties"]["timeout_ms"]["minimum"],
                1,
            )
            self.assertEqual(
                offline_analysis_timeout["properties"]["timeout_ms"]["maximum"],
                gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
            )
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
        for patch_tool_name in (
            "arborist/validate_patch_with_trace_context",
            "arborist/validate_patch_with_trace_context_at_position",
            "arborist/validate_patch_with_graph_context",
            "arborist/validate_patch_with_graph_context_at_position",
            "arborist/validate_patch_with_neighborhood_context",
            "arborist/validate_patch_with_neighborhood_context_at_position",
            "arborist/validate_patch_with_discovery_context",
            "arborist/validate_patch_with_discovery_context_at_position",
        ):
            patch_timeout = by_name[patch_tool_name]["inputSchema"]
            self.assertNotIn("timeout_ms", patch_timeout["required"])
            self.assertEqual(patch_timeout["properties"]["timeout_ms"]["minimum"], 1)
            self.assertEqual(
                patch_timeout["properties"]["timeout_ms"]["maximum"],
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
        for list_tool_name in (
            "arborist/list_symbols",
            "arborist/list_symbols_context",
            "arborist/list_symbols_neighborhood_context",
            "arborist/list_symbols_discovery_context",
        ):
            list_timeout = by_name[list_tool_name]["inputSchema"]
            self.assertNotIn("timeout_ms", list_timeout["required"])
            self.assertEqual(list_timeout["properties"]["timeout_ms"]["minimum"], 1)
            self.assertEqual(
                list_timeout["properties"]["timeout_ms"]["maximum"],
                gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
            )
        search_timeout = by_name["arborist/search_symbols"]["inputSchema"]
        self.assertNotIn("timeout_ms", search_timeout["required"])
        self.assertEqual(search_timeout["properties"]["timeout_ms"]["minimum"], 1)
        self.assertEqual(
            search_timeout["properties"]["timeout_ms"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        )
        for read_tool_name in (
            "arborist/read_symbol",
            "arborist/read_symbol_at_position",
            "arborist/read_symbol_context",
            "arborist/read_symbol_context_at_position",
            "arborist/read_symbol_neighborhood_context",
            "arborist/read_symbol_neighborhood_context_at_position",
            "arborist/read_symbol_discovery_context",
            "arborist/read_symbol_discovery_context_at_position",
        ):
            read_timeout = by_name[read_tool_name]["inputSchema"]
            self.assertNotIn("timeout_ms", read_timeout["required"])
            self.assertEqual(read_timeout["properties"]["timeout_ms"]["minimum"], 1)
            self.assertEqual(
                read_timeout["properties"]["timeout_ms"]["maximum"],
                gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
            )
        search_context_timeout = by_name["arborist/search_symbols_context"]["inputSchema"]
        self.assertNotIn("timeout_ms", search_context_timeout["required"])
        self.assertEqual(search_context_timeout["properties"]["timeout_ms"]["minimum"], 1)
        self.assertEqual(
            search_context_timeout["properties"]["timeout_ms"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        )
        neighborhood_timeout = by_name[
            "arborist/search_symbols_neighborhood_context"
        ]["inputSchema"]
        self.assertNotIn("timeout_ms", neighborhood_timeout["required"])
        self.assertEqual(neighborhood_timeout["properties"]["timeout_ms"]["minimum"], 1)
        self.assertEqual(
            neighborhood_timeout["properties"]["timeout_ms"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        )
        discovery_timeout = by_name["arborist/search_symbols_discovery_context"][
            "inputSchema"
        ]
        self.assertNotIn("timeout_ms", discovery_timeout["required"])
        self.assertEqual(discovery_timeout["properties"]["timeout_ms"]["minimum"], 1)
        self.assertEqual(
            discovery_timeout["properties"]["timeout_ms"]["maximum"],
            gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS,
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

    def test_resources_read_rejects_nonstandard_catalog_json(self) -> None:
        with mock.patch(
            "arborist_mcp.resources.build_tool_catalog",
            return_value=[{"invalid": float("nan")}],
        ):
            response = self.call_gateway(
                self.make_gateway(),
                "resources/read",
                {"uri": gateway_module.TOOL_CATALOG_RESOURCE_URI},
                request_id=581,
            )

        self.assert_jsonrpc_error(
            response,
            request_id=581,
            code=-32602,
            contains="Out of range float values",
        )

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
