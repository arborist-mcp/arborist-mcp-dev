from __future__ import annotations

import ast
import importlib
import io
import json
import pickle
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from unittest import mock

import arborist_mcp
from arborist_mcp import gateway as gateway_module
from arborist_mcp import tool_definitions as tool_definitions_module
from arborist_mcp import tool_manifest as tool_manifest_module
from arborist_mcp import tool_spec_models as tool_spec_models_module
from arborist_mcp import tool_specs as tool_specs_module
from arborist_mcp import _version as version_module
from arborist_mcp.gateway import ArboristGateway
from arborist_mcp.gateway_core_helpers import GatewayCoreHelpers
from arborist_mcp.gateway_source_query_routes import GatewaySourceQueryRoutes

from tests.gateway_protocol import (
    GROUP_MODULES,
    GROUP_SUITES,
    GROUPS,
    MANIFEST,
    SUITE_MODULES,
    SUITES,
    build_manifest_snapshot,
)


class GatewayMetadataRequestValidationMixin:
    def test_gateway_reuses_package_version(self) -> None:
        self.assertEqual(gateway_module.__version__, arborist_mcp.__version__)
        self.assertEqual(gateway_module.__version__, version_module.__version__)

    def test_repo_root_shim_extends_package_path_for_extra_install_locations(self) -> None:
        repo_root = Path(__file__).resolve().parents[2]

        with tempfile.TemporaryDirectory() as temp_dir:
            package_dir = Path(temp_dir) / "arborist_mcp"
            package_dir.mkdir()
            package_dir.joinpath("_probe.py").write_text(
                "VALUE = 'probe-loaded'\n",
                encoding="utf-8",
                newline="\n",
            )

            completed = subprocess.run(
                [
                    sys.executable,
                    "-c",
                    (
                        "import json, sys; "
                        f"sys.path[:0] = [{str(repo_root)!r}, {temp_dir!r}]; "
                        "import arborist_mcp; "
                        "from arborist_mcp import _probe; "
                        "print(json.dumps({'path': list(arborist_mcp.__path__), 'value': _probe.VALUE}))"
                    ),
                ],
                cwd=repo_root,
                check=True,
                capture_output=True,
                text=True,
            )

        payload = json.loads(completed.stdout)
        self.assertEqual(payload["value"], "probe-loaded")
        normalized_paths = [entry.replace("\\", "/") for entry in payload["path"]]
        self.assertIn(str(package_dir).replace("\\", "/"), normalized_paths)

    def test_cli_version_reports_package_version(self) -> None:
        stdout = io.StringIO()

        with mock.patch("sys.stdout", stdout):
            with self.assertRaises(SystemExit) as context:
                gateway_module.main(["--version"])

        self.assertEqual(context.exception.code, 0)
        self.assertIn(gateway_module.__version__, stdout.getvalue())

    def test_advertised_tools_have_gateway_handlers(self) -> None:
        self.assertEqual(gateway_module.TOOL_NAMES, tuple(gateway_module.TOOL_HANDLERS))
        for handler_name in gateway_module.TOOL_HANDLERS.values():
            with self.subTest(handler_name=handler_name):
                self.assertTrue(callable(getattr(ArboristGateway, handler_name, None)))

    def test_source_query_handlers_are_composed_from_route_mixin(self) -> None:
        self.assertTrue(issubclass(ArboristGateway, GatewaySourceQueryRoutes))
        for handler_name in ("_get_semantic_skeleton", "_execute_tree_query"):
            with self.subTest(handler_name=handler_name):
                self.assertNotIn(handler_name, ArboristGateway.__dict__)
                self.assertIs(
                    getattr(ArboristGateway, handler_name),
                    getattr(GatewaySourceQueryRoutes, handler_name),
                )

    def test_shared_core_helpers_are_composed_from_dedicated_mixin(self) -> None:
        self.assertTrue(issubclass(ArboristGateway, GatewayCoreHelpers))
        helper_names = (
            "_call_with_optional_timeout",
            "_decode_core_payload",
            "_decode_core_object",
            "_decode_core_object_array",
        )
        for helper_name in helper_names:
            with self.subTest(helper_name=helper_name):
                self.assertNotIn(helper_name, ArboristGateway.__dict__)
                self.assertIs(
                    getattr(ArboristGateway, helper_name),
                    getattr(GatewayCoreHelpers, helper_name),
                )
        self.assertNotIn(
            "_call_with_optional_timeout",
            gateway_module.GatewaySymbolRoutes.__dict__,
        )

    def test_path_validation_helpers_are_composed_from_parameter_mixin(self) -> None:
        parameter_mixin = gateway_module.GatewayParameterValidation
        self.assertIs(ArboristGateway.__bases__[-1], parameter_mixin)
        helper_names = (
            "_require_file_path_for_source",
            "_ensure_write_path_inside_server_workspace",
        )
        for helper_name in helper_names:
            with self.subTest(helper_name=helper_name):
                self.assertNotIn(helper_name, ArboristGateway.__dict__)
                self.assertIsInstance(
                    parameter_mixin.__dict__[helper_name],
                    staticmethod,
                )
                self.assertIs(
                    getattr(ArboristGateway, helper_name),
                    getattr(parameter_mixin, helper_name),
                )

    def test_domain_result_schemas_do_not_eagerly_load_tool_registry(self) -> None:
        module_names = (
            "tool_result_schema_common",
            "tool_result_schema_symbols",
            "tool_result_schema_patching",
            "tool_result_schema_query",
            "tool_result_schema_vfs",
            "tool_result_schema_index",
        )

        for module_name in module_names:
            with self.subTest(module_name=module_name):
                completed = subprocess.run(
                    [
                        sys.executable,
                        "-c",
                        (
                            "import importlib, sys; "
                            f"importlib.import_module('arborist_mcp.{module_name}'); "
                            "assert 'arborist_mcp.tool_specs' not in sys.modules"
                        ),
                    ],
                    check=False,
                    capture_output=True,
                    text=True,
                )

                self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_tool_definitions_preserve_registry_identity(self) -> None:
        registry_names = (
            "TOOL_SPECS",
            "TOOL_NAMES",
            "TOOL_SPECS_BY_NAME",
            "TOOL_HANDLERS",
            "TOOL_PARAM_NAMES",
            "TOOL_CATEGORIES",
        )
        for name in registry_names:
            with self.subTest(name=name):
                self.assertIs(
                    getattr(tool_specs_module, name),
                    getattr(tool_definitions_module, name),
                )

        completed = subprocess.run(
            [
                sys.executable,
                "-c",
                (
                    "import sys; "
                    "import arborist_mcp.tool_definitions; "
                    "assert 'arborist_mcp.tool_specs' not in sys.modules; "
                    "assert 'arborist_mcp.tool_param_specs' not in sys.modules"
                ),
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_tool_spec_models_preserve_compatibility_identity(self) -> None:
        model_types = (
            (tool_specs_module.ToolSpec, tool_spec_models_module.ToolSpec),
            (tool_specs_module.ToolParamSpec, tool_spec_models_module.ToolParamSpec),
        )
        for exported_type, model_type in model_types:
            with self.subTest(model_type=model_type.__name__):
                self.assertIs(exported_type, model_type)
                self.assertEqual(model_type.__module__, "arborist_mcp.tool_specs")
                self.assertIs(pickle.loads(pickle.dumps(exported_type)), exported_type)

        values = (
            tool_specs_module.ToolSpec("tool", "handler", (), "read"),
            tool_specs_module.ToolParamSpec({"type": "string"}),
        )
        for value in values:
            with self.subTest(value_type=type(value).__name__):
                restored = pickle.loads(pickle.dumps(value))
                self.assertIs(type(restored), type(value))
                self.assertEqual(restored, value)

    def test_tool_specs_are_the_catalog_source_of_truth(self) -> None:
        specs = gateway_module.TOOL_SPECS
        self.assertEqual(len({spec.name for spec in specs}), len(specs))
        self.assertEqual(gateway_module.TOOL_NAMES, tuple(spec.name for spec in specs))
        self.assertEqual(
            gateway_module.TOOL_HANDLERS,
            {spec.name: spec.handler for spec in specs},
        )
        self.assertEqual(
            gateway_module.TOOL_PARAM_NAMES,
            {spec.name: spec.params for spec in specs},
        )
        self.assertEqual(
            gateway_module.TOOL_CATEGORIES,
            {spec.name: spec.category for spec in specs},
        )
        self.assertTrue(
            {spec.category for spec in specs} <= {"read", "write", "vfs", "index", "trace"}
        )

    def test_advertised_tools_have_param_specs(self) -> None:
        self.assertEqual(
            set(gateway_module.TOOL_HANDLERS),
            set(gateway_module.TOOL_PARAM_NAMES),
        )

    def test_advertised_tool_params_have_schema_specs(self) -> None:
        expected_params = {
            param_name
            for param_names in gateway_module.TOOL_PARAM_NAMES.values()
            for param_name in param_names
        }

        self.assertEqual(expected_params, set(gateway_module.TOOL_PARAM_SCHEMAS))
        self.assertEqual(expected_params, set(gateway_module.TOOL_PARAM_SPECS))

    def test_tool_param_specs_drive_optional_defaults_and_length_maps(self) -> None:
        self.assertEqual(
            gateway_module.OPTIONAL_TOOL_PARAMS,
            frozenset(
                name
                for name, spec in gateway_module.TOOL_PARAM_SPECS.items()
                if spec.optional
            ),
        )
        self.assertEqual(
            gateway_module.TOOL_PARAM_DEFAULTS,
            {
                name: spec.default
                for name, spec in gateway_module.TOOL_PARAM_SPECS.items()
                if spec.default is not None
            },
        )
        self.assertEqual(
            gateway_module.STRING_PARAM_MAX_LENGTHS,
            {
                name: spec.string_max_length
                for name, spec in gateway_module.TOOL_PARAM_SPECS.items()
                if spec.string_max_length is not None
            },
        )
        self.assertEqual(
            {
                name: spec.schema["maximum"]
                for name, spec in gateway_module.TOOL_PARAM_SPECS.items()
                if spec.int_max_value is not None
            },
            {
                name: spec.int_max_value
                for name, spec in gateway_module.TOOL_PARAM_SPECS.items()
                if spec.int_max_value is not None
            },
        )
        self.assertEqual(
            gateway_module.SOURCE_ANCHORED_OPTIONAL_FILE_PATH_TOOLS,
            frozenset(
                tool_name
                for spec in gateway_module.TOOL_PARAM_SPECS.values()
                for tool_name in spec.source_anchored_optional_tools
            ),
        )

    def test_route_runtime_defaults_match_tool_manifest(self) -> None:
        repo_root = Path(__file__).resolve().parents[2]
        route_methods: dict[str, ast.FunctionDef | ast.AsyncFunctionDef] = {}
        calls_by_method: dict[str, set[str]] = {}
        default_calls: list[tuple[str, str, object, int]] = []

        gateway_package = repo_root.joinpath("python", "arborist_mcp")
        gateway_paths = (
            gateway_package.joinpath("gateway.py"),
            *sorted(gateway_package.glob("gateway*_routes.py")),
        )
        for route_path in gateway_paths:
            tree = ast.parse(
                route_path.read_text(encoding="utf-8"),
                filename=str(route_path),
            )
            for class_node in tree.body:
                if not isinstance(class_node, ast.ClassDef):
                    continue
                for method in class_node.body:
                    if not isinstance(method, (ast.FunctionDef, ast.AsyncFunctionDef)):
                        continue
                    self.assertNotIn(method.name, route_methods)
                    route_methods[method.name] = method
                    calls_by_method[method.name] = {
                        call.func.attr
                        for call in ast.walk(method)
                        if isinstance(call, ast.Call)
                        and isinstance(call.func, ast.Attribute)
                        and isinstance(call.func.value, ast.Name)
                        and call.func.value.id == "self"
                    }
                    for call in ast.walk(method):
                        if (
                            not isinstance(call, ast.Call)
                            or not isinstance(call.func, ast.Attribute)
                            or not call.func.attr.startswith("_optional_")
                        ):
                            continue
                        default_keyword = next(
                            (keyword for keyword in call.keywords if keyword.arg == "default"),
                            None,
                        )
                        if default_keyword is None:
                            continue
                        self.assertGreaterEqual(len(call.args), 2)
                        param_node = call.args[1]
                        self.assertIsInstance(param_node, ast.Constant)
                        self.assertIsInstance(param_node.value, str)
                        try:
                            runtime_default = ast.literal_eval(default_keyword.value)
                        except (ValueError, TypeError) as error:
                            self.fail(
                                f"{route_path.name}:{call.lineno} has a non-literal "
                                f"runtime default: {error}"
                            )
                        default_calls.append(
                            (method.name, param_node.value, runtime_default, call.lineno)
                        )

        tools_by_method: dict[str, set[str]] = {}
        for tool_name, handler_name in gateway_module.TOOL_HANDLERS.items():
            tools_by_method.setdefault(handler_name, set()).add(tool_name)
        changed = True
        while changed:
            changed = False
            for caller_name, called_methods in calls_by_method.items():
                caller_tools = tools_by_method.get(caller_name, set())
                for called_method in called_methods & route_methods.keys():
                    inherited_tools = tools_by_method.setdefault(called_method, set())
                    previous_count = len(inherited_tools)
                    inherited_tools.update(caller_tools)
                    changed = changed or len(inherited_tools) != previous_count

        self.assertGreaterEqual(len(default_calls), 90)
        for method_name, param_name, runtime_default, line_number in default_calls:
            tool_names = tools_by_method.get(method_name, set())
            self.assertTrue(
                tool_names,
                msg=(
                    f"route method {method_name} has a literal runtime default at "
                    f"line {line_number} but is not reachable from an advertised tool"
                ),
            )
            for tool_name in sorted(tool_names):
                with self.subTest(tool=tool_name, param=param_name):
                    self.assertIn(
                        param_name,
                        gateway_module.TOOL_PARAM_NAMES[tool_name],
                    )
                    self.assertEqual(
                        runtime_default,
                        tool_manifest_module.tool_param_default(tool_name, param_name),
                    )

    def test_generated_tool_catalog_matches_gateway_specs(self) -> None:
        catalog = gateway_module.build_tool_catalog()

        self.assertEqual(len(catalog), len(gateway_module.TOOL_NAMES))
        for tool in catalog:
            with self.subTest(tool=tool["name"]):
                tool_name = tool["name"]
                self.assertIn(tool_name, gateway_module.TOOL_HANDLERS)
                self.assertEqual(
                    tool["metadata"]["category"],
                    gateway_module.TOOL_CATEGORIES[tool_name],
                )
                self.assertEqual(tool["metadata"]["legacyMethod"], tool_name)
                self.assertEqual(
                    tool["metadata"]["mutatesState"],
                    tool_name in gateway_module.MUTATING_TOOLS,
                )
                self.assertEqual(
                    set(tool["inputSchema"]["properties"]),
                    set(gateway_module.TOOL_PARAM_NAMES[tool_name]),
                )
                self.assertEqual(
                    tool["inputSchema"]["required"],
                    list(gateway_module.required_tool_params(tool_name)),
                )
                self.assertEqual(tool["inputSchema"]["additionalProperties"], False)
                self.assertEqual(tool["outputSchema"]["required"], ["result"])
                expected_result_schema = gateway_module.TOOL_RESULT_SCHEMAS.get(
                    tool_name,
                    gateway_module.OBJECT_RESULT_SCHEMA,
                )
                self.assertEqual(
                    tool["outputSchema"]["properties"]["result"],
                    expected_result_schema,
                )

    def test_generated_tool_catalog_isolated_from_caller_mutation(self) -> None:
        first_catalog = {
            tool["name"]: tool for tool in gateway_module.build_tool_catalog()
        }
        output_schema = first_catalog["arborist/get_semantic_skeleton"][
            "outputSchema"
        ]["properties"]["result"]
        position_schema = first_catalog["arborist/patch_ast_node_at_position"][
            "inputSchema"
        ]["properties"]["position"]
        original_minimum = position_schema["properties"]["row"]["minimum"]

        try:
            output_schema["callerMutation"] = True
            position_schema["properties"]["row"]["minimum"] = 99

            second_catalog = {
                tool["name"]: tool for tool in gateway_module.build_tool_catalog()
            }
            second_output_schema = second_catalog["arborist/get_semantic_skeleton"][
                "outputSchema"
            ]["properties"]["result"]
            second_position_schema = second_catalog[
                "arborist/patch_ast_node_at_position"
            ]["inputSchema"]["properties"]["position"]

            self.assertIsNot(output_schema, second_output_schema)
            self.assertNotIn("callerMutation", second_output_schema)
            self.assertIsNot(position_schema, second_position_schema)
            self.assertEqual(
                second_position_schema["properties"]["row"]["minimum"],
                original_minimum,
            )
        finally:
            output_schema.pop("callerMutation", None)
            position_schema["properties"]["row"]["minimum"] = original_minimum

    def test_generated_tool_descriptors_have_no_shared_mutable_containers(
        self,
    ) -> None:
        def collect_container_paths(
            value: object,
            path: tuple[str, ...] = (),
        ) -> list[tuple[int, tuple[str, ...]]]:
            containers: list[tuple[int, tuple[str, ...]]] = []
            if isinstance(value, dict):
                containers.append((id(value), path))
                for key, child in value.items():
                    containers.extend(
                        collect_container_paths(child, (*path, str(key)))
                    )
            elif isinstance(value, list):
                containers.append((id(value), path))
                for index, child in enumerate(value):
                    containers.extend(
                        collect_container_paths(child, (*path, str(index)))
                    )
            return containers

        for tool in gateway_module.build_tool_catalog():
            with self.subTest(tool=tool["name"]):
                seen_paths: dict[int, tuple[str, ...]] = {}
                for object_id, path in collect_container_paths(tool):
                    self.assertNotIn(
                        object_id,
                        seen_paths,
                        msg=(
                            f"shared mutable schema container at {seen_paths.get(object_id)} "
                            f"and {path}"
                        ),
                    )
                    seen_paths[object_id] = path

    def test_tool_catalog_script_and_snapshot_match_generated_catalog(self) -> None:
        repo_root = Path(__file__).resolve().parents[2]
        script_path = repo_root / "scripts" / "tool_catalog.py"
        snapshot_path = repo_root / "docs" / "tool-catalog.json"

        completed = subprocess.run(
            [sys.executable, str(script_path)],
            cwd=repo_root,
            check=True,
            capture_output=True,
            text=True,
        )

        generated = gateway_module.build_tool_catalog()
        self.assertEqual(json.loads(completed.stdout), generated)
        check_completed = subprocess.run(
            [sys.executable, str(script_path), "--check"],
            cwd=repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(check_completed.stdout, "")
        self.assertEqual(
            json.loads(snapshot_path.read_text(encoding="utf-8")),
            generated,
        )

    def test_tool_catalog_script_reports_outdated_snapshot(self) -> None:
        repo_root = Path(__file__).resolve().parents[2]
        script_path = repo_root / "scripts" / "tool_catalog.py"

        with tempfile.TemporaryDirectory() as temp_dir:
            snapshot_path = Path(temp_dir) / "tool-catalog.json"
            snapshot_path.write_text("[]\n", encoding="utf-8", newline="\n")

            completed = subprocess.run(
                [
                    sys.executable,
                    str(script_path),
                    "--check",
                    "--snapshot",
                    str(snapshot_path),
                ],
                cwd=repo_root,
                capture_output=True,
                text=True,
            )

        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("out of date", completed.stderr)

    def test_readme_tool_counts_match_generated_catalog(self) -> None:
        readme = Path(__file__).resolve().parents[2].joinpath("README.md").read_text(
            encoding="utf-8"
        )

        total_match = re.search(r"returns (\d+) tools", readme)
        self.assertIsNotNone(total_match)
        assert total_match is not None
        self.assertEqual(int(total_match.group(1)), len(gateway_module.TOOL_NAMES))

        expected_counts = {
            "Read": 0,
            "Write": 0,
            "VFS": 0,
            "Index": 0,
            "Trace": 0,
        }
        for category in gateway_module.TOOL_CATEGORIES.values():
            expected_counts[category.upper() if category == "vfs" else category.title()] += 1

        for label, expected_count in expected_counts.items():
            count_match = re.search(rf"{label} tools: (\d+)", readme)
            self.assertIsNotNone(count_match, msg=f"README missing {label} tool count")
            assert count_match is not None
            self.assertEqual(int(count_match.group(1)), expected_count)

    def test_gateway_suite_metadata_covers_all_advertised_tools(self) -> None:
        suite_manifest = MANIFEST["suites"]
        assert isinstance(suite_manifest, dict)

        expected_tools = set(gateway_module.TOOL_HANDLERS)
        covered_tools: set[str] = set()

        for suite_name in suite_manifest:
            module = importlib.import_module(SUITE_MODULES[suite_name])
            self.assertEqual(module.SUITE_NAME, suite_name)
            self.assertEqual(
                module.REQUIRES_EXTENSION,
                suite_manifest[suite_name]["requires_extension"],
            )

            module_tools = set(module.COVERED_TOOLS)
            self.assertTrue(module_tools, msg=f"{suite_name} must declare covered tools")
            self.assertTrue(
                module_tools <= expected_tools,
                msg=f"{suite_name} declares unknown tools: {sorted(module_tools - expected_tools)}",
            )
            covered_tools.update(module_tools)

        self.assertEqual(
            covered_tools,
            expected_tools,
            msg=(
                "gateway suite tool coverage drifted; missing="
                f"{sorted(expected_tools - covered_tools)}, extra={sorted(covered_tools - expected_tools)}"
            ),
        )

    def test_gateway_groups_match_extension_requirements(self) -> None:
        suite_manifest = MANIFEST["suites"]
        assert isinstance(suite_manifest, dict)

        for suite_name in GROUP_SUITES["gateway-fast"]:
            with self.subTest(group="gateway-fast", suite=suite_name):
                self.assertFalse(suite_manifest[suite_name]["requires_extension"])

        for suite_name in GROUP_SUITES["gateway-native"]:
            with self.subTest(group="gateway-native", suite=suite_name):
                self.assertTrue(suite_manifest[suite_name]["requires_extension"])

    def test_gateway_manifest_snapshot_matches_loaded_metadata(self) -> None:
        snapshot = build_manifest_snapshot()
        self.assertEqual(snapshot["suites"], SUITES)

        snapshot_groups = snapshot["groups"]
        assert isinstance(snapshot_groups, dict)
        self.assertEqual(set(snapshot_groups), set(GROUPS))
        for group_name, metadata in snapshot_groups.items():
            with self.subTest(group=group_name):
                self.assertEqual(tuple(metadata["suite_names"]), GROUP_SUITES[group_name])
                self.assertEqual(tuple(metadata["module_names"]), GROUP_MODULES[group_name])
                self.assertEqual(metadata["requires_extension"], GROUPS[group_name]["requires_extension"])

    def test_gateway_manifest_cli_emits_normalized_metadata(self) -> None:
        repo_root = Path(__file__).resolve().parents[2]
        script_path = repo_root / "scripts" / "gateway_suite_manifest.py"
        completed = subprocess.run(
            [sys.executable, str(script_path)],
            cwd=repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(json.loads(completed.stdout), build_manifest_snapshot())\n
