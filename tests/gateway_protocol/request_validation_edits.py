from __future__ import annotations

from arborist_mcp import gateway as gateway_module


class GatewayEditRequestValidationMixin:
    def test_rejects_non_json_serializable_edits_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 10,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [{"new_text": {1, 2, 3}}],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 10)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits", response["error"]["message"])

    def test_rejects_too_many_position_edits_before_core_call(self) -> None:
        class StubCore:
            def apply_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        edit = {
            "start": {"row": 0, "column": 0},
            "end": {"row": 0, "column": 0},
            "new_text": "",
        }
        gateway = self.make_gateway()
        gateway._core = StubCore()

        self.assert_invalid_params(
            "arborist/did_change",
            {
                "file_path": "sample.py",
                "edits": [edit] * (gateway_module.MAX_POSITION_EDITS + 1),
            },
            request_id=11,
            contains="position edits",
            gateway=gateway,
        )

    def test_rejects_position_edit_text_budget_before_core_call(self) -> None:
        class StubCore:
            def apply_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()
        edit = {
            "start": {"row": 0, "column": 0},
            "end": {"row": 0, "column": 0},
            "new_text": "x" * gateway_module.TEXT_PARAM_MAX_LENGTH,
        }

        self.assert_invalid_params(
            "arborist/did_change",
            {
                "file_path": "sample.py",
                "edits": [
                    edit
                    for _ in range(
                        gateway_module.MAX_POSITION_EDIT_TEXT_BYTES
                        // gateway_module.TEXT_PARAM_MAX_LENGTH
                        + 1
                    )
                ],
            },
            request_id=12,
            contains="replacement text exceeds",
            gateway=gateway,
        )

    def test_rejects_too_many_preview_position_edits_before_core_call(self) -> None:
        class StubCore:
            def preview_workspace_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        edit = {
            "start": {"row": 0, "column": 0},
            "end": {"row": 0, "column": 0},
            "new_text": "",
        }
        gateway = self.make_gateway()
        gateway._core = StubCore()

        self.assert_invalid_params(
            "arborist/preview_workspace_position_edits",
            {
                "files": [
                    {
                        "file_path": "sample.py",
                        "edits": [edit] * (gateway_module.MAX_POSITION_EDITS + 1),
                    }
                ]
            },
            request_id=12,
            contains="files[0].edits",
            gateway=gateway,
        )

    def test_rejects_too_many_workspace_preview_files_before_core_call(self) -> None:
        class StubCore:
            def preview_workspace_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        self.assert_invalid_params(
            "arborist/preview_workspace_position_edits",
            {
                "files": [
                    {"file_path": f"sample-{index}.py", "edits": []}
                    for index in range(gateway_module.MAX_WORKSPACE_EDIT_PREVIEW_FILES + 1)
                ]
            },
            request_id=13,
            contains="files must contain at most",
            gateway=gateway,
        )

    def test_rejects_empty_workspace_preview_files_before_core_call(self) -> None:
        class StubCore:
            def preview_workspace_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()
        self.assert_invalid_params(
            "arborist/preview_workspace_position_edits",
            {"files": []},
            request_id=16,
            contains="files must contain at least 1 entry",
            gateway=gateway,
        )

    def test_rejects_oversized_workspace_preview_source_before_core_call(self) -> None:
        class StubCore:
            def preview_workspace_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()
        self.assert_invalid_params(
            "arborist/preview_workspace_position_edits",
            {
                "files": [
                    {
                        "file_path": "sample.py",
                        "source": "x" * (gateway_module.TEXT_PARAM_MAX_LENGTH + 1),
                        "edits": [],
                    }
                ]
            },
            request_id=14,
            contains="files[0].source",
            gateway=gateway,
        )

    def test_rejects_malformed_workspace_preview_file_before_core_call(self) -> None:
        class StubCore:
            def preview_workspace_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()
        malformed_files = (
            [None],
            [{"file_path": "sample.py"}],
            [{"file_path": "sample.py", "edits": [], "unexpected": True}],
        )
        expected_messages = (
            "expected object",
            "files[0].edits",
            "files[0].unexpected",
        )
        for files, message in zip(malformed_files, expected_messages):
            with self.subTest(files=files):
                self.assert_invalid_params(
                    "arborist/preview_workspace_position_edits",
                    {"files": files},
                    request_id=15,
                    contains=message,
                    gateway=gateway,
                )

    def test_rejects_multibyte_source_over_byte_limit(self) -> None:
        gateway = self.make_gateway()
        source = "é" * (gateway_module.TEXT_PARAM_MAX_LENGTH // 2 + 1)

        with self.assertRaisesRegex(Exception, "max byte length"):
            gateway._optional_string({"source": source}, "source", allow_empty=True)

    def test_rejects_multibyte_query_over_byte_limit(self) -> None:
        gateway = self.make_gateway()
        query = "é" * (gateway_module.TREE_QUERY_MAX_LENGTH // 2 + 1)

        with self.assertRaisesRegex(Exception, "max byte length"):
            gateway._require_string({"query": query}, "query")

    def test_rejects_multibyte_bypass_reason_over_byte_limit(self) -> None:
        gateway = self.make_gateway()
        reason = "é" * (gateway_module.BYPASS_REASON_MAX_LENGTH // 2 + 1)

        with self.assertRaisesRegex(Exception, "max byte length"):
            gateway._optional_string({"bypass_reason": reason}, "bypass_reason")

    def test_rejects_multibyte_nested_text_over_byte_limit(self) -> None:
        text = "é" * (gateway_module.TEXT_PARAM_MAX_LENGTH // 2 + 1)
        edit = {
            "start": {"row": 0, "column": 0},
            "end": {"row": 0, "column": 0},
            "new_text": text,
        }

        with self.assertRaisesRegex(Exception, "max byte length"):
            self.make_gateway()._validate_position_edits([edit])

    def test_rejects_non_finite_edits_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 12,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [{"start": {"row": float("nan"), "column": 0}}],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 12)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits", response["error"]["message"])

    def test_rejects_negative_position_edit_coordinates(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 28,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": -1, "column": 0},
                            "end": {"row": 0, "column": 0},
                            "new_text": "x",
                        }
                    ],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 28)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits[0].start.row", response["error"]["message"])

    def test_rejects_missing_position_edit_new_text(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 29,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": 0, "column": 0},
                            "end": {"row": 0, "column": 0},
                        }
                    ],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 29)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits[0].new_text", response["error"]["message"])

    def test_rejects_reversed_position_edit_range(self) -> None:
        class StubCore:
            def apply_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 37,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": 2, "column": 0},
                            "end": {"row": 1, "column": 9},
                            "new_text": "x",
                        }
                    ],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 37)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits[0].start", response["error"]["message"])

    def test_rejects_unknown_position_edit_fields(self) -> None:
        class StubCore:
            def apply_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 40,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": 0, "column": 0},
                            "end": {"row": 0, "column": 0},
                            "new_text": "x",
                            "newText": "x",
                        }
                    ],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 40)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits[0].newText", response["error"]["message"])

    def test_rejects_unknown_position_fields(self) -> None:
        class StubCore:
            def apply_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 41,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": 0, "column": 0, "character": 0},
                            "end": {"row": 0, "column": 0},
                            "new_text": "x",
                        }
                    ],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 41)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits[0].start.character", response["error"]["message"])\n
