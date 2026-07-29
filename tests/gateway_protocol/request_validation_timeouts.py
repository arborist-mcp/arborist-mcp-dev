from __future__ import annotations

from arborist_mcp import gateway as gateway_module


class GatewayTimeoutRequestValidationMixin:
    def test_rejects_invalid_semantic_skeleton_timeout_bounds(self) -> None:
        for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
            with self.subTest(timeout_ms=timeout_ms):
                response = self.make_gateway().handle_request(
                    self.request(
                        "arborist/get_semantic_skeleton",
                        {
                            "file_path": "sample.py",
                            "source": "def sample():\n    return 1\n",
                            "timeout_ms": timeout_ms,
                        },
                        request_id=80 + timeout_ms,
                    )
                )

                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_index_health_timeout_bounds(self) -> None:
        for method in (
            "arborist/inspect_symbol_index",
            "arborist/migrate_symbol_index",
        ):
            for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
                with self.subTest(method=method, timeout_ms=timeout_ms):
                    response = self.make_gateway().handle_request(
                        self.request(
                            method,
                            {
                                "db_path": "symbols.db",
                                "timeout_ms": timeout_ms,
                            },
                            request_id=82 + timeout_ms,
                        )
                    )

                    self.assertEqual(response["error"]["code"], -32602)
                    self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_offline_patch_analysis_timeout_bounds(self) -> None:
        cases = (
            (
                "arborist/replay_patch_evidence_against_trace",
                {"patch": {}, "trace": {}},
            ),
            (
                "arborist/validate_patch_commit_with_trace",
                {"patch": {}, "trace": {}},
            ),
            (
                "arborist/export_patch_diagnostics_sarif",
                {"patch": {}},
            ),
        )
        for method, base_params in cases:
            for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
                with self.subTest(method=method, timeout_ms=timeout_ms):
                    response = self.make_gateway().handle_request(
                        self.request(
                            method,
                            {**base_params, "timeout_ms": timeout_ms},
                            request_id=84 + timeout_ms,
                        )
                    )

                    self.assertEqual(response["error"]["code"], -32602)
                    self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_index_registry_timeout_bounds(self) -> None:
        cases = (
            ("arborist/unregister_symbol_index", {"workspace_root": "."}),
            ("arborist/list_symbol_indexes", {}),
        )
        for method, base_params in cases:
            for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
                with self.subTest(method=method, timeout_ms=timeout_ms):
                    response = self.make_gateway().handle_request(
                        self.request(
                            method,
                            {**base_params, "timeout_ms": timeout_ms},
                            request_id=83 + timeout_ms,
                        )
                    )

                    self.assertEqual(response["error"]["code"], -32602)
                    self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_batch_timeout_bounds(self) -> None:
        for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
            with self.subTest(timeout_ms=timeout_ms):
                response = self.make_gateway().handle_request(
                    self.request(
                        "arborist/batch",
                        {
                            "calls": [{"name": "arborist/list_symbol_indexes"}],
                            "timeout_ms": timeout_ms,
                        },
                        request_id=84 + timeout_ms,
                    )
                )

                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_patch_preview_timeout_bounds(self) -> None:
        cases = (
            (
                "arborist/preview_patch_ast_node",
                {"semantic_path": "sample"},
            ),
            (
                "arborist/preview_patch_ast_node_at_position",
                {"position": {"row": 0, "column": 4}},
            ),
        )
        for method, target_params in cases:
            for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
                with self.subTest(method=method, timeout_ms=timeout_ms):
                    response = self.make_gateway().handle_request(
                        self.request(
                            method,
                            {
                                "file_path": "sample.py",
                                "source": "def sample():\n    return 1\n",
                                "new_code": "def sample():\n    return 2\n",
                                "timeout_ms": timeout_ms,
                                **target_params,
                            },
                            request_id=85 + timeout_ms,
                        )
                    )

                    self.assertEqual(response["error"]["code"], -32602)
                    self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_patch_apply_timeout_bounds(self) -> None:
        cases = (
            (
                "arborist/patch_ast_node",
                {
                    "semantic_path": "sample",
                    "source": "def sample():\n    return 1\n",
                },
            ),
            (
                "arborist/patch_ast_node_at_position",
                {
                    "position": {"row": 0, "column": 4},
                    "source": "def sample():\n    return 1\n",
                },
            ),
            (
                "arborist/patch_virtual_ast_node",
                {"semantic_path": "sample"},
            ),
            (
                "arborist/patch_virtual_ast_node_at_position",
                {"position": {"row": 0, "column": 4}},
            ),
        )
        for method, target_params in cases:
            for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
                with self.subTest(method=method, timeout_ms=timeout_ms):
                    response = self.make_gateway().handle_request(
                        self.request(
                            method,
                            {
                                "file_path": "sample.py",
                                "new_code": "def sample():\n    return 2\n",
                                "timeout_ms": timeout_ms,
                                **target_params,
                            },
                            request_id=87 + timeout_ms,
                        )
                    )

                    self.assertEqual(response["error"]["code"], -32602)
                    self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_virtual_operation_timeout_bounds(self) -> None:
        cases = (
            ("arborist/did_open", {"file_path": "sample.py"}),
            ("arborist/did_change", {"file_path": "sample.py", "edits": []}),
            ("arborist/read_virtual_file", {"file_path": "sample.py"}),
            ("arborist/list_virtual_files", {}),
            (
                "arborist/apply_buffer_edit",
                {
                    "file_path": "sample.py",
                    "start_byte": 0,
                    "old_end_byte": 0,
                    "new_text": "x",
                },
            ),
            ("arborist/commit_virtual_file", {"file_path": "sample.py"}),
            ("arborist/discard_virtual_file", {"file_path": "sample.py"}),
            ("arborist/did_close", {"file_path": "sample.py"}),
        )
        for method, base_params in cases:
            for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
                with self.subTest(method=method, timeout_ms=timeout_ms):
                    response = self.make_gateway().handle_request(
                        self.request(
                            method,
                            {**base_params, "timeout_ms": timeout_ms},
                            request_id=89 + timeout_ms,
                        )
                    )

                    self.assertEqual(response["error"]["code"], -32602)
                    self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_workspace_edit_preview_timeout_bounds(self) -> None:
        for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
            with self.subTest(timeout_ms=timeout_ms):
                response = self.make_gateway().handle_request(
                    self.request(
                        "arborist/preview_workspace_position_edits",
                        {
                            "files": [
                                {
                                    "file_path": "sample.py",
                                    "source": "value = 1\n",
                                    "edits": [],
                                }
                            ],
                            "timeout_ms": timeout_ms,
                        },
                        request_id=90 + timeout_ms,
                    )
                )

                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_direct_trace_timeout_bounds(self) -> None:
        methods = (
            "arborist/trace_symbol_graph",
            "arborist/trace_symbol_neighborhood",
            "arborist/trace_symbol_graph_at_position",
            "arborist/trace_symbol_neighborhood_at_position",
        )
        for method in methods:
            for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
                with self.subTest(method=method, timeout_ms=timeout_ms):
                    params: dict[str, Any] = {
                        "workspace_root": ".",
                        "timeout_ms": timeout_ms,
                    }
                    if method.endswith("_at_position"):
                        params.update(
                            {
                                "file_path": "graph_b.py",
                                "position": {"row": 0, "column": 5},
                            }
                        )
                    else:
                        params["symbol_path"] = "orchestrate"

                    response = self.make_gateway().handle_request(
                        self.request(method, params, request_id=100 + timeout_ms)
                    )

                    self.assertEqual(response["error"]["code"], -32602)
                    self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_search_symbols_timeout_bounds(self) -> None:
        for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
            with self.subTest(timeout_ms=timeout_ms):
                response = self.make_gateway().handle_request(
                    self.request(
                        "arborist/search_symbols",
                        {
                            "workspace_root": ".",
                            "query": "helper",
                            "timeout_ms": timeout_ms,
                        },
                        request_id=130 + timeout_ms,
                    )
                )

                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_search_symbols_context_timeout_bounds(self) -> None:
        for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
            with self.subTest(timeout_ms=timeout_ms):
                response = self.make_gateway().handle_request(
                    self.request(
                        "arborist/search_symbols_context",
                        {
                            "workspace_root": ".",
                            "query": "helper",
                            "timeout_ms": timeout_ms,
                        },
                        request_id=140 + timeout_ms,
                    )
                )

                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_search_symbols_neighborhood_context_timeout_bounds(self) -> None:
        for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
            with self.subTest(timeout_ms=timeout_ms):
                response = self.make_gateway().handle_request(
                    self.request(
                        "arborist/search_symbols_neighborhood_context",
                        {
                            "workspace_root": ".",
                            "query": "helper",
                            "direction": "callers",
                            "max_depth": 2,
                            "max_nodes": 10,
                            "timeout_ms": timeout_ms,
                        },
                        request_id=150 + timeout_ms,
                    )
                )

                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("timeout_ms", response["error"]["message"])

    def test_rejects_invalid_search_symbols_discovery_context_timeout_bounds(self) -> None:
        for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
            with self.subTest(timeout_ms=timeout_ms):
                response = self.make_gateway().handle_request(
                    self.request(
                        "arborist/search_symbols_discovery_context",
                        {
                            "workspace_root": ".",
                            "query": "helper",
                            "direction": "callers",
                            "max_depth": 2,
                            "max_nodes": 10,
                            "timeout_ms": timeout_ms,
                        },
                        request_id=160 + timeout_ms,
                    )
                )

                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("timeout_ms", response["error"]["message"])

