import unittest

from arborist_mcp.gateway_core_helpers import GatewayCoreHelpers
from arborist_mcp.jsonrpc import is_notification_request
from arborist_mcp.tool_result_schemas import JsonRpcError


class GatewayCoreHelperTests(unittest.TestCase):
    def test_call_with_optional_timeout_omits_placeholder_padding_when_unset(self) -> None:
        observed: list[tuple[object, ...]] = []

        def method(*args: object) -> str:
            observed.append(args)
            return "ok"

        result = GatewayCoreHelpers._call_with_optional_timeout(
            method, ("a", "b"), None, omitted_before_timeout=("<omitted>", "<omitted>")
        )

        self.assertEqual(result, "ok")
        self.assertEqual(observed, [("a", "b")])

    def test_call_with_optional_timeout_appends_placeholders_then_timeout(self) -> None:
        observed: list[tuple[object, ...]] = []

        def method(*args: object) -> None:
            observed.append(args)

        GatewayCoreHelpers._call_with_optional_timeout(
            method, ("a",), 1500, omitted_before_timeout=("x", "y")
        )
        GatewayCoreHelpers._call_with_optional_timeout(method, (), 0)

        self.assertEqual(
            observed,
            [("a", "x", "y", 1500), (0,)],
        )

    def test_decode_core_payload_rejects_invalid_json_with_core_message(self) -> None:
        for payload in ("{", "[1,", "NaN", '{"k":1,"k":2}'):
            with self.subTest(payload=payload):
                with self.assertRaisesRegex(
                    JsonRpcError, "invalid JSON from arborist core"
                ):
                    GatewayCoreHelpers._decode_core_payload(payload)

    def test_decode_core_object_rejects_non_object_payloads(self) -> None:
        for payload in ("[1,2]", "3", '"text"', "null"):
            with self.subTest(payload=payload):
                with self.assertRaisesRegex(
                    JsonRpcError, "expected object payload"
                ):
                    GatewayCoreHelpers._decode_core_object(payload)
        self.assertEqual(
            GatewayCoreHelpers._decode_core_object('{"ok":true}'), {"ok": True}
        )

    def test_decode_core_object_array_reports_mid_list_offender_index(self) -> None:
        with self.assertRaisesRegex(
            JsonRpcError, r"expected object item at index 1"
        ):
            GatewayCoreHelpers._decode_core_object_array('[{"a":1},7,{"b":2}]')

    def test_decode_core_object_array_rejects_non_array_and_validates_all_items(self) -> None:
        with self.assertRaisesRegex(JsonRpcError, "expected array payload"):
            GatewayCoreHelpers._decode_core_object_array('{"a":1}')
        self.assertEqual(
            GatewayCoreHelpers._decode_core_object_array("[]"),
            [],
        )
        self.assertEqual(
            GatewayCoreHelpers._decode_core_object_array('[{"a":1},{"b":2}]'),
            [{"a": 1}, {"b": 2}],
        )


class IsNotificationRequestTests(unittest.TestCase):
    def test_accepts_well_formed_notifications(self) -> None:
        self.assertTrue(
            is_notification_request({"jsonrpc": "2.0", "method": "initialized"})
        )
        self.assertTrue(
            is_notification_request(
                {"jsonrpc": "2.0", "method": "notify/x", "params": {}}
            )
        )

    def test_id_key_makes_request_not_a_notification_even_with_null_id(self) -> None:
        self.assertFalse(
            is_notification_request(
                {"jsonrpc": "2.0", "method": "read", "id": None}
            )
        )
        self.assertFalse(
            is_notification_request({"jsonrpc": "2.0", "method": "read", "id": 1})
        )

    def test_rejects_malformed_shapes(self) -> None:
        for request in (
            None,
            [],
            "notification",
            {},
            {"jsonrpc": "1.0", "method": "m"},
            {"jsonrpc": "2.0"},
            {"jsonrpc": "2.0", "method": ""},
            {"jsonrpc": "2.0", "method": 7},
            {"jsonrpc": "2.0", "method": None},
            {"method": "m"},
        ):
            with self.subTest(request=request):
                self.assertFalse(is_notification_request(request))


if __name__ == "__main__":
    unittest.main()
