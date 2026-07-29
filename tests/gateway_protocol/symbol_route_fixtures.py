from __future__ import annotations

from tests.gateway_protocol.helpers import make_recording_json_core


class GatewaySymbolRouteFixtureMixin:
    def helper_symbol(
        self,
        *,
        file_path: str = "sample.py",
        origin_type: str = "workspace_symbol",
        include_trace_fields: bool = False,
        dependencies: list[str] | None = None,
        references: list[str] | None = None,
    ) -> dict[str, object]:
        return self.make_symbol(
            "helper",
            file_path=file_path,
            origin_type=origin_type,
            byte_range=(0, 10),
            include_trace_fields=include_trace_fields,
            dependencies=dependencies,
            references=references,
        )

    def orchestrate_symbol(
        self,
        *,
        file_path: str = "caller.py",
        origin_type: str = "workspace_symbol",
        include_trace_fields: bool = False,
        dependencies: list[str] | None = None,
        references: list[str] | None = None,
    ) -> dict[str, object]:
        return self.make_symbol(
            "orchestrate",
            file_path=file_path,
            origin_type=origin_type,
            byte_range=(0, 20),
            include_trace_fields=include_trace_fields,
            dependencies=dependencies,
            references=references,
        )

    def entrypoint_symbol(self) -> dict[str, object]:
        return self.make_symbol(
            "entrypoint",
            file_path="entry.py",
            origin_type="trace_caller",
            byte_range=(0, 20),
        )

    def helper_source(self) -> str:
        return "def helper() -> int:\n    return 1\n"

    def orchestrate_source(self) -> str:
        return "def orchestrate() -> int:\n    return helper()\n"

    def orchestrate_updated_source(self) -> str:
        return "def orchestrate(value: int) -> int:\n    return helper(value)\n"

    def make_search_result(self) -> dict[str, object]:
        return {
            "query": "helper",
            "indexed_files": 2,
            "total_matches": 1,
            "truncated": False,
            "matches": [self.helper_symbol()],
            "match_details": [
                {
                    "symbol_id": "helper",
                    "score": 1000,
                    "matched_fields": ["base_name", "semantic_path"],
                }
            ],
        }

    def make_list_result(self) -> dict[str, object]:
        return {
            "indexed_files": 2,
            "total_symbols": 1,
            "truncated": False,
            "symbols": [self.helper_symbol()],
        }

    def helper_read(self, *, file_path: str = "sample.py") -> dict[str, object]:
        return self.make_read(
            self.helper_symbol(file_path=file_path),
            source=self.helper_source(),
        )

    def orchestrate_read(
        self,
        *,
        file_path: str = "caller.py",
        source: str | None = None,
    ) -> dict[str, object]:
        return self.make_read(
            self.orchestrate_symbol(file_path=file_path),
            source=source or self.orchestrate_source(),
            indexed_files=3,
            end_point=(1, 18 if source is None else 24),
        )

    def helper_trace_context(self, *, file_path: str = "sample.py") -> dict[str, object]:
        return self.make_trace(
            self.helper_symbol(
                file_path=file_path,
                origin_type="trace_root",
                include_trace_fields=True,
                references=["orchestrate"],
            ),
            callers=[
                self.orchestrate_symbol(
                    file_path="caller.py" if file_path == "sample.py" else "graph_a.py",
                    origin_type="trace_caller",
                )
            ],
            indexed_files=2,
        )

    def helper_neighborhood_context(
        self,
        *,
        file_path: str = "sample.py",
    ) -> dict[str, object]:
        caller_file = "caller.py" if file_path == "sample.py" else "graph_a.py"
        helper_workspace = self.helper_symbol(file_path=file_path)
        helper_trace = self.helper_symbol(
            file_path=file_path,
            origin_type="trace_root",
            include_trace_fields=True,
            references=["orchestrate"],
        )
        orchestrate_caller = self.orchestrate_symbol(
            file_path=caller_file,
            origin_type="trace_caller",
        )
        return {
            "neighborhood": self.make_neighborhood(
                helper_trace,
                direction="callers",
                nodes=[(helper_workspace, 0), (orchestrate_caller, 1)],
                edges=[{"from_symbol_id": "orchestrate", "to_symbol_id": "helper"}],
                indexed_files=2,
            ),
            "reads": [
                self.helper_read(file_path=file_path),
                self.make_read(
                    orchestrate_caller,
                    source=self.orchestrate_source(),
                    end_point=(1, 18),
                ),
            ],
        }

    def orchestrate_trace_context(self) -> dict[str, object]:
        return self.make_trace(
            self.orchestrate_symbol(
                origin_type="trace_root",
                include_trace_fields=True,
                dependencies=["helper"],
                references=["entrypoint"],
            ),
            callers=[self.entrypoint_symbol()],
            callees=[self.helper_symbol(file_path="helper.py", origin_type="trace_callee")],
            indexed_files=3,
        )

    def orchestrate_neighborhood_context(self) -> dict[str, object]:
        orchestrate_workspace = self.orchestrate_symbol(file_path="caller.py")
        helper_callee = self.helper_symbol(
            file_path="helper.py",
            origin_type="trace_callee",
        )
        return {
            "neighborhood": self.make_neighborhood(
                self.orchestrate_symbol(
                    origin_type="trace_root",
                    include_trace_fields=True,
                    dependencies=["helper"],
                    references=["entrypoint"],
                ),
                direction="both",
                nodes=[(orchestrate_workspace, 0), (helper_callee, 1)],
                edges=[{"from_symbol_id": "orchestrate", "to_symbol_id": "helper"}],
                indexed_files=3,
            ),
            "reads": [
                self.make_read(
                    orchestrate_workspace,
                    source=self.orchestrate_updated_source(),
                    indexed_files=3,
                    end_point=(1, 24),
                ),
                self.make_read(
                    helper_callee,
                    source=self.helper_source(),
                    indexed_files=3,
                ),
            ],
        }

    def make_graph_context_payload(self) -> dict[str, object]:
        payload = {
            "patch": self.make_patch_result(),
            "trace_target": "orchestrate",
            "trace": self.orchestrate_trace_context(),
            "neighborhood": self.orchestrate_neighborhood_context()["neighborhood"],
            "trace_validation": self.make_trace_validation(),
            "trace_error": None,
        }
        return payload

    def make_neighborhood_context_payload(self) -> dict[str, object]:
        payload = self.make_graph_context_payload()
        payload["neighborhood_context"] = self.orchestrate_neighborhood_context()
        payload.pop("neighborhood")
        return payload

    def make_discovery_context_payload(self) -> dict[str, object]:
        payload = self.make_neighborhood_context_payload()
        payload["read"] = self.make_read(
            self.orchestrate_symbol(file_path="caller.py"),
            source=self.orchestrate_updated_source(),
            indexed_files=3,
            end_point=(1, 24),
        )
        return payload

    def assert_routed_json(
        self,
        *,
        core_method: str,
        rpc_method: str,
        params: dict[str, object],
        payload: object,
        request_id: int,
        expected_call: tuple[object, ...],
        check_result,
    ) -> None:
        core = make_recording_json_core(**{core_method: payload})
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                rpc_method,
                params,
                request_id=request_id,
            ),
            request_id=request_id,
        )
        check_result(result)
        self.assertEqual(core.calls_for(core_method), [expected_call])
