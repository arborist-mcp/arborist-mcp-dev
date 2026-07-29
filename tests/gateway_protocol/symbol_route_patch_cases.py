from __future__ import annotations


class GatewaySymbolRoutePatchTestsMixin:
    def test_patch_at_position_routes_params_to_core(self) -> None:
        cases = [
            {
                "core_method": "patch_ast_node_at_position_json",
                "rpc_method": "arborist/patch_ast_node_at_position",
                "request_id": 96,
                "params": {
                    "file_path": "sample.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": "def helper() -> int:\n    return 2\n",
                    "source": "def helper() -> int:\n    return 1\n",
                    "bypass_reason": "known-safe",
                },
                "expected_call": (
                    "sample.py",
                    3,
                    1,
                    "def helper() -> int:\n    return 2\n",
                    "def helper() -> int:\n    return 1\n",
                    "known-safe",
                ),
            },
            {
                "core_method": "patch_virtual_ast_node_at_position_json",
                "rpc_method": "arborist/patch_virtual_ast_node_at_position",
                "request_id": 97,
                "params": {
                    "file_path": "sample.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": "def helper() -> int:\n    return 2\n",
                    "bypass_reason": "known-safe",
                },
                "expected_call": (
                    "sample.py",
                    3,
                    1,
                    "def helper() -> int:\n    return 2\n",
                    "known-safe",
                ),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload={},
                    request_id=case["request_id"],
                    expected_call=case["expected_call"],
                    check_result=lambda result: self.assertEqual(result, {}),
                )

    def test_context_validation_routes_params_to_core(self) -> None:
        updated_source = self.orchestrate_updated_source()
        cases = [
            {
                "core_method": "validate_patch_with_graph_context_json",
                "rpc_method": "arborist/validate_patch_with_graph_context",
                "request_id": 70,
                "payload": self.make_graph_context_payload(),
                "check": lambda result: (
                    self.assertTrue(result["patch"]["applied"]),
                    self.assertEqual(
                        result["trace"]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(
                        result["neighborhood"]["nodes"][1]["symbol"]["semantic_path"],
                        "helper",
                    ),
                    self.assertTrue(result["trace_validation"]["allowed"]),
                ),
            },
            {
                "core_method": "validate_patch_with_neighborhood_context_json",
                "rpc_method": "arborist/validate_patch_with_neighborhood_context",
                "request_id": 75,
                "payload": self.make_neighborhood_context_payload(),
                "check": lambda result: (
                    self.assertTrue(result["patch"]["applied"]),
                    self.assertEqual(
                        result["trace"]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(
                        result["neighborhood_context"]["neighborhood"]["nodes"][1][
                            "symbol"
                        ]["semantic_path"],
                        "helper",
                    ),
                    self.assertEqual(
                        result["neighborhood_context"]["reads"][1]["symbol"][
                            "semantic_path"
                        ],
                        "helper",
                    ),
                    self.assertTrue(result["trace_validation"]["allowed"]),
                ),
            },
            {
                "core_method": "validate_patch_with_discovery_context_json",
                "rpc_method": "arborist/validate_patch_with_discovery_context",
                "request_id": 79,
                "payload": self.make_discovery_context_payload(),
                "check": lambda result: (
                    self.assertTrue(result["patch"]["applied"]),
                    self.assertEqual(
                        result["trace"]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(
                        result["read"]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(
                        result["neighborhood_context"]["reads"][1]["symbol"][
                            "semantic_path"
                        ],
                        "helper",
                    ),
                    self.assertTrue(result["trace_validation"]["allowed"]),
                ),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params={
                        "workspace_root": ".",
                        "file_path": "caller.py",
                        "semantic_path": "orchestrate",
                        "new_code": updated_source,
                        "direction": "both",
                        "max_depth": 2,
                        "max_nodes": 10,
                    },
                    payload=case["payload"],
                    request_id=case["request_id"],
                    expected_call=(
                        ".",
                        "caller.py",
                        "orchestrate",
                        updated_source,
                        None,
                        None,
                        "both",
                        2,
                        10,
                        None,
                    ),
                    check_result=case["check"],
                )

    def test_context_validation_at_position_routes_params_to_core(self) -> None:
        updated_source = self.orchestrate_updated_source()
        cases = [
            {
                "core_method": "validate_patch_with_trace_context_at_position_json",
                "rpc_method": "arborist/validate_patch_with_trace_context_at_position",
                "request_id": 98,
                "expected_call": (
                    ".",
                    "caller.py",
                    3,
                    1,
                    updated_source,
                    "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "known-safe",
                    "callers",
                    None,
                ),
            },
            {
                "core_method": "validate_patch_with_graph_context_at_position_json",
                "rpc_method": "arborist/validate_patch_with_graph_context_at_position",
                "request_id": 99,
                "expected_call": (
                    ".",
                    "caller.py",
                    3,
                    1,
                    updated_source,
                    "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "known-safe",
                    "callers",
                    2,
                    10,
                    None,
                ),
            },
            {
                "core_method": "validate_patch_with_neighborhood_context_at_position_json",
                "rpc_method": "arborist/validate_patch_with_neighborhood_context_at_position",
                "request_id": 100,
                "expected_call": (
                    ".",
                    "caller.py",
                    3,
                    1,
                    updated_source,
                    "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "known-safe",
                    "callers",
                    2,
                    10,
                    None,
                ),
            },
            {
                "core_method": "validate_patch_with_discovery_context_at_position_json",
                "rpc_method": "arborist/validate_patch_with_discovery_context_at_position",
                "request_id": 101,
                "expected_call": (
                    ".",
                    "caller.py",
                    3,
                    1,
                    updated_source,
                    "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "known-safe",
                    "callers",
                    2,
                    10,
                    None,
                ),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                params = {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": updated_source,
                    "source": "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "bypass_reason": "known-safe",
                    "direction": "callers",
                }
                if case["core_method"] != "validate_patch_with_trace_context_at_position_json":
                    params["max_depth"] = 2
                    params["max_nodes"] = 10
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=params,
                    payload={},
                    request_id=case["request_id"],
                    expected_call=case["expected_call"],
                    check_result=lambda result: self.assertEqual(result, {}),
                )

    def test_trace_context_timeouts_reach_final_core_parameter(self) -> None:
        source = "def orchestrate(value: int) -> int:\n    return value + 1\n"
        updated_source = self.orchestrate_updated_source()
        cases = (
            {
                "core_method": "validate_patch_with_trace_context_json",
                "rpc_method": "arborist/validate_patch_with_trace_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": updated_source,
                    "source": source,
                    "bypass_reason": "known-safe",
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "expected_call": (
                    ".",
                    "caller.py",
                    "orchestrate",
                    updated_source,
                    source,
                    "known-safe",
                    "callers",
                    "symbols.db",
                    37,
                ),
            },
            {
                "core_method": "validate_patch_with_trace_context_at_position_json",
                "rpc_method": "arborist/validate_patch_with_trace_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 0, "column": 5},
                    "new_code": updated_source,
                    "source": source,
                    "bypass_reason": "known-safe",
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "expected_call": (
                    ".",
                    "caller.py",
                    0,
                    5,
                    updated_source,
                    source,
                    "known-safe",
                    "callers",
                    "symbols.db",
                    37,
                ),
            },
        )

        for request_id, case in enumerate(cases, start=246):
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload={},
                    request_id=request_id,
                    expected_call=case["expected_call"],
                    check_result=lambda result: self.assertEqual(result, {}),
                )

    def test_graph_context_timeouts_reach_final_core_parameter(self) -> None:
        source = "def orchestrate(value: int) -> int:\n    return value + 1\n"
        updated_source = self.orchestrate_updated_source()
        cases = (
            {
                "core_method": "validate_patch_with_graph_context_json",
                "rpc_method": "arborist/validate_patch_with_graph_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": updated_source,
                    "source": source,
                    "bypass_reason": "known-safe",
                    "direction": "callers",
                    "max_depth": 3,
                    "max_nodes": 17,
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "expected_call": (
                    ".",
                    "caller.py",
                    "orchestrate",
                    updated_source,
                    source,
                    "known-safe",
                    "callers",
                    3,
                    17,
                    "symbols.db",
                    37,
                ),
            },
            {
                "core_method": "validate_patch_with_graph_context_at_position_json",
                "rpc_method": "arborist/validate_patch_with_graph_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 0, "column": 5},
                    "new_code": updated_source,
                    "source": source,
                    "bypass_reason": "known-safe",
                    "direction": "callers",
                    "max_depth": 3,
                    "max_nodes": 17,
                    "index_db_path": "symbols.db",
                    "timeout_ms": 37,
                },
                "expected_call": (
                    ".",
                    "caller.py",
                    0,
                    5,
                    updated_source,
                    source,
                    "known-safe",
                    "callers",
                    3,
                    17,
                    "symbols.db",
                    37,
                ),
            },
        )

        for request_id, case in enumerate(cases, start=248):
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload={},
                    request_id=request_id,
                    expected_call=case["expected_call"],
                    check_result=lambda result: self.assertEqual(result, {}),
                )

    def test_rich_context_timeouts_reach_final_core_parameter(self) -> None:
        source = "def orchestrate(value: int) -> int:\n    return value + 1\n"
        updated_source = self.orchestrate_updated_source()
        cases = []
        for offset, kind in enumerate(("neighborhood_context", "discovery_context")):
            cases.extend(
                (
                    {
                        "core_method": f"validate_patch_with_{kind}_json",
                        "rpc_method": f"arborist/validate_patch_with_{kind}",
                        "params": {
                            "workspace_root": ".",
                            "file_path": "caller.py",
                            "semantic_path": "orchestrate",
                            "new_code": updated_source,
                            "source": source,
                            "bypass_reason": "known-safe",
                            "direction": "callers",
                            "max_depth": 3,
                            "max_nodes": 17,
                            "index_db_path": "symbols.db",
                            "timeout_ms": 37 + offset,
                        },
                        "expected_call": (
                            ".",
                            "caller.py",
                            "orchestrate",
                            updated_source,
                            source,
                            "known-safe",
                            "callers",
                            3,
                            17,
                            "symbols.db",
                            37 + offset,
                        ),
                    },
                    {
                        "core_method": f"validate_patch_with_{kind}_at_position_json",
                        "rpc_method": f"arborist/validate_patch_with_{kind}_at_position",
                        "params": {
                            "workspace_root": ".",
                            "file_path": "caller.py",
                            "position": {"row": 0, "column": 5},
                            "new_code": updated_source,
                            "source": source,
                            "bypass_reason": "known-safe",
                            "direction": "callers",
                            "max_depth": 3,
                            "max_nodes": 17,
                            "index_db_path": "symbols.db",
                            "timeout_ms": 37 + offset,
                        },
                        "expected_call": (
                            ".",
                            "caller.py",
                            0,
                            5,
                            updated_source,
                            source,
                            "known-safe",
                            "callers",
                            3,
                            17,
                            "symbols.db",
                            37 + offset,
                        ),
                    },
                )
            )

        for request_id, case in enumerate(cases, start=250):
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload={},
                    request_id=request_id,
                    expected_call=case["expected_call"],
                    check_result=lambda result: self.assertEqual(result, {}),
                )
