from __future__ import annotations

from tests.gateway_protocol.helpers import GatewayProtocolTestCase
from tests.gateway_protocol.semantic_fixtures import GatewaySemanticFixtureMixin
from tests.gateway_protocol.symbol_route_fixtures import GatewaySymbolRouteFixtureMixin
from tests.gateway_protocol.symbol_route_patch_cases import (
    GatewaySymbolRoutePatchTestsMixin,
)
from tests.gateway_protocol.symbol_route_read_cases import (
    GatewaySymbolRouteReadTestsMixin,
)

SUITE_NAME = "gateway-symbol-routes"
REQUIRES_EXTENSION = False
COVERED_TOOLS = (
    "arborist/list_symbols",
    "arborist/list_symbols_context",
    "arborist/list_symbols_discovery_context",
    "arborist/list_symbols_neighborhood_context",
    "arborist/patch_ast_node_at_position",
    "arborist/patch_virtual_ast_node_at_position",
    "arborist/read_symbol",
    "arborist/read_symbol_at_position",
    "arborist/read_symbol_context",
    "arborist/read_symbol_context_at_position",
    "arborist/read_symbol_discovery_context",
    "arborist/read_symbol_discovery_context_at_position",
    "arborist/read_symbol_neighborhood_context",
    "arborist/read_symbol_neighborhood_context_at_position",
    "arborist/search_symbols",
    "arborist/search_symbols_context",
    "arborist/search_symbols_discovery_context",
    "arborist/search_symbols_neighborhood_context",
    "arborist/trace_symbol_graph",
    "arborist/trace_symbol_graph_at_position",
    "arborist/trace_symbol_neighborhood",
    "arborist/trace_symbol_neighborhood_at_position",
    "arborist/validate_patch_with_discovery_context",
    "arborist/validate_patch_with_discovery_context_at_position",
    "arborist/validate_patch_with_graph_context",
    "arborist/validate_patch_with_graph_context_at_position",
    "arborist/validate_patch_with_neighborhood_context",
    "arborist/validate_patch_with_neighborhood_context_at_position",
    "arborist/validate_patch_with_trace_context",
    "arborist/validate_patch_with_trace_context_at_position",
)


class GatewaySymbolRouteTests(
    GatewaySymbolRouteReadTestsMixin,
    GatewaySymbolRoutePatchTestsMixin,
    GatewaySymbolRouteFixtureMixin,
    GatewaySemanticFixtureMixin,
    GatewayProtocolTestCase,
):
    pass
