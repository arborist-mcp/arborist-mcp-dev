from __future__ import annotations


class GatewayContextRequestValidationMixin:
    def test_rejects_invalid_read_symbol_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 87,
                "method": "arborist/read_symbol_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 4},
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 87)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_read_symbol_neighborhood_context_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 70,
                "method": "arborist/read_symbol_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 70)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_read_symbol_neighborhood_context_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 71,
                "method": "arborist/read_symbol_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 71)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_read_symbol_neighborhood_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 88,
                "method": "arborist/read_symbol_neighborhood_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 4},
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 88)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_read_symbol_neighborhood_context_at_position_max_nodes(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 89,
                "method": "arborist/read_symbol_neighborhood_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 4},
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 89)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_read_symbol_discovery_context_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 72,
                "method": "arborist/read_symbol_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 72)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_read_symbol_discovery_context_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 73,
                "method": "arborist/read_symbol_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 73)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_read_symbol_discovery_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 90,
                "method": "arborist/read_symbol_discovery_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 4},
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 90)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_read_symbol_discovery_context_at_position_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 91,
                "method": "arborist/read_symbol_discovery_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 4},
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 91)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_list_symbols_neighborhood_context_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 79,
                "method": "arborist/list_symbols_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 79)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_list_symbols_neighborhood_context_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 80,
                "method": "arborist/list_symbols_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 80)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_search_symbols_discovery_context_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 82,
                "method": "arborist/search_symbols_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 82)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_search_symbols_discovery_context_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 83,
                "method": "arborist/search_symbols_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 83)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_list_symbols_discovery_context_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 84,
                "method": "arborist/list_symbols_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 84)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_list_symbols_discovery_context_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 85,
                "method": "arborist/list_symbols_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 85)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_trace_context_direction_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 17,
                "method": "arborist/validate_patch_with_trace_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "semantic_path": "orchestrate",
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 17)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_trace_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 89,
                "method": "arborist/validate_patch_with_trace_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "position": {"row": 0, "column": 4},
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 89)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_graph_context_direction_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 68,
                "method": "arborist/validate_patch_with_graph_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "semantic_path": "orchestrate",
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 68)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_graph_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 90,
                "method": "arborist/validate_patch_with_graph_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "position": {"row": 0, "column": 4},
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 90)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_graph_context_max_nodes_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 69,
                "method": "arborist/validate_patch_with_graph_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "semantic_path": "orchestrate",
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 69)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_zero_graph_context_at_position_max_nodes_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 91,
                "method": "arborist/validate_patch_with_graph_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": 0, "column": 4},
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 91)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_neighborhood_context_direction_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 73,
                "method": "arborist/validate_patch_with_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "semantic_path": "orchestrate",
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 73)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_neighborhood_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 92,
                "method": "arborist/validate_patch_with_neighborhood_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "position": {"row": 0, "column": 4},
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 92)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_neighborhood_context_max_nodes_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 74,
                "method": "arborist/validate_patch_with_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "semantic_path": "orchestrate",
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 74)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_zero_neighborhood_context_at_position_max_nodes_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 93,
                "method": "arborist/validate_patch_with_neighborhood_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": 0, "column": 4},
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 93)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_discovery_context_direction_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 77,
                "method": "arborist/validate_patch_with_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "semantic_path": "orchestrate",
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 77)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_discovery_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 94,
                "method": "arborist/validate_patch_with_discovery_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "position": {"row": 0, "column": 4},
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 94)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_discovery_context_max_nodes_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 78,
                "method": "arborist/validate_patch_with_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "semantic_path": "orchestrate",
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 78)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_zero_discovery_context_at_position_max_nodes_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 95,
                "method": "arborist/validate_patch_with_discovery_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": 0, "column": 4},
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 95)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])
