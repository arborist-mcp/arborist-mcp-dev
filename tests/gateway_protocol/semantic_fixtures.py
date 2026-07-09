from __future__ import annotations


class GatewaySemanticFixtureMixin:
    def make_symbol(
        self,
        symbol_id: str,
        *,
        file_path: str = "sample.py",
        origin_type: str = "workspace_symbol",
        byte_range: tuple[int, int] = (0, 10),
        semantic_path: str | None = None,
        node_kind: str = "function_definition",
        scope_path: str | None = None,
        include_trace_fields: bool = False,
        dependencies: list[str] | None = None,
        references: list[str] | None = None,
        evidence_key: str | None = None,
    ) -> dict[str, object]:
        start, end = byte_range
        symbol = {
            "symbol_id": symbol_id,
            "semantic_path": semantic_path or symbol_id,
            "scope_path": scope_path,
            "file_path": file_path,
            "node_kind": node_kind,
            "origin_type": origin_type,
            "evidence_key": evidence_key
            or f"{symbol_id}|{file_path}|{node_kind}|{origin_type}|{start}..{end}|",
            "byte_range": [start, end],
            "signature": None,
            "parameters": [],
            "return_type": None,
            "docstring": None,
        }
        if include_trace_fields:
            symbol["dependencies"] = dependencies or []
            symbol["references"] = references or []
        return symbol

    def make_read(
        self,
        symbol: dict[str, object],
        *,
        source: str,
        indexed_files: int = 2,
        start_point: tuple[int, int] = (0, 0),
        end_point: tuple[int, int] = (1, 12),
    ) -> dict[str, object]:
        return {
            "indexed_files": indexed_files,
            "symbol": symbol,
            "source": source,
            "start_point": {"row": start_point[0], "column": start_point[1]},
            "end_point": {"row": end_point[0], "column": end_point[1]},
        }

    def make_trace(
        self,
        symbol: dict[str, object],
        *,
        callers: list[dict[str, object]] | None = None,
        callees: list[dict[str, object]] | None = None,
        indexed_files: int = 2,
    ) -> dict[str, object]:
        callers = callers or []
        callees = callees or []
        return {
            "symbol": symbol,
            "callers": callers,
            "callees": callees,
            "evidence_keys": {
                "symbol": symbol["evidence_key"],
                "callers": [entry["evidence_key"] for entry in callers],
                "callees": [entry["evidence_key"] for entry in callees],
            },
            "indexed_files": indexed_files,
        }

    def make_neighborhood(
        self,
        symbol: dict[str, object],
        *,
        direction: str,
        nodes: list[tuple[dict[str, object], int]],
        edges: list[dict[str, str]],
        indexed_files: int = 2,
        max_depth: int = 2,
        max_nodes: int = 10,
    ) -> dict[str, object]:
        return {
            "symbol": symbol,
            "direction": direction,
            "max_depth": max_depth,
            "max_nodes": max_nodes,
            "truncated": False,
            "indexed_files": indexed_files,
            "nodes": [
                {"symbol": node_symbol, "depth": depth}
                for node_symbol, depth in nodes
            ],
            "edges": edges,
        }

    def make_evidence_invariant(
        self,
        *,
        name: str = "helper",
        status: str = "passed",
        reason: str = "resolved binding has one selected candidate evidence key",
        selected_evidence_key: str = "helper|sample.py|function_definition|callee|12..34|",
        candidate_evidence_keys: list[str] | None = None,
    ) -> dict[str, object]:
        return {
            "name": name,
            "status": status,
            "reason": reason,
            "selected_evidence_key": selected_evidence_key,
            "candidate_evidence_keys": candidate_evidence_keys
            or ["helper|sample.py|function_definition|callee|12..34|"],
        }

    def make_binding_decision(
        self,
        *,
        name: str = "helper",
        status: str = "resolved",
        reason: str = "resolved uniquely",
        selected_symbol_id: str = "helper",
        candidates: list[dict[str, object]] | None = None,
    ) -> dict[str, object]:
        return {
            "name": name,
            "status": status,
            "reason": reason,
            "selected_symbol_id": selected_symbol_id,
            "candidates": candidates
            or [
                self.make_symbol(
                    "helper",
                    origin_type="callee",
                    byte_range=(12, 34),
                    evidence_key="helper|sample.py|function_definition|callee|12..34|",
                )
            ],
        }

    def make_commit_gate(
        self,
        *,
        status: str = "allowed",
        allowed: bool = True,
        reason: str = "ok",
        bypass_reason: str | None = None,
        blocking_decisions: list[object] | None = None,
        evidence_invariants: list[dict[str, object]] | None = None,
        syntax_error_count: int = 0,
    ) -> dict[str, object]:
        return {
            "status": status,
            "allowed": allowed,
            "reason": reason,
            "bypass_reason": bypass_reason,
            "blocking_decisions": blocking_decisions or [],
            "evidence_invariants": evidence_invariants or [],
            "syntax_error_count": syntax_error_count,
        }

    def make_patch_validation(
        self,
        *,
        syntax_errors: list[dict[str, object]] | None = None,
        unresolved_identifiers: list[object] | None = None,
        resolved_identifiers: list[dict[str, object]] | None = None,
        ambiguous_identifiers: list[object] | None = None,
        binding_decisions: list[dict[str, object]] | None = None,
        commit_gate: dict[str, object] | None = None,
    ) -> dict[str, object]:
        return {
            "syntax_errors": syntax_errors or [],
            "unresolved_identifiers": unresolved_identifiers or [],
            "resolved_identifiers": resolved_identifiers or [],
            "ambiguous_identifiers": ambiguous_identifiers or [],
            "binding_decisions": binding_decisions or [],
            "commit_gate": commit_gate or self.make_commit_gate(),
        }

    def make_patch_result(
        self,
        *,
        file: str = "caller.py",
        target_path: str = "orchestrate",
        resolved_path: str = "orchestrate",
        resolved_symbol_id: str = "orchestrate",
        applied: bool = True,
        bypass_applied: bool = False,
        updated_source: str = "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        validation: dict[str, object] | None = None,
    ) -> dict[str, object]:
        return {
            "file": file,
            "target_path": target_path,
            "resolved_path": resolved_path,
            "resolved_symbol_id": resolved_symbol_id,
            "applied": applied,
            "bypass_applied": bypass_applied,
            "updated_source": updated_source,
            "validation": validation or self.make_patch_validation(),
        }

    def make_trace_validation(self) -> dict[str, object]:
        return {
            "allowed": True,
            "status": "allowed",
            "reason": "ok",
            "patch_gate_status": "allowed",
            "replay_status": "matched",
            "replay": {
                "consistent": True,
                "matched_items": 0,
                "blocked_items": 0,
                "items": [],
            },
        }
