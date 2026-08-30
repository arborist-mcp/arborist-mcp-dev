from __future__ import annotations


class GatewayTraceRequestValidationMixin:
    def test_rejects_invalid_trace_direction_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 16,
                "method": "arborist/trace_symbol_graph",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 16)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_trace_symbol_neighborhood_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 64,
                "method": "arborist/trace_symbol_neighborhood",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 64)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_trace_symbol_graph_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 97,
                "method": "arborist/trace_symbol_graph_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 97)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_negative_trace_symbol_neighborhood_limits(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 65,
                "method": "arborist/trace_symbol_neighborhood",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "max_depth": -1,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 65)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_depth", response["error"]["message"])

    def test_rejects_zero_trace_symbol_neighborhood_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 67,
                "method": "arborist/trace_symbol_neighborhood",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 67)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_trace_symbol_neighborhood_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 98,
                "method": "arborist/trace_symbol_neighborhood_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 98)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_trace_symbol_neighborhood_at_position_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 99,
                "method": "arborist/trace_symbol_neighborhood_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 99)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])\n
