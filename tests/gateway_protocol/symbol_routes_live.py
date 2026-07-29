from __future__ import annotations

from tests.gateway_protocol.helpers import GatewayProtocolTestCase
from tests.gateway_protocol.semantic_fixtures import GatewaySemanticFixtureMixin
from tests.gateway_protocol.symbol_route_fixtures import GatewaySymbolRouteFixtureMixin
from tests.gateway_protocol.symbol_route_live_cases import (
    GatewaySymbolRouteLiveTestsMixin,
)

SUITE_NAME = "gateway-symbol-routes-native"
REQUIRES_EXTENSION = True
COVERED_TOOLS = (
    "arborist/list_symbols",
    "arborist/read_symbol",
    "arborist/read_symbol_at_position",
    "arborist/read_symbol_discovery_context_at_position",
    "arborist/search_symbols",
    "arborist/trace_symbol_graph",
    "arborist/trace_symbol_graph_at_position",
    "arborist/validate_patch_with_discovery_context",
    "arborist/validate_patch_with_graph_context",
    "arborist/validate_patch_with_neighborhood_context",
    "arborist/validate_patch_with_trace_context",
)


class GatewaySymbolRouteNativeTests(
    GatewaySymbolRouteLiveTestsMixin,
    GatewaySymbolRouteFixtureMixin,
    GatewaySemanticFixtureMixin,
    GatewayProtocolTestCase,
):
    pass
