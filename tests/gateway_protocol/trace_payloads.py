from __future__ import annotations

from tests.gateway_protocol.helpers import GatewayProtocolTestCase, deep_merge
from tests.gateway_protocol.semantic_fixtures import GatewaySemanticFixtureMixin

SUITE_NAME = "gateway-trace-payloads"
REQUIRES_EXTENSION = True
COVERED_TOOLS = (
    "arborist/replay_patch_evidence_against_trace",
    "arborist/validate_patch_commit_with_trace",
)


class GatewayTracePayloadTests(GatewaySemanticFixtureMixin, GatewayProtocolTestCase):
    TRACE_METHODS = (
        "arborist/replay_patch_evidence_against_trace",
        "arborist/validate_patch_commit_with_trace",
    )

    def setUp(self) -> None:
        self.gateway = self.make_live_gateway()

    def minimal_params(self) -> dict[str, object]:
        root_key = "top_level|sample.py|function_definition|trace_root|0..10|"
        return {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": True,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return 2\n",
                "validation": {
                    "syntax_errors": [],
                    "unresolved_identifiers": [],
                    "resolved_identifiers": [],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [],
                    "commit_gate": {
                        "status": "allowed",
                        "allowed": True,
                        "reason": "syntax and symbol binding validation passed",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [],
                        "syntax_error_count": 0,
                    },
                },
            },
            "trace": {
                "symbol": self.make_symbol(
                    symbol_id="top_level",
                    origin_type="trace_root",
                    evidence_key=root_key,
                    include_trace_fields=True,
                ),
                "callers": [],
                "callees": [],
                "evidence_keys": {
                    "symbol": root_key,
                    "callers": [],
                    "callees": [],
                },
                "indexed_files": 1,
            },
        }

    def syntax_error_params(self) -> dict[str, object]:
        return deep_merge(
            self.minimal_params(),
            {
                "patch": {
                    "applied": False,
                    "updated_source": "def top_level() -> int:\n    return (\n",
                    "validation": {
                        "syntax_errors": [
                            {
                                "kind": "error",
                                "message": "unexpected end of input",
                                "start_byte": 31,
                                "end_byte": 31,
                                "start_point": {"row": 1, "column": 11},
                                "end_point": {"row": 1, "column": 11},
                            }
                        ],
                        "commit_gate": {
                            "status": "rejected",
                            "allowed": False,
                            "reason": "syntax validation failed",
                            "syntax_error_count": 1,
                        },
                    },
                }
            },
        )

    def helper_binding_params(self) -> dict[str, object]:
        helper_symbol = self.make_symbol(
            symbol_id="helper",
            origin_type="callee",
            evidence_key="helper|sample.py|function_definition|callee|12..34|",
            byte_range=(12, 34),
        )
        return deep_merge(
            self.minimal_params(),
            {
                "patch": {
                    "updated_source": "def top_level() -> int:\n    return helper()\n",
                    "validation": {
                        "resolved_identifiers": [{"name": "helper", "symbol": helper_symbol}],
                        "binding_decisions": [
                            self.make_binding_decision(candidates=[helper_symbol])
                        ],
                        "commit_gate": {
                            "evidence_invariants": [self.make_evidence_invariant()],
                        },
                    },
                },
                "trace": {
                    "callees": [helper_symbol],
                    "evidence_keys": {
                        "callees": ["helper|sample.py|function_definition|callee|12..34|"]
                    },
                },
            },
        )

    def assert_invalid_for_trace_methods(
        self,
        params: dict[str, object],
        *,
        request_id: int,
        contains: str,
    ) -> None:
        for method in self.TRACE_METHODS:
            with self.subTest(method=method):
                response = self.call_gateway(
                    self.gateway,
                    method,
                    params,
                    request_id=request_id,
                )
                self.assert_jsonrpc_error(
                    response,
                    request_id=request_id,
                    code=-32602,
                    contains=contains,
                )

    def test_rejects_malformed_patch_trace_payloads_as_invalid_params(self) -> None:
        params = {
            "patch": {"file": "sample.py"},
            "trace": {"symbol": {}},
        }
        self.assert_invalid_for_trace_methods(
            params,
            request_id=49,
            contains="missing field",
        )

    def test_rejects_unknown_nested_patch_trace_fields_as_invalid_params(self) -> None:
        replay_params = deep_merge(
            self.minimal_params(),
            {
                "patch": {
                    "validation": {
                        "commit_gate": {"unexpected": True},
                    }
                }
            },
        )
        validate_params = deep_merge(
            self.minimal_params(),
            {
                "trace": {
                    "symbol": {"unexpected": True},
                }
            },
        )

        response = self.call_gateway(
            self.gateway,
            "arborist/replay_patch_evidence_against_trace",
            replay_params,
            request_id=52,
        )
        self.assert_jsonrpc_error(
            response,
            request_id=52,
            code=-32602,
            contains="unknown field",
        )

        response = self.call_gateway(
            self.gateway,
            "arborist/validate_patch_commit_with_trace",
            validate_params,
            request_id=52,
        )
        self.assert_jsonrpc_error(
            response,
            request_id=52,
            code=-32602,
            contains="unknown field",
        )

    def test_rejects_missing_nested_trace_fields_as_invalid_params(self) -> None:
        params = self.minimal_params()
        params["trace"]["symbol"] = {"symbol_id": "top_level"}  # type: ignore[index]
        self.assert_invalid_for_trace_methods(
            params,
            request_id=53,
            contains="missing field",
        )

    def test_rejects_missing_nested_patch_fields_as_invalid_params(self) -> None:
        cast_params = self.minimal_params()
        cast_params["patch"]["validation"].pop("unresolved_identifiers", None)  # type: ignore[index]
        self.assert_invalid_for_trace_methods(
            cast_params,
            request_id=54,
            contains="missing field",
        )

    def test_rejects_blank_patch_replay_evidence_keys_as_invalid_params(self) -> None:
        params = deep_merge(
            self.helper_binding_params(),
            {
                "patch": {
                    "validation": {
                        "commit_gate": {
                            "evidence_invariants": [
                                self.make_evidence_invariant(selected_evidence_key="   ")
                            ]
                        }
                    }
                }
            },
        )
        self.assert_invalid_for_trace_methods(
            params,
            request_id=55,
            contains="selected_evidence_key",
        )

    def test_rejects_tampered_syntax_error_details_as_invalid_params(self) -> None:
        params = deep_merge(
            self.syntax_error_params(),
            {
                "patch": {
                    "validation": {
                        "syntax_errors": [
                            {
                                "kind": "error",
                                "message": "manually tampered",
                                "start_byte": 0,
                                "end_byte": 1,
                                "start_point": {"row": 0, "column": 0},
                                "end_point": {"row": 0, "column": 1},
                            }
                        ]
                    }
                }
            },
        )
        self.assert_invalid_for_trace_methods(
            params,
            request_id=56,
            contains="patch.validation.syntax_errors",
        )

    def test_rejects_duplicate_candidate_evidence_keys_as_invalid_params(self) -> None:
        params = deep_merge(
            self.helper_binding_params(),
            {
                "patch": {
                    "validation": {
                        "commit_gate": {
                            "evidence_invariants": [
                                self.make_evidence_invariant(
                                    candidate_evidence_keys=[
                                        "helper|sample.py|function_definition|callee|12..34|",
                                        "helper|sample.py|function_definition|callee|12..34|",
                                    ]
                                )
                            ]
                        }
                    }
                }
            },
        )
        self.assert_invalid_for_trace_methods(
            params,
            request_id=57,
            contains="candidate_evidence_keys[1]",
        )

    def test_rejects_non_root_trace_symbol_origin_type_as_invalid_params(self) -> None:
        params = deep_merge(
            self.minimal_params(),
            {
                "trace": {
                    "symbol": {
                        "origin_type": "callee",
                        "evidence_key": "top_level|sample.py|function_definition|callee|0..10|",
                    },
                    "evidence_keys": {
                        "symbol": "top_level|sample.py|function_definition|callee|0..10|"
                    },
                }
            },
        )
        self.assert_invalid_for_trace_methods(
            params,
            request_id=58,
            contains="trace.symbol.origin_type",
        )

    def test_rejects_tampered_resolved_identifier_summaries_as_invalid_params(
        self,
    ) -> None:
        params = deep_merge(
            self.helper_binding_params(),
            {
                "patch": {
                    "validation": {
                        "resolved_identifiers": [],
                    }
                }
            },
        )
        self.assert_invalid_for_trace_methods(
            params,
            request_id=56,
            contains="resolved_identifiers",
        )

    def test_rejects_unsupported_binding_decision_statuses_as_invalid_params(self) -> None:
        params = deep_merge(
            self.helper_binding_params(),
            {
                "patch": {
                    "validation": {
                        "binding_decisions": [
                            self.make_binding_decision(
                                status="mystery",
                                reason="manually tampered",
                            )
                        ]
                    }
                }
            },
        )
        self.assert_invalid_for_trace_methods(
            params,
            request_id=56,
            contains="binding_decisions[0].status",
        )

    def test_rejects_inconsistent_trace_evidence_summaries_as_invalid_params(self) -> None:
        params = deep_merge(
            self.helper_binding_params(),
            {
                "trace": {
                    "evidence_keys": {
                        "callees": [],
                    }
                }
            },
        )
        self.assert_invalid_for_trace_methods(
            params,
            request_id=56,
            contains="trace.evidence_keys.callees",
        )

    def test_rejects_inconsistent_patch_gate_flags_as_invalid_params(self) -> None:
        params = deep_merge(
            self.minimal_params(),
            {
                "patch": {
                    "applied": False,
                }
            },
        )
        self.assert_invalid_for_trace_methods(
            params,
            request_id=57,
            contains="patch.applied",
        )

    def test_rejects_tampered_patch_commit_gate_reason_as_invalid_params(self) -> None:
        params = deep_merge(
            self.helper_binding_params(),
            {
                "patch": {
                    "validation": {
                        "commit_gate": {
                            "reason": "manually overridden",
                        }
                    }
                }
            },
        )
        self.assert_invalid_for_trace_methods(
            params,
            request_id=58,
            contains="commit_gate.reason",
        )

    def test_rejects_mismatched_trace_roots_as_invalid_params(self) -> None:
        params = deep_merge(
            self.helper_binding_params(),
            {
                "trace": {
                    "symbol": self.make_symbol(
                        symbol_id="helper",
                        origin_type="trace_root",
                        evidence_key="helper|sample.py|function_definition|trace_root|12..34|",
                        byte_range=(12, 34),
                        include_trace_fields=True,
                    ),
                    "evidence_keys": {
                        "symbol": "helper|sample.py|function_definition|trace_root|12..34|"
                    },
                }
            },
        )
        self.assert_invalid_for_trace_methods(
            params,
            request_id=59,
            contains="trace.symbol.symbol_id",
        )

    def test_rejects_mismatched_trace_root_files_as_invalid_params(self) -> None:
        params = deep_merge(
            self.minimal_params(),
            {
                "patch": {
                    "file": "sample_a.py",
                },
                "trace": {
                    "symbol": self.make_symbol(
                        symbol_id="top_level",
                        file_path="sample_b.py",
                        origin_type="trace_root",
                        evidence_key="top_level|sample_b.py|function_definition|trace_root|0..10|",
                        include_trace_fields=True,
                    ),
                    "evidence_keys": {
                        "symbol": "top_level|sample_b.py|function_definition|trace_root|0..10|"
                    },
                }
            },
        )
        self.assert_invalid_for_trace_methods(
            params,
            request_id=60,
            contains="trace.symbol.file_path",
        )

    def test_rejects_non_json_serializable_trace_object_as_invalid_params(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "arborist/validate_patch_commit_with_trace",
            {
                "patch": {},
                "trace": {"callee_keys": {1, 2}},
            },
            request_id=22,
        )
        self.assert_jsonrpc_error(response, request_id=22, code=-32602, contains="trace")

    def test_rejects_python_only_trace_values_as_invalid_params(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "arborist/validate_patch_commit_with_trace",
            {
                "patch": {},
                "trace": {"callee_keys": ("tuple", "is-not-json")},
            },
            request_id=51,
        )
        self.assert_jsonrpc_error(response, request_id=51, code=-32602, contains="trace")
