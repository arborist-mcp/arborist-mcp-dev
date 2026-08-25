import json
import unittest

from arborist_mcp.mcp_lifecycle import initialize, initialized, is_mcp_initialize
from arborist_mcp.mcp_tools import tools_list
from arborist_mcp.mcp_validation import reject_unexpected_params
from arborist_mcp.resources import resources_list, resources_read
from arborist_mcp.tool_manifest import build_resource_catalog, build_tool_catalog
from arborist_mcp.tool_result_schemas import JsonRpcError
from arborist_mcp.tool_specs import (
    MCP_PROTOCOL_VERSION,
    TOOL_CATALOG_RESOURCE_MIME_TYPE,
    TOOL_CATALOG_RESOURCE_URI,
    TOOL_NAMES,
)


def _supported_languages() -> list[str]:
    return ["python", "rust"]


def _server_info() -> dict[str, object]:
    return {"name": "arborist-mcp", "version": "test"}


class McpInitializeTests(unittest.TestCase):
    def test_legacy_initialize_reports_capabilities_without_protocol_version(self) -> None:
        result = initialize(
            {},
            server_info=_server_info(),
            supported_languages=_supported_languages,
        )

        self.assertNotIn("protocolVersion", result)
        self.assertEqual(result["serverInfo"], _server_info())
        self.assertEqual(result["capabilities"]["tools"], list(TOOL_NAMES))
        self.assertEqual(result["capabilities"]["resources"], build_resource_catalog())
        self.assertEqual(result["supportedLanguages"], ["python", "rust"])

    def test_mcp_initialize_echoes_supported_protocol_version(self) -> None:
        for client_version in ("2020-01-01", MCP_PROTOCOL_VERSION):
            with self.subTest(client_version=client_version):
                result = initialize(
                    {"protocolVersion": client_version},
                    server_info=_server_info(),
                    supported_languages=_supported_languages,
                )

                self.assertEqual(result["protocolVersion"], MCP_PROTOCOL_VERSION)
                self.assertIn("instructions", result)
                self.assertIn("serverInfo", result)
                self.assertEqual(result["supportedLanguages"], ["python", "rust"])

    def test_is_mcp_initialize_detects_marker_keys(self) -> None:
        self.assertFalse(is_mcp_initialize({}))
        self.assertTrue(is_mcp_initialize({"protocolVersion": "2025-06-18"}))
        self.assertTrue(is_mcp_initialize({"capabilities": {}}))
        self.assertTrue(is_mcp_initialize({"clientInfo": {"name": "x"}}))

    def test_initialize_rejects_null_and_blank_protocol_version(self) -> None:
        for value in (None, "   ", 7):
            with self.subTest(value=value):
                with self.assertRaisesRegex(
                    JsonRpcError, "invalid string param: protocolVersion"
                ):
                    initialize(
                        {"protocolVersion": value},
                        server_info=_server_info(),
                        supported_languages=_supported_languages,
                    )

    def test_initialize_rejects_non_object_capabilities_and_client_info(self) -> None:
        with self.assertRaisesRegex(JsonRpcError, "capabilities must be an object"):
            initialize(
                {"capabilities": []},
                server_info=_server_info(),
                supported_languages=_supported_languages,
            )
        with self.assertRaisesRegex(JsonRpcError, "clientInfo must be an object"):
            initialize(
                {"clientInfo": "me"},
                server_info=_server_info(),
                supported_languages=_supported_languages,
            )

    def test_initialize_rejects_unexpected_params(self) -> None:
        with self.assertRaisesRegex(JsonRpcError, "unexpected param: roots"):
            initialize(
                {"roots": []},
                server_info=_server_info(),
                supported_languages=_supported_languages,
            )
        with self.assertRaisesRegex(JsonRpcError, "unexpected param: extra"):
            initialize(
                {"capabilities": {}, "extra": 1},
                server_info=_server_info(),
                supported_languages=_supported_languages,
            )

    def test_initialized_accepts_meta_only(self) -> None:
        self.assertEqual(initialized({}), {})
        self.assertEqual(initialized({"_meta": {}}), {})
        with self.assertRaisesRegex(JsonRpcError, "unexpected param: extra"):
            initialized({"extra": 1})


class McpResourceAndToolListTests(unittest.TestCase):
    def test_resources_list_rejects_non_string_cursor(self) -> None:
        for cursor in (5, [], True):
            with self.subTest(cursor=cursor):
                with self.assertRaisesRegex(
                    JsonRpcError, "invalid params: cursor must be a string"
                ):
                    resources_list({"cursor": cursor})

    def test_resources_list_accepts_empty_and_string_cursor(self) -> None:
        self.assertEqual(resources_list({}), {"resources": build_resource_catalog()})
        self.assertEqual(
            resources_list({"cursor": "next"}),
            {"resources": build_resource_catalog()},
        )

    def test_resources_read_rejects_missing_blank_and_unknown_uri(self) -> None:
        with self.assertRaisesRegex(JsonRpcError, "missing required string param: uri"):
            resources_read({})
        with self.assertRaisesRegex(JsonRpcError, "missing required string param: uri"):
            resources_read({"uri": "   "})
        with self.assertRaisesRegex(JsonRpcError, "unknown resource: arborist://nope"):
            resources_read({"uri": "arborist://nope"})
        with self.assertRaisesRegex(JsonRpcError, "unexpected param: extra"):
            resources_read({"uri": TOOL_CATALOG_RESOURCE_URI, "extra": 1})

    def test_resources_read_returns_parseable_catalog(self) -> None:
        response = resources_read({"uri": TOOL_CATALOG_RESOURCE_URI})

        self.assertEqual(len(response["contents"]), 1)
        content = response["contents"][0]
        self.assertEqual(content["uri"], TOOL_CATALOG_RESOURCE_URI)
        self.assertEqual(content["mimeType"], TOOL_CATALOG_RESOURCE_MIME_TYPE)
        self.assertEqual(json.loads(content["text"]), build_tool_catalog())

    def test_tools_list_rejects_non_string_cursor(self) -> None:
        with self.assertRaisesRegex(
            JsonRpcError, "invalid params: cursor must be a string"
        ):
            tools_list({"cursor": 5})
        self.assertEqual(tools_list({}), {"tools": build_tool_catalog()})


class RejectUnexpectedParamsTests(unittest.TestCase):
    def test_allowed_keys_pass_through(self) -> None:
        reject_unexpected_params({"a": 1, "b": 2}, ("a", "b"))

    def test_reports_sorted_first_unexpected_key(self) -> None:
        with self.assertRaisesRegex(JsonRpcError, "unexpected param: alpha"):
            reject_unexpected_params({"zeta": 1, "alpha": 2}, ("mid",))
        with self.assertRaisesRegex(JsonRpcError, "unexpected param: anything"):
            reject_unexpected_params({"anything": 1}, ())


if __name__ == "__main__":
    unittest.main()
