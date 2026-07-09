from __future__ import annotations

import io
from unittest import mock

from arborist_mcp import gateway as gateway_module

from tests.gateway_protocol.helpers import GatewayProtocolTestCase


class GatewayRuntimeTests(GatewayProtocolTestCase):
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

