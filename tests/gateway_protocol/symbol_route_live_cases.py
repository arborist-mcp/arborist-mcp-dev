from __future__ import annotations


class GatewaySymbolRouteLiveTestsMixin:
    def test_trace_context_returns_trace_error_when_patch_gate_rejects(self) -> None:
        with self.temp_workspace(
            {
                "caller.py": "def orchestrate(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            caller = workspace.joinpath("caller.py")
            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_trace_context",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return missing_helper(value)\n"
                        ),
                        "direction": "both",
                    },
                    request_id=41,
                ),
                request_id=41,
            )

        assert isinstance(result, dict)
        self.assertFalse(result["patch"]["applied"])
        self.assertEqual(result["trace_target"], result["patch"]["resolved_symbol_id"])
        self.assertIsNone(result["trace"])
        self.assertIsNone(result["trace_validation"])
        self.assertEqual(
            result["trace_error"],
            "trace skipped because patch validation rejected the patch",
        )

    def test_trace_context_returns_trace_error_when_patch_has_syntax_errors(self) -> None:
        with self.temp_workspace(
            {
                "caller.py": "def orchestrate(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            caller = workspace.joinpath("caller.py")
            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_trace_context",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(\n"
                        ),
                        "direction": "both",
                    },
                    request_id=42,
                ),
                request_id=42,
            )

        assert isinstance(result, dict)
        self.assertFalse(result["patch"]["applied"])
        self.assertEqual(result["trace_target"], result["patch"]["resolved_symbol_id"])
        self.assertTrue(result["patch"]["validation"]["syntax_errors"])
        self.assertIsNone(result["trace"])
        self.assertIsNone(result["trace_validation"])
        self.assertEqual(
            result["trace_error"],
            "trace skipped because patch validation reported syntax errors",
        )

    def test_trace_context_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_trace_context",
                    {
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
                    request_id=43,
                ),
                request_id=43,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertTrue(result["patch"]["applied"])
            self.assertEqual(result["patch"]["file"], expected_file)
            self.assertEqual(result["trace_target"], result["patch"]["resolved_symbol_id"])
            self.assertIsNone(result["trace_error"])
            self.assertTrue(result["trace_validation"]["allowed"])
            self.assertEqual(result["trace_validation"]["replay_status"], "matched")
            self.assertEqual(result["trace"]["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["trace"]["symbol"]["file_path"], expected_file)
            self.assertTrue(
                any(symbol["semantic_path"] == "helper" for symbol in result["trace"]["callees"])
            )

    def test_graph_context_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "entry.py": (
                    "from caller import orchestrate\n\n\n"
                    "def entrypoint(value: int) -> int:\n"
                    "    return orchestrate(value)\n"
                ),
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_graph_context",
                    {
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
                    request_id=71,
                ),
                request_id=71,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertTrue(result["patch"]["applied"])
            self.assertEqual(result["patch"]["file"], expected_file)
            self.assertIsNone(result["trace_error"])
            self.assertTrue(result["trace_validation"]["allowed"])
            self.assertEqual(result["trace"]["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["trace"]["symbol"]["file_path"], expected_file)
            self.assertEqual(result["neighborhood"]["symbol"]["semantic_path"], "orchestrate")
            self.assertTrue(
                any(
                    node["symbol"]["semantic_path"] == "helper"
                    for node in result["neighborhood"]["nodes"]
                )
            )

    def test_neighborhood_context_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "entry.py": (
                    "from caller import orchestrate\n\n\n"
                    "def entrypoint(value: int) -> int:\n"
                    "    return orchestrate(value)\n"
                ),
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_neighborhood_context",
                    {
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
                    request_id=76,
                ),
                request_id=76,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertTrue(result["patch"]["applied"])
            self.assertEqual(result["trace"]["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(
                result["neighborhood_context"]["neighborhood"]["symbol"]["semantic_path"],
                "orchestrate",
            )
            self.assertTrue(
                any(
                    read["symbol"]["semantic_path"] == "helper"
                    for read in result["neighborhood_context"]["reads"]
                )
            )

    def test_discovery_context_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "entry.py": (
                    "from caller import orchestrate\n\n\n"
                    "def entrypoint(value: int) -> int:\n"
                    "    return orchestrate(value)\n"
                ),
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_discovery_context",
                    {
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
                    request_id=80,
                ),
                request_id=80,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertTrue(result["patch"]["applied"])
            self.assertEqual(result["trace"]["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["read"]["symbol"]["semantic_path"], "orchestrate")
            self.assertTrue(
                any(
                    read["symbol"]["semantic_path"] == "helper"
                    for read in result["neighborhood_context"]["reads"]
                )
            )

    def test_trace_context_accepts_index_db_path_with_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "caller.py": "def orchestrate(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            caller = workspace.joinpath("caller.py")
            db_path = workspace.joinpath("symbols.db")

            rebuild = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/rebuild_symbol_index",
                    {
                        "workspace_root": str(workspace),
                        "db_path": str(db_path),
                    },
                    request_id=180,
                ),
                request_id=180,
            )

            assert isinstance(rebuild, dict)
            self.assertEqual(rebuild["indexed_files"], 2)

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_trace_context",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller),
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
                        "index_db_path": str(db_path),
                    },
                    request_id=181,
                ),
                request_id=181,
            )

            assert isinstance(result, dict)
            self.assertTrue(result["patch"]["applied"])
            self.assertIsNone(result["trace_error"])
            self.assertTrue(result["trace_validation"]["allowed"])
            self.assertEqual(result["trace"]["symbol"]["semantic_path"], "orchestrate")
            self.assertTrue(
                any(symbol["semantic_path"] == "helper" for symbol in result["trace"]["callees"])
            )

    def test_trace_symbol_graph_accepts_index_db_path_with_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "caller.py": "def orchestrate(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            caller = workspace.joinpath("caller.py")
            db_path = workspace.joinpath("symbols.db")

            rebuild = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/rebuild_symbol_index",
                    {
                        "workspace_root": str(workspace),
                        "db_path": str(db_path),
                    },
                    request_id=182,
                ),
                request_id=182,
            )

            assert isinstance(rebuild, dict)
            self.assertEqual(rebuild["indexed_files"], 2)

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/trace_symbol_graph",
                    {
                        "workspace_root": str(workspace),
                        "symbol_path": "orchestrate",
                        "direction": "both",
                        "file_path": str(caller),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "index_db_path": str(db_path),
                    },
                    request_id=183,
                ),
                request_id=183,
            )

            assert isinstance(result, dict)
            self.assertEqual(result["symbol"]["semantic_path"], "orchestrate")
            self.assertTrue(
                any(symbol["semantic_path"] == "helper" for symbol in result["callees"])
            )
            self.assertIn("return value + 1", caller.read_text(encoding="utf-8"))

    def test_search_symbols_accepts_index_db_path_with_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper() -> int:\n    return 1\n",
            }
        ) as workspace:
            helper = workspace.joinpath("helper.py")
            db_path = workspace.joinpath("symbols.db")

            rebuild = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/rebuild_symbol_index",
                    {
                        "workspace_root": str(workspace),
                        "db_path": str(db_path),
                    },
                    request_id=184,
                ),
                request_id=184,
            )

            assert isinstance(rebuild, dict)
            self.assertEqual(rebuild["indexed_files"], 1)

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/search_symbols",
                    {
                        "workspace_root": str(workspace),
                        "query": "helper_alias",
                        "limit": 10,
                        "file_path": str(helper),
                        "source": (
                            "def helper() -> int:\n"
                            "    return 1\n\n\n"
                            "def helper_alias() -> int:\n"
                            "    return helper()\n"
                        ),
                        "index_db_path": str(db_path),
                    },
                    request_id=185,
                ),
                request_id=185,
            )

            assert isinstance(result, dict)
            self.assertEqual(result["total_matches"], 1)
            self.assertEqual(result["matches"][0]["semantic_path"], "helper_alias")
            self.assertNotIn("helper_alias", helper.read_text(encoding="utf-8"))

    def test_read_at_position_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/read_symbol_at_position",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "position": {"row": 3, "column": 5},
                    },
                    request_id=81,
                ),
                request_id=81,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["symbol"]["file_path"], expected_file)
            self.assertEqual(result["source"], "def orchestrate(value: int) -> int:\n    return helper(value)")

    def test_trace_graph_at_position_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/trace_symbol_graph_at_position",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "position": {"row": 3, "column": 5},
                        "direction": "both",
                    },
                    request_id=82,
                ),
                request_id=82,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["symbol"]["file_path"], expected_file)
            self.assertTrue(
                any(symbol["semantic_path"] == "helper" for symbol in result["callees"])
            )

    def test_discovery_context_at_position_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "entry.py": (
                    "from caller import orchestrate\n\n\n"
                    "def entrypoint(value: int) -> int:\n"
                    "    return orchestrate(value)\n"
                ),
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/read_symbol_discovery_context_at_position",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "position": {"row": 3, "column": 5},
                        "direction": "both",
                        "max_depth": 2,
                        "max_nodes": 10,
                    },
                    request_id=83,
                ),
                request_id=83,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["read"]["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["read"]["symbol"]["file_path"], expected_file)
            self.assertEqual(result["trace"]["symbol"]["file_path"], expected_file)
            self.assertTrue(
                any(
                    read["symbol"]["semantic_path"] == "helper"
                    for read in result["neighborhood_context"]["reads"]
                )
            )

    def test_patch_context_at_position_variants_accept_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            caller = workspace.joinpath("caller.py")
            source = (
                "from helper import helper\n\n\n"
                "def orchestrate(value: int) -> int:\n"
                "    return value + 1\n"
            )
            new_code = (
                "def orchestrate(value: int) -> int:\n"
                "    return helper(value)\n"
            )
            cases = (
                "arborist/validate_patch_with_trace_context_at_position",
                "arborist/validate_patch_with_graph_context_at_position",
                "arborist/validate_patch_with_neighborhood_context_at_position",
                "arborist/validate_patch_with_discovery_context_at_position",
            )
            for request_id, method in enumerate(cases, start=220):
                with self.subTest(method=method):
                    params = {
                        "workspace_root": str(workspace),
                        "file_path": str(caller),
                        "position": {"row": 3, "column": 5},
                        "new_code": new_code,
                        "source": source,
                        "direction": "both",
                    }
                    if method != "arborist/validate_patch_with_trace_context_at_position":
                        params["max_depth"] = 2
                        params["max_nodes"] = 10
                    result = self.assert_jsonrpc_ok(
                        self.call_gateway(
                            self.make_live_gateway(),
                            method,
                            params,
                            request_id=request_id,
                        ),
                        request_id=request_id,
                    )

                    assert isinstance(result, dict)
                    self.assertTrue(result["patch"]["applied"])
                    self.assertEqual(result["patch"]["file"], str(caller).replace("\\", "/"))
                    if method.endswith("trace_context_at_position"):
                        self.assertIsNotNone(result["impact"])
                    elif method.endswith("graph_context_at_position"):
                        self.assertIsNotNone(result["neighborhood"])
                    elif method.endswith("neighborhood_context_at_position"):
                        self.assertIsNotNone(result["neighborhood_context"])
                        self.assertNotIn("read", result)
                    else:
                        self.assertIsNotNone(result["read"])
                        self.assertIsNotNone(result["neighborhood_context"])

    def test_patch_context_index_variants_accept_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "caller.py": "def orchestrate(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            caller = workspace.joinpath("caller.py")
            db_path = workspace.joinpath("symbols.db")
            isolated_root = workspace.joinpath("isolated")
            isolated_root.mkdir()
            source = (
                "from helper import helper\n\n\n"
                "def orchestrate(value: int) -> int:\n"
                "    return value + 1\n"
            )
            new_code = (
                "def orchestrate(value: int) -> int:\n"
                "    return helper(value)\n"
            )
            rebuild = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/rebuild_symbol_index",
                    {
                        "workspace_root": str(workspace),
                        "db_path": str(db_path),
                    },
                    request_id=230,
                ),
                request_id=230,
            )
            assert isinstance(rebuild, dict)

            cases = (
                "arborist/validate_patch_with_graph_context",
                "arborist/validate_patch_with_neighborhood_context",
                "arborist/validate_patch_with_discovery_context",
            )
            for request_id, method in enumerate(cases, start=231):
                with self.subTest(method=method):
                    result = self.assert_jsonrpc_ok(
                        self.call_gateway(
                            self.make_live_gateway(),
                            method,
                            {
                                "workspace_root": str(isolated_root),
                                "file_path": str(caller),
                                "semantic_path": "orchestrate",
                                "new_code": new_code,
                                "source": source,
                                "direction": "both",
                                "max_depth": 2,
                                "max_nodes": 10,
                                "index_db_path": str(db_path),
                            },
                            request_id=request_id,
                        ),
                        request_id=request_id,
                    )

                    assert isinstance(result, dict)
                    self.assertTrue(result["patch"]["applied"])
                    self.assertTrue(result["trace_validation"]["allowed"])
                    if method.endswith("graph_context"):
                        self.assertIsNotNone(result["neighborhood"])
                    elif method.endswith("neighborhood_context"):
                        self.assertIsNotNone(result["neighborhood_context"])
                        self.assertNotIn("read", result)
                    else:
                        self.assertIsNotNone(result["read"])
                        self.assertIsNotNone(result["neighborhood_context"])

    def test_read_symbol_accepts_unsaved_source_with_file_anchor(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/read_symbol",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "symbol_path": "orchestrate",
                    },
                    request_id=84,
                ),
                request_id=84,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["symbol"]["file_path"], expected_file)
            self.assertEqual(
                result["source"],
                "def orchestrate(value: int) -> int:\n    return helper(value)",
            )

    def test_trace_symbol_graph_accepts_unsaved_source_with_file_anchor(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/trace_symbol_graph",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "symbol_path": "orchestrate",
                        "direction": "both",
                    },
                    request_id=85,
                ),
                request_id=85,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["symbol"]["file_path"], expected_file)
            self.assertTrue(
                any(symbol["semantic_path"] == "helper" for symbol in result["callees"])
            )

    def test_list_symbols_file_target_skips_unrelated_workspace_parse(self) -> None:
        with self.temp_workspace(
            {
                "target.py": "def target():\n    return 1\n",
                "unrelated.py": "def broken(:\n",
            }
        ) as workspace:
            target = workspace.joinpath("target.py")
            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/list_symbols",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(target),
                        "limit": 10,
                    },
                    request_id=87,
                ),
                request_id=87,
            )

            assert isinstance(result, dict)
            self.assertEqual(result["indexed_files"], 1)
            self.assertEqual(result["total_symbols"], 1)
            self.assertEqual(result["symbols"][0]["semantic_path"], "target")
            self.assertEqual(
                result["symbols"][0]["file_path"],
                str(target).replace("\\", "/"),
            )

    def test_list_symbols_accepts_unsaved_source_with_file_anchor(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/list_symbols",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "limit": 10,
                        "file_path_contains": "caller",
                    },
                    request_id=86,
                ),
                request_id=86,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["total_symbols"], 1)
            self.assertEqual(result["symbols"][0]["semantic_path"], "orchestrate")
            self.assertEqual(result["symbols"][0]["file_path"], expected_file)
