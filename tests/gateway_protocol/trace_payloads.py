from __future__ import annotations

import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import arborist_mcp
from arborist_mcp import gateway as gateway_module
from arborist_mcp import _version as version_module
from arborist_mcp.gateway import ArboristGateway


class GatewayTracePayloadTests(unittest.TestCase):
    def test_rejects_malformed_patch_trace_payloads_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            (
                "arborist/replay_patch_evidence_against_trace",
                {
                    "patch": {"file": "sample.py"},
                    "trace": {"symbol": {}},
                },
            ),
            (
                "arborist/validate_patch_commit_with_trace",
                {
                    "patch": {"file": "sample.py"},
                    "trace": {"symbol": {}},
                },
            ),
        ]

        for method, params in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 49,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 49)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("missing field", response["error"]["message"])

    def test_rejects_unknown_nested_patch_trace_fields_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            (
                "arborist/replay_patch_evidence_against_trace",
                {
                    "patch": {
                        "file": "sample.py",
                        "target_path": "top_level",
                        "resolved_path": "top_level",
                        "resolved_symbol_id": "top_level",
                        "applied": True,
                        "bypass_applied": False,
                        "updated_source": "def top_level() -> int:\n    return 1\n",
                        "validation": {
                            "syntax_errors": [],
                            "unresolved_identifiers": [],
                            "resolved_identifiers": [],
                            "ambiguous_identifiers": [],
                            "binding_decisions": [],
                            "commit_gate": {
                                "status": "allowed",
                                "allowed": True,
                                "reason": "ok",
                                "bypass_reason": None,
                                "blocking_decisions": [],
                                "evidence_invariants": [],
                                "syntax_error_count": 0,
                                "unexpected": True,
                            },
                        },
                    },
                    "trace": {
                        "symbol": {
                            "symbol_id": "top_level",
                            "semantic_path": "top_level",
                            "file_path": "sample.py",
                            "node_kind": "function_definition",
                            "origin_type": "trace_root",
                            "evidence_key": "top_level|sample.py|function_definition|trace_root|0..10|",
                            "byte_range": [0, 10],
                            "parameters": [],
                            "dependencies": [],
                            "references": [],
                        },
                        "callers": [],
                        "callees": [],
                        "evidence_keys": {
                            "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                            "callers": [],
                            "callees": [],
                        },
                        "indexed_files": 1,
                    },
                },
            ),
            (
                "arborist/validate_patch_commit_with_trace",
                {
                    "patch": {
                        "file": "sample.py",
                        "target_path": "top_level",
                        "resolved_path": "top_level",
                        "resolved_symbol_id": "top_level",
                        "applied": True,
                        "bypass_applied": False,
                        "updated_source": "def top_level() -> int:\n    return 1\n",
                        "validation": {
                            "syntax_errors": [],
                            "unresolved_identifiers": [],
                            "resolved_identifiers": [],
                            "ambiguous_identifiers": [],
                            "binding_decisions": [],
                            "commit_gate": {
                                "status": "allowed",
                                "allowed": True,
                                "reason": "ok",
                                "bypass_reason": None,
                                "blocking_decisions": [],
                                "evidence_invariants": [],
                                "syntax_error_count": 0,
                            },
                        },
                    },
                    "trace": {
                        "symbol": {
                            "symbol_id": "top_level",
                            "semantic_path": "top_level",
                            "file_path": "sample.py",
                            "node_kind": "function_definition",
                            "origin_type": "trace_root",
                            "evidence_key": "top_level|sample.py|function_definition|trace_root|0..10|",
                            "byte_range": [0, 10],
                            "parameters": [],
                            "dependencies": [],
                            "references": [],
                            "unexpected": True,
                        },
                        "callers": [],
                        "callees": [],
                        "evidence_keys": {
                            "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                            "callers": [],
                            "callees": [],
                        },
                        "indexed_files": 1,
                    },
                },
            ),
        ]

        for method, params in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 52,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 52)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("unknown field", response["error"]["message"])

    def test_rejects_missing_nested_trace_fields_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": True,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return 1\n",
                "validation": {
                    "syntax_errors": [],
                    "unresolved_identifiers": [],
                    "resolved_identifiers": [],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [],
                    "commit_gate": {
                        "status": "allowed",
                        "allowed": True,
                        "reason": "ok",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [],
                        "syntax_error_count": 0,
                    },
                },
            },
            "trace": {
                "symbol": {"symbol_id": "top_level"},
                "callers": [],
                "callees": [],
                "evidence_keys": {
                    "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers": [],
                    "callees": [],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 53,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 53)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("missing field", response["error"]["message"])

    def test_rejects_missing_nested_patch_fields_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": True,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return 1\n",
                "validation": {
                    "syntax_errors": [],
                    "resolved_identifiers": [],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [],
                    "commit_gate": {
                        "status": "allowed",
                        "allowed": True,
                        "reason": "ok",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [],
                        "syntax_error_count": 0,
                    },
                },
            },
            "trace": {
                "symbol": {
                    "symbol_id": "top_level",
                    "semantic_path": "top_level",
                    "file_path": "sample.py",
                    "node_kind": "function_definition",
                    "origin_type": "trace_root",
                    "evidence_key": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range": [0, 10],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [],
                "evidence_keys": {
                    "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers": [],
                    "callees": [],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 54,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 54)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("missing field", response["error"]["message"])

    def test_rejects_blank_patch_replay_evidence_keys_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": True,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return 1\n",
                "validation": {
                    "syntax_errors": [],
                    "unresolved_identifiers": [],
                    "resolved_identifiers": [],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [],
                    "commit_gate": {
                        "status": "allowed",
                        "allowed": True,
                        "reason": "ok",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [
                            {
                                "name": "helper",
                                "status": "passed",
                                "reason": "ok",
                                "selected_evidence_key": "   ",
                                "candidate_evidence_keys": [
                                    "top_level|sample.py|function_definition|trace_root|0..10|"
                                ],
                            }
                        ],
                        "syntax_error_count": 0,
                    },
                },
            },
            "trace": {
                "symbol": {
                    "symbol_id": "top_level",
                    "semantic_path": "top_level",
                    "file_path": "sample.py",
                    "node_kind": "function_definition",
                    "origin_type": "trace_root",
                    "evidence_key": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range": [0, 10],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [],
                "evidence_keys": {
                    "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers": [],
                    "callees": [],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 55,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 55)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("selected_evidence_key", response["error"]["message"])

    def test_rejects_tampered_syntax_error_details_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": False,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return (\n",
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
                    ],
                    "unresolved_identifiers": [],
                    "resolved_identifiers": [],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [],
                    "commit_gate": {
                        "status": "rejected",
                        "allowed": False,
                        "reason": "syntax validation failed",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [],
                        "syntax_error_count": 1,
                    },
                },
            },
            "trace": {
                "symbol": {
                    "symbol_id": "top_level",
                    "semantic_path": "top_level",
                    "file_path": "sample.py",
                    "node_kind": "function_definition",
                    "origin_type": "trace_root",
                    "evidence_key": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range": [0, 10],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [],
                "evidence_keys": {
                    "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers": [],
                    "callees": [],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 56,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 56)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("patch.validation.syntax_errors", response["error"]["message"])

    def test_rejects_duplicate_candidate_evidence_keys_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": True,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return helper()\n",
                "validation": {
                    "syntax_errors": [],
                    "unresolved_identifiers": [],
                    "resolved_identifiers": [
                        {
                            "name": "helper",
                            "symbol": {
                                "symbol_id": "helper",
                                "semantic_path": "helper",
                                "scope_path": None,
                                "file_path": "sample.py",
                                "node_kind": "function_definition",
                                "origin_type": "callee",
                                "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                "byte_range": [12, 34],
                                "signature": None,
                                "parameters": [],
                                "return_type": None,
                                "docstring": None,
                            },
                        }
                    ],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [
                        {
                            "name": "helper",
                            "status": "resolved",
                            "reason": "resolved uniquely",
                            "selected_symbol_id": "helper",
                            "candidates": [
                                {
                                    "symbol_id": "helper",
                                    "semantic_path": "helper",
                                    "scope_path": None,
                                    "file_path": "sample.py",
                                    "node_kind": "function_definition",
                                    "origin_type": "callee",
                                    "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                    "byte_range": [12, 34],
                                    "signature": None,
                                    "parameters": [],
                                    "return_type": None,
                                    "docstring": None,
                                }
                            ],
                        }
                    ],
                    "commit_gate": {
                        "status": "allowed",
                        "allowed": True,
                        "reason": "syntax and symbol binding validation passed",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [
                            {
                                "name": "helper",
                                "status": "passed",
                                "reason": "resolved binding has one selected candidate evidence key",
                                "selected_evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys": [
                                    "helper|sample.py|function_definition|callee|12..34|",
                                    "helper|sample.py|function_definition|callee|12..34|",
                                ],
                            }
                        ],
                        "syntax_error_count": 0,
                    },
                },
            },
            "trace": {
                "symbol": {
                    "symbol_id": "top_level",
                    "semantic_path": "top_level",
                    "file_path": "sample.py",
                    "node_kind": "function_definition",
                    "origin_type": "trace_root",
                    "evidence_key": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range": [0, 10],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [
                    {
                        "symbol_id": "helper",
                        "semantic_path": "helper",
                        "scope_path": None,
                        "file_path": "sample.py",
                        "node_kind": "function_definition",
                        "origin_type": "callee",
                        "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                        "byte_range": [12, 34],
                        "signature": None,
                        "parameters": [],
                        "return_type": None,
                        "docstring": None,
                    }
                ],
                "evidence_keys": {
                    "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers": [],
                    "callees": ["helper|sample.py|function_definition|callee|12..34|"],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 57,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 57)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("candidate_evidence_keys[1]", response["error"]["message"])

    def test_rejects_non_root_trace_symbol_origin_type_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
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
                "symbol": {
                    "symbol_id": "top_level",
                    "semantic_path": "top_level",
                    "file_path": "sample.py",
                    "node_kind": "function_definition",
                    "origin_type": "callee",
                    "evidence_key": "top_level|sample.py|function_definition|callee|0..10|",
                    "byte_range": [0, 10],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [],
                "evidence_keys": {
                    "symbol": "top_level|sample.py|function_definition|callee|0..10|",
                    "callers": [],
                    "callees": [],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 58,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 58)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("trace.symbol.origin_type", response["error"]["message"])

    def test_rejects_tampered_resolved_identifier_summaries_as_invalid_params(
        self,
    ) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": True,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return helper()\n",
                "validation": {
                    "syntax_errors": [],
                    "unresolved_identifiers": [],
                    "resolved_identifiers": [],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [
                        {
                            "name": "helper",
                            "status": "resolved",
                            "reason": "resolved uniquely",
                            "selected_symbol_id": "helper",
                            "candidates": [
                                {
                                    "symbol_id": "helper",
                                    "semantic_path": "helper",
                                    "scope_path": None,
                                    "file_path": "sample.py",
                                    "node_kind": "function_definition",
                                    "origin_type": "callee",
                                    "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                    "byte_range": [12, 34],
                                    "signature": None,
                                    "parameters": [],
                                    "return_type": None,
                                    "docstring": None,
                                }
                            ],
                        }
                    ],
                    "commit_gate": {
                        "status": "allowed",
                        "allowed": True,
                        "reason": "syntax and symbol binding validation passed",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [
                            {
                                "name": "helper",
                                "status": "passed",
                                "reason": "resolved binding has one selected candidate evidence key",
                                "selected_evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys": [
                                    "helper|sample.py|function_definition|callee|12..34|"
                                ],
                            }
                        ],
                        "syntax_error_count": 0,
                    },
                },
            },
            "trace": {
                "symbol": {
                    "symbol_id": "top_level",
                    "semantic_path": "top_level",
                    "file_path": "sample.py",
                    "node_kind": "function_definition",
                    "origin_type": "trace_root",
                    "evidence_key": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range": [0, 10],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [
                    {
                        "symbol_id": "helper",
                        "semantic_path": "helper",
                        "scope_path": None,
                        "file_path": "sample.py",
                        "node_kind": "function_definition",
                        "origin_type": "callee",
                        "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                        "byte_range": [12, 34],
                        "signature": None,
                        "parameters": [],
                        "return_type": None,
                        "docstring": None,
                    }
                ],
                "evidence_keys": {
                    "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers": [],
                    "callees": ["helper|sample.py|function_definition|callee|12..34|"],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 56,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 56)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("resolved_identifiers", response["error"]["message"])

    def test_rejects_unsupported_binding_decision_statuses_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": True,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return helper()\n",
                "validation": {
                    "syntax_errors": [],
                    "unresolved_identifiers": [],
                    "resolved_identifiers": [
                        {
                            "name": "helper",
                            "symbol": {
                                "symbol_id": "helper",
                                "semantic_path": "helper",
                                "scope_path": None,
                                "file_path": "sample.py",
                                "node_kind": "function_definition",
                                "origin_type": "callee",
                                "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                "byte_range": [12, 34],
                                "signature": None,
                                "parameters": [],
                                "return_type": None,
                                "docstring": None,
                            },
                        }
                    ],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [
                        {
                            "name": "helper",
                            "status": "mystery",
                            "reason": "manually tampered",
                            "selected_symbol_id": "helper",
                            "candidates": [
                                {
                                    "symbol_id": "helper",
                                    "semantic_path": "helper",
                                    "scope_path": None,
                                    "file_path": "sample.py",
                                    "node_kind": "function_definition",
                                    "origin_type": "callee",
                                    "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                    "byte_range": [12, 34],
                                    "signature": None,
                                    "parameters": [],
                                    "return_type": None,
                                    "docstring": None,
                                }
                            ],
                        }
                    ],
                    "commit_gate": {
                        "status": "allowed",
                        "allowed": True,
                        "reason": "syntax and symbol binding validation passed",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [
                            {
                                "name": "helper",
                                "status": "passed",
                                "reason": "resolved binding has one selected candidate evidence key",
                                "selected_evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys": [
                                    "helper|sample.py|function_definition|callee|12..34|"
                                ],
                            }
                        ],
                        "syntax_error_count": 0,
                    },
                },
            },
            "trace": {
                "symbol": {
                    "symbol_id": "top_level",
                    "semantic_path": "top_level",
                    "file_path": "sample.py",
                    "node_kind": "function_definition",
                    "origin_type": "trace_root",
                    "evidence_key": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range": [0, 10],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [
                    {
                        "symbol_id": "helper",
                        "semantic_path": "helper",
                        "scope_path": None,
                        "file_path": "sample.py",
                        "node_kind": "function_definition",
                        "origin_type": "callee",
                        "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                        "byte_range": [12, 34],
                        "signature": None,
                        "parameters": [],
                        "return_type": None,
                        "docstring": None,
                    }
                ],
                "evidence_keys": {
                    "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers": [],
                    "callees": ["helper|sample.py|function_definition|callee|12..34|"],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 56,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 56)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("binding_decisions[0].status", response["error"]["message"])

    def test_rejects_inconsistent_trace_evidence_summaries_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": True,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return helper()\n",
                "validation": {
                    "syntax_errors": [],
                    "unresolved_identifiers": [],
                    "resolved_identifiers": [
                        {
                            "name": "helper",
                            "symbol": {
                                "symbol_id": "helper",
                                "semantic_path": "helper",
                                "scope_path": None,
                                "file_path": "sample.py",
                                "node_kind": "function_definition",
                                "origin_type": "callee",
                                "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                "byte_range": [12, 34],
                                "signature": None,
                                "parameters": [],
                                "return_type": None,
                                "docstring": None,
                            },
                        }
                    ],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [
                        {
                            "name": "helper",
                            "status": "resolved",
                            "reason": "resolved uniquely",
                            "selected_symbol_id": "helper",
                            "candidates": [
                                {
                                    "symbol_id": "helper",
                                    "semantic_path": "helper",
                                    "scope_path": None,
                                    "file_path": "sample.py",
                                    "node_kind": "function_definition",
                                    "origin_type": "callee",
                                    "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                    "byte_range": [12, 34],
                                    "signature": None,
                                    "parameters": [],
                                    "return_type": None,
                                    "docstring": None,
                                }
                            ],
                        }
                    ],
                    "commit_gate": {
                        "status": "allowed",
                        "allowed": True,
                        "reason": "syntax and symbol binding validation passed",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [
                            {
                                "name": "helper",
                                "status": "passed",
                                "reason": "resolved binding has one selected candidate evidence key",
                                "selected_evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys": [
                                    "helper|sample.py|function_definition|callee|12..34|"
                                ],
                            }
                        ],
                        "syntax_error_count": 0,
                    },
                },
            },
            "trace": {
                "symbol": {
                    "symbol_id": "top_level",
                    "semantic_path": "top_level",
                    "file_path": "sample.py",
                    "node_kind": "function_definition",
                    "origin_type": "trace_root",
                    "evidence_key": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range": [0, 10],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [
                    {
                        "symbol_id": "helper",
                        "semantic_path": "helper",
                        "file_path": "sample.py",
                        "node_kind": "function_definition",
                        "origin_type": "callee",
                        "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                        "byte_range": [12, 34],
                        "parameters": [],
                    }
                ],
                "evidence_keys": {
                    "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers": [],
                    "callees": [],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 56,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 56)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("trace.evidence_keys.callees", response["error"]["message"])

    def test_rejects_inconsistent_patch_gate_flags_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": False,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return 1\n",
                "validation": {
                    "syntax_errors": [],
                    "unresolved_identifiers": [],
                    "resolved_identifiers": [],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [],
                    "commit_gate": {
                        "status": "allowed",
                        "allowed": True,
                        "reason": "ok",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [],
                        "syntax_error_count": 0,
                    },
                },
            },
            "trace": {
                "symbol": {
                    "symbol_id": "top_level",
                    "semantic_path": "top_level",
                    "file_path": "sample.py",
                    "node_kind": "function_definition",
                    "origin_type": "trace_root",
                    "evidence_key": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range": [0, 10],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [],
                "evidence_keys": {
                    "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers": [],
                    "callees": [],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 57,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 57)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("patch.applied", response["error"]["message"])

    def test_rejects_tampered_patch_commit_gate_reason_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": True,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return helper()\n",
                "validation": {
                    "syntax_errors": [],
                    "unresolved_identifiers": [],
                    "resolved_identifiers": [
                        {
                            "name": "helper",
                            "symbol": {
                                "symbol_id": "helper",
                                "semantic_path": "helper",
                                "scope_path": None,
                                "file_path": "sample.py",
                                "node_kind": "function_definition",
                                "origin_type": "callee",
                                "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                "byte_range": [12, 34],
                                "signature": None,
                                "parameters": [],
                                "return_type": None,
                                "docstring": None,
                            },
                        }
                    ],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [
                        {
                            "name": "helper",
                            "status": "resolved",
                            "reason": "resolved uniquely",
                            "selected_symbol_id": "helper",
                            "candidates": [
                                {
                                    "symbol_id": "helper",
                                    "semantic_path": "helper",
                                    "scope_path": None,
                                    "file_path": "sample.py",
                                    "node_kind": "function_definition",
                                    "origin_type": "callee",
                                    "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                    "byte_range": [12, 34],
                                    "signature": None,
                                    "parameters": [],
                                    "return_type": None,
                                    "docstring": None,
                                }
                            ],
                        }
                    ],
                    "commit_gate": {
                        "status": "allowed",
                        "allowed": True,
                        "reason": "manually overridden",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [
                            {
                                "name": "helper",
                                "status": "passed",
                                "reason": "resolved binding has one selected candidate evidence key",
                                "selected_evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys": [
                                    "helper|sample.py|function_definition|callee|12..34|"
                                ],
                            }
                        ],
                        "syntax_error_count": 0,
                    },
                },
            },
            "trace": {
                "symbol": {
                    "symbol_id": "top_level",
                    "semantic_path": "top_level",
                    "file_path": "sample.py",
                    "node_kind": "function_definition",
                    "origin_type": "trace_root",
                    "evidence_key": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "byte_range": [0, 10],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [
                    {
                        "symbol_id": "helper",
                        "semantic_path": "helper",
                        "scope_path": None,
                        "file_path": "sample.py",
                        "node_kind": "function_definition",
                        "origin_type": "callee",
                        "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                        "byte_range": [12, 34],
                        "signature": None,
                        "parameters": [],
                        "return_type": None,
                        "docstring": None,
                    }
                ],
                "evidence_keys": {
                    "symbol": "top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers": [],
                    "callees": ["helper|sample.py|function_definition|callee|12..34|"],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 58,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 58)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("commit_gate.reason", response["error"]["message"])

    def test_rejects_mismatched_trace_roots_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": True,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return helper()\n",
                "validation": {
                    "syntax_errors": [],
                    "unresolved_identifiers": [],
                    "resolved_identifiers": [
                        {
                            "name": "helper",
                            "symbol": {
                                "symbol_id": "helper",
                                "semantic_path": "helper",
                                "scope_path": None,
                                "file_path": "sample.py",
                                "node_kind": "function_definition",
                                "origin_type": "callee",
                                "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                "byte_range": [12, 34],
                                "signature": None,
                                "parameters": [],
                                "return_type": None,
                                "docstring": None,
                            },
                        }
                    ],
                    "ambiguous_identifiers": [],
                    "binding_decisions": [
                        {
                            "name": "helper",
                            "status": "resolved",
                            "reason": "resolved uniquely",
                            "selected_symbol_id": "helper",
                            "candidates": [
                                {
                                    "symbol_id": "helper",
                                    "semantic_path": "helper",
                                    "scope_path": None,
                                    "file_path": "sample.py",
                                    "node_kind": "function_definition",
                                    "origin_type": "callee",
                                    "evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                    "byte_range": [12, 34],
                                    "signature": None,
                                    "parameters": [],
                                    "return_type": None,
                                    "docstring": None,
                                }
                            ],
                        }
                    ],
                    "commit_gate": {
                        "status": "allowed",
                        "allowed": True,
                        "reason": "syntax and symbol binding validation passed",
                        "bypass_reason": None,
                        "blocking_decisions": [],
                        "evidence_invariants": [
                            {
                                "name": "helper",
                                "status": "passed",
                                "reason": "resolved binding has one selected candidate evidence key",
                                "selected_evidence_key": "helper|sample.py|function_definition|callee|12..34|",
                                "candidate_evidence_keys": [
                                    "helper|sample.py|function_definition|callee|12..34|"
                                ],
                            }
                        ],
                        "syntax_error_count": 0,
                    },
                },
            },
            "trace": {
                "symbol": {
                    "symbol_id": "helper",
                    "semantic_path": "helper",
                    "file_path": "sample.py",
                    "node_kind": "function_definition",
                    "origin_type": "trace_root",
                    "evidence_key": "helper|sample.py|function_definition|trace_root|12..34|",
                    "byte_range": [12, 34],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [],
                "evidence_keys": {
                    "symbol": "helper|sample.py|function_definition|trace_root|12..34|",
                    "callers": [],
                    "callees": [],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 59,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 59)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("trace.symbol.symbol_id", response["error"]["message"])

    def test_rejects_mismatched_trace_root_files_as_invalid_params(self) -> None:
        gateway = ArboristGateway()

        cases = [
            "arborist/replay_patch_evidence_against_trace",
            "arborist/validate_patch_commit_with_trace",
        ]

        params = {
            "patch": {
                "file": "sample_a.py",
                "target_path": "top_level",
                "resolved_path": "top_level",
                "resolved_symbol_id": "top_level",
                "applied": True,
                "bypass_applied": False,
                "updated_source": "def top_level() -> int:\n    return 1\n",
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
                "symbol": {
                    "symbol_id": "top_level",
                    "semantic_path": "top_level",
                    "file_path": "sample_b.py",
                    "node_kind": "function_definition",
                    "origin_type": "trace_root",
                    "evidence_key": "top_level|sample_b.py|function_definition|trace_root|0..10|",
                    "byte_range": [0, 10],
                    "parameters": [],
                    "dependencies": [],
                    "references": [],
                },
                "callers": [],
                "callees": [],
                "evidence_keys": {
                    "symbol": "top_level|sample_b.py|function_definition|trace_root|0..10|",
                    "callers": [],
                    "callees": [],
                },
                "indexed_files": 1,
            },
        }

        for method in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 60,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 60)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("trace.symbol.file_path", response["error"]["message"])

    def test_rejects_non_json_serializable_trace_object_as_invalid_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 22,
                "method": "arborist/validate_patch_commit_with_trace",
                "params": {
                    "patch": {},
                    "trace": {"callee_keys": {1, 2}},
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 22)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("trace", response["error"]["message"])

    def test_rejects_python_only_trace_values_as_invalid_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 51,
                "method": "arborist/validate_patch_commit_with_trace",
                "params": {
                    "patch": {},
                    "trace": {"callee_keys": ("tuple", "is-not-json")},
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 51)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("trace", response["error"]["message"])
