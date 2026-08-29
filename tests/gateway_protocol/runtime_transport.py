from __future__ import annotations

import io
from unittest import mock

from arborist_mcp import gateway as gateway_module


class GatewayRuntimeTransportTestsMixin:
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

    def test_legacy_error_response_escapes_malformed_exception_text(self) -> None:
        gateway = self.make_gateway(object())
        tool_name = "arborist/get_semantic_skeleton"
        with mock.patch.object(
            gateway,
            gateway_module.tool_spec(tool_name).handler,
            side_effect=ValueError("core failed: caf\u00e9 \ud800"),
        ):
            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 25,
                    "method": tool_name,
                    "params": {},
                }
            )

        payload = gateway_module._serialize_response(response)
        decoded = gateway_module.json.loads(payload)
        self.assertEqual(decoded["error"]["code"], -32602)
        self.assertEqual(decoded["error"]["message"], "core failed: caf\u00e9 \\ud800")

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

    def test_stdio_bounds_and_discards_an_oversized_line(self) -> None:
        class StubGateway:
            def __init__(self) -> None:
                self.requests: list[object] = []

            def handle_request(self, request: object) -> dict[str, object]:
                self.requests.append(request)
                return {"jsonrpc": "2.0", "id": 9, "result": {"ok": True}}

        fake_gateway = StubGateway()
        stdin = io.StringIO(
            "x" * (gateway_module.MAX_REQUEST_BYTES + 64)
            + '\n{"jsonrpc":"2.0","id":9,"method":"initialize","params":{}}\n'
        )
        stdout = io.StringIO()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        responses = [
            gateway_module.json.loads(line)
            for line in stdout.getvalue().splitlines()
        ]
        self.assertEqual(len(responses), 2)
        self.assertEqual(responses[0]["error"]["code"], -32600)
        self.assertIn("request exceeds maximum size", responses[0]["error"]["message"])
        self.assertEqual(responses[1]["result"], {"ok": True})
        self.assertEqual(
            fake_gateway.requests,
            [{"jsonrpc": "2.0", "id": 9, "method": "initialize", "params": {}}],
        )

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

    def test_stdio_rejects_non_json_unicode_whitespace_around_request(self) -> None:
        stdin = io.StringIO(
            "\u00a0{\"jsonrpc\":\"2.0\",\"id\":27,\"method\":\"initialize\",\"params\":{}}\u00a0\n"
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

    def test_once_rejects_non_json_unicode_whitespace_around_request(self) -> None:
        with mock.patch.object(
            gateway_module.ArboristGateway,
            "__init__",
            side_effect=AssertionError("gateway should not initialize"),
        ):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value="\u00a0{\"jsonrpc\":\"2.0\",\"id\":28,\"method\":\"initialize\",\"params\":{}}\u00a0",
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32700)

    def test_stdio_invalid_utf8_bytes_emit_parse_error_without_initializing_gateway(
        self,
    ) -> None:
        stdin = io.TextIOWrapper(io.BytesIO(b"\xff\n"), encoding="utf-8")
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
        self.assertIn("not valid UTF-8 text", response["error"]["message"])

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

    def test_once_oversized_request_file_reports_cli_error_without_reading(self) -> None:
        stderr = io.StringIO()
        stat_result = mock.Mock(st_size=gateway_module.MAX_REQUEST_BYTES + 1)

        with mock.patch("pathlib.Path.stat", return_value=stat_result):
            with mock.patch("pathlib.Path.read_text") as mock_read_text:
                with mock.patch("sys.stderr", stderr):
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 1)
        self.assertIn("request file exceeds maximum size", stderr.getvalue())
        mock_read_text.assert_not_called()

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

    def test_stdio_retries_with_ascii_escaped_json_when_stdout_cannot_encode_unicode(
        self,
    ) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                return {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {"message": "\u00e9\U0001f600"},
                }

        fake_gateway = StubGateway()
        stdin = io.StringIO(
            '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n'
        )
        stdout_bytes = io.BytesIO()
        stdout = io.TextIOWrapper(stdout_bytes, encoding="ascii", errors="strict")

        try:
            with mock.patch.object(
                gateway_module, "ArboristGateway", return_value=fake_gateway
            ):
                with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                    exit_code = gateway_module.run_stdio()
            stdout.flush()
            raw_output = stdout_bytes.getvalue().decode("ascii")
            response = gateway_module.json.loads(raw_output)
        finally:
            stdout.detach()

        self.assertEqual(exit_code, 0)
        self.assertIn(r"\u00e9", raw_output)
        self.assertIn(r"\ud83d\ude00", raw_output)
        self.assertEqual(response["result"], {"message": "\u00e9\U0001f600"})

    def test_once_retries_with_ascii_escaped_json_when_stdout_cannot_encode_unicode(
        self,
    ) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                return {
                    "jsonrpc": "2.0",
                    "id": 2,
                    "result": {"message": "\u00e9\U0001f600"},
                }

        fake_gateway = StubGateway()
        stdout_bytes = io.BytesIO()
        stdout = io.TextIOWrapper(stdout_bytes, encoding="ascii", errors="strict")

        try:
            with mock.patch.object(
                gateway_module, "ArboristGateway", return_value=fake_gateway
            ):
                with mock.patch(
                    "pathlib.Path.read_text",
                    return_value=(
                        '{"jsonrpc":"2.0","id":2,"method":"initialize","params":{}}'
                    ),
                ):
                    with mock.patch("sys.stdout", stdout):
                        exit_code = gateway_module.main(["--once", "dummy.json"])
            stdout.flush()
            raw_output = stdout_bytes.getvalue().decode("ascii")
            response = gateway_module.json.loads(raw_output)
        finally:
            stdout.detach()

        self.assertEqual(exit_code, 0)
        self.assertIn(r"\u00e9", raw_output)
        self.assertIn(r"\ud83d\ude00", raw_output)
        self.assertEqual(response["result"], {"message": "\u00e9\U0001f600"})

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

    def test_stdio_deep_response_emits_internal_error(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                nested: object = {"value": True}
                for _ in range(20_000):
                    nested = [nested]
                return {"jsonrpc": "2.0", "id": 34, "result": nested}

        fake_gateway = StubGateway()
        stdin = io.StringIO('{"jsonrpc":"2.0","id":34,"method":"initialize","params":{}}\n')
        stdout = io.StringIO()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 34)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to serialize response", response["error"]["message"])

    def test_stdio_response_with_lone_surrogate_emits_internal_error(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {
                    "jsonrpc": "2.0",
                    "id": 35,
                    "result": {"value": "\ud800"},
                }

        fake_gateway = StubGateway()
        stdin = io.StringIO('{"jsonrpc":"2.0","id":35,"method":"initialize","params":{}}\n')
        stdout = io.StringIO()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 35)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to serialize response", response["error"]["message"])

    def test_stdio_lone_surrogate_response_id_falls_back_to_null(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {
                    "jsonrpc": "2.0",
                    "id": "\ud800",
                    "result": {"value": object()},
                }

        fake_gateway = StubGateway()
        stdin = io.StringIO('{"jsonrpc":"2.0","id":36,"method":"initialize","params":{}}\n')
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
